//! `LightboxImage` in a line: the button about `ChatLoadingImage`, and a failed load's card.

use std::{cell::RefCell, collections::HashMap};

use gpui::{
    AnyElement, App, ElementId, FontWeight, InteractiveElement as _, IntoElement as _, ObjectFit,
    ParentElement as _, SharedString, StatefulInteractiveElement as _, Styled as _,
    StyledImage as _, Window, div, img, prelude::FluentBuilder as _,
};

use super::{
    chip, handoff,
    images::{self, Failed, Load, Shown},
    select,
};
use crate::{
    appearance_settings::text_size,
    cindy_json::decode_uri_component,
    i18n::Translator,
    local_path::shape,
    markdown::hast::{Element, Hast, Prop},
    markdown_media, motion,
    session_view::flow::draw::shape as shaped,
    tip,
    transition::{self, Curve},
    units::{self, css},
    workspace_media,
};

/// The button's `marginTop` and `marginBottom`.
pub const MARGIN: f32 = 18.0;

/// `ChatLoadingImage`'s frame: `w-[280px] min-h-[180px]`.
const FRAME: (f32, f32) = (280.0, 180.0);

/// The frame's baseline under its top: the bottom of its centred 16px spinner.
const SPINNER: f32 = 98.0;

/// Where a drawn image stands.
enum State {
    /// The frame, drawn once.
    Loading(RefCell<Option<AnyElement>>),
    Ready(Shown, f32),
    /// `ImageMissingPlaceholder`, drawn once, a block among the lines.
    Failed(RefCell<Option<AnyElement>>, handoff::Metrics),
}

/// One `img` of a message that draws.
pub struct Picture {
    id: String,
    label: SharedString,
    tip: SharedString,
    /// The `width` attribute, which `height: auto` leaves alone in effect.
    width: Option<f32>,
    /// `min(50vw, 480px)` and `min(40vh, 420px)`.
    max: (f32, f32),
    /// Whether the button has an address to zoom: `cursor: pointer`.
    live: bool,
    state: State,
}

fn text<'a>(element: &'a Element, name: &str) -> Option<&'a str> {
    match element.prop(name) {
        Some(Prop::Text(value)) => Some(value),
        _ => None,
    }
}

fn collect<'a>(nodes: &'a [Hast], found: &mut Vec<&'a Element>) {
    for node in nodes {
        if let Hast::Element(element) = node {
            if element.tag == "img" {
                found.push(element);
            }
            collect(&element.children, found);
        }
    }
}

/// The button's width, then the image's box, in a line `avail` wide (`9.6i3a-blink-image.log`).
fn fit((w, h): (u32, u32), attr: Option<f32>, (max_w, max_h): (f32, f32), avail: f32) -> [f32; 3] {
    let (w, h) = (w as f32, h as f32);
    if w == 0.0 || h == 0.0 {
        return [0.0; 3];
    }
    // The button shrinks to the image before its percentage `max-width` counts.
    let own = attr.unwrap_or(if h > max_h { w * max_h / h } else { w });
    let button = own.min(avail);
    let width = button.min(max_w);
    [button, width, (width * h / w).min(max_h)]
}

/// `fallbackName`: the address's last segment, decoded where it decodes.
fn name(alt: &str, src: Option<&str>) -> String {
    if !alt.is_empty() {
        return alt.to_owned();
    }
    let last = src.and_then(|src| src.rsplit(['\\', '/']).next());
    let last = last.unwrap_or("image");
    decode_uri_component(last).unwrap_or_else(|| last.to_owned())
}

/// The card of a failed load, with the place a list's marker hangs by: its icon's bottom.
fn card(
    id: &str,
    name: &str,
    failed: Failed,
    (italic, weight): (bool, FontWeight),
    window: &Window,
    cx: &App,
) -> (AnyElement, handoff::Metrics) {
    let t = cx.global::<Translator>();
    let title = t.t(
        match failed.missing {
            true => "chat.imageMissing.title",
            false => "chat.imageMissing.unavailable",
        },
        &[],
    );
    let zoom = cx.global::<units::Zoom>().0;
    let wide = |words: &str, size: f32, weight: FontWeight| {
        let mut style = window.text_style();
        style.font_size = css(size).into();
        style.font_weight = weight;
        f32::from(shaped(words, &style, window).width) / zoom
    };
    let small = text_size(11.0, cx);
    let titled = wide(&title, 12.0, FontWeight::MEDIUM);
    // A cell breaks the title anywhere, so its widest character is the least it asks.
    let letter = |c: char| wide(c.encode_utf8(&mut [0; 4]), 12.0, FontWeight::MEDIUM);
    let letter = title.chars().map(letter).fold(0.0, f32::max);
    let named = wide(&chip::nowrap(name), small, weight);
    let again = failed.retry.is_some();
    // Retry is no narrower than its label asks.
    let retry = if again {
        images::retry_width(window, cx)
    } else {
        0.0
    };
    // The icon, the title's line and the name's, `gap-2` apart; then Retry.
    let content =
        32.0 + 8.0 + 16.0 + 8.0 + small * 1.6 + if again { 8.0 + images::RETRY } else { 0.0 };
    let height = (content + 34.0).max(140.0);
    let metrics = handoff::Metrics {
        baseline: 17.0 + (height - 34.0 - content) / 2.0 + 32.0,
        height,
        least: (letter.max(named).max(32.0).max(retry) + 34.0).min(280.0),
        most: (titled.max(named).max(retry) + 34.0).min(280.0),
        margin: 0.0,
    };
    let element = div()
        .font_weight(weight)
        .line_height(gpui::relative(1.6))
        .when(italic, |el| el.italic())
        .child(images::failed(id, name, failed, window, cx));
    (element.into_any_element(), metrics)
}

/// What stands about an `img`: an `em`'s slant and the weight, by the node.
pub type About = HashMap<*const Element, (bool, FontWeight)>;

/// The message's images that draw, each mounted as `<key>-g<n>`; `dir` is the task's folder.
pub fn read(
    key: &str,
    tree: &[Hast],
    dir: &str,
    about: &About,
    window: &mut Window,
    cx: &mut App,
) -> HashMap<*const Element, Picture> {
    let mut found = Vec::new();
    collect(tree, &mut found);
    let view = window.viewport_size();
    let (vw, vh) = (
        units::to_css(view.width, cx),
        units::to_css(view.height, cx),
    );
    let (mut out, mut wants) = (HashMap::new(), Vec::new());
    let (mut drawn, mut again) = (HashMap::new(), Vec::new());
    for (n, node) in found.into_iter().enumerate() {
        let id = format!("{key}-g{n}");
        let raw = text(node, "data-cindy-raw-local-image-src").or_else(|| text(node, "src"));
        // A local task's privileged message: its relative paths are searched for.
        let media = workspace_media::path(&id, raw, dir, (true, false), cx);
        if media.pending {
            continue;
        }
        let src = shape::image_src(media.src.as_deref().unwrap_or(""), dir, true);
        let src = src.filter(|src| !src.is_empty());
        drawn.insert(id.clone(), src.clone());
        // `ChatVideoView` comes with its chapter.
        if src.as_deref().is_some_and(markdown_media::is_managed_video) {
            continue;
        }
        let read = |src: &&String| {
            let schemes = ["xdt-image://", "cindy-media://", "xdt-file://", "data:"];
            images::web(src) || schemes.iter().any(|scheme| src.starts_with(scheme))
        };
        let load = match &src {
            // No address: the frame stays.
            None => Load::Loading,
            Some(url) if read(&url) => {
                if images::revised(&id, url, media.revision, cx) {
                    again.push((id.clone(), url.clone()));
                }
                // A load this image's own Retry started stands before the shared one.
                let held = images::own(&id, url, cx).or_else(|| images::held(url, cx));
                held.unwrap_or_else(|| {
                    wants.push(url.clone());
                    Load::Loading
                })
            }
            // A remote task's media comes with its chapter.
            Some(_) => Load::Unavailable,
        };
        let element = ElementId::Name(id.clone().into());
        // `animate-fade-in` once the image has loaded.
        let fade = Curve {
            length: motion::length(motion::FAST, cx),
            easing: motion::motion_ease_out,
            lerp: transition::lerp,
        };
        let loaded = f32::from(u8::from(matches!(load, Load::Ready(_))));
        let fade_id = ElementId::NamedChild(std::sync::Arc::new(element), "fade".into());
        let opacity = transition::keyed(fade_id, loaded, fade, window, cx);
        let alt = text(node, "alt").unwrap_or_default();
        let failed = |missing: bool, retry: Option<String>| {
            let failed = Failed {
                missing,
                block: true,
                retry,
            };
            let about = about.get(&(node as *const Element)).copied();
            let about = about.expect("each img has what stands about it");
            let (element, metrics) =
                card(&id, &name(alt, src.as_deref()), failed, about, window, cx);
            State::Failed(RefCell::new(Some(element)), metrics)
        };
        let state = match load {
            Load::Loading => State::Loading(RefCell::new(Some(images::loading(&id, cx)))),
            Load::Ready(shown) => State::Ready(shown, opacity),
            Load::Missing => failed(true, None),
            Load::Unavailable => failed(false, None),
            Load::Undecoded => failed(false, src.clone()),
        };
        let zoom = cx.global::<Translator>().t("chat.media.clickToZoom", &[]);
        let width = match node.prop("width") {
            Some(Prop::Text(width)) => width.parse().ok(),
            Some(Prop::Number(width)) => Some(*width as f32),
            _ => None,
        };
        let picture = Picture {
            id,
            label: if alt.is_empty() {
                zoom.clone()
            } else {
                alt.to_owned()
            }
            .into(),
            tip: text(node, "title").map_or(zoom, str::to_owned).into(),
            width,
            max: ((vw / 2.0).min(480.0), (vh * 0.4).min(420.0)),
            live: src.is_some(),
            state,
        };
        out.insert(node as *const Element, picture);
    }
    images::mounted(key, &drawn, cx);
    if !wants.is_empty() || !again.is_empty() {
        cx.defer(move |cx| {
            images::fetch(wants, cx);
            for (id, url) in again {
                images::reload(&id, &url, cx);
            }
        });
    }
    out
}

impl Picture {
    /// The failed card's metrics; none while the image is an atom of its line.
    pub fn card(&self) -> Option<handoff::Metrics> {
        match &self.state {
            State::Failed(_, metrics) => Some(*metrics),
            _ => None,
        }
    }

    /// The failed card, drawn once.
    pub fn block(&self) -> AnyElement {
        let State::Failed(card, _) = &self.state else {
            panic!("an image that loads is no block");
        };
        card.take().expect("a card is drawn once")
    }

    /// The button's box in a line `avail` wide: its width and height, in CSS pixels.
    pub fn size(&self, avail: f32) -> (f32, f32) {
        match &self.state {
            State::Ready(shown, _) => {
                let [button, _, height] = fit(shown.natural, self.width, self.max, avail);
                (button, height)
            }
            _ => FRAME,
        }
    }

    /// The button's margin box over and under the baseline, in a line `avail` wide.
    pub fn reach(&self, avail: f32) -> (f32, f32) {
        match &self.state {
            // A loaded image has no line of its own: its bottom is the baseline.
            State::Ready(..) => (self.size(avail).1 + MARGIN, MARGIN),
            _ => (SPINNER + MARGIN, FRAME.1 - SPINNER + MARGIN),
        }
    }

    /// The least and the most a line asks for the button: a percentage `max-width` asks nothing.
    pub fn content(&self) -> (f32, f32) {
        match &self.state {
            State::Ready(..) => (0.0, self.size(f32::INFINITY).0),
            _ => (FRAME.0, FRAME.0),
        }
    }

    /// The button in a line `avail` wide; its click and menu come with 9.13.
    pub fn atom(&self, avail: f32) -> AnyElement {
        let selector = self.id.clone();
        let button = div()
            .id(ElementId::Name(self.id.clone().into()))
            .debug_selector(move || selector)
            .relative()
            .flex_none()
            .aria_label(self.label.clone())
            .when(self.live, |el| el.cursor_pointer())
            .tooltip(tip::tip(self.tip.clone()))
            .child(select::still(0.0, 0.0));
        match &self.state {
            State::Loading(frame) => {
                let frame = frame.take().expect("a frame is drawn once");
                button.w(css(FRAME.0)).child(frame).into_any_element()
            }
            State::Ready(shown, opacity) => {
                let [wide, width, height] = fit(shown.natural, self.width, self.max, avail);
                let selector = format!("{}-img", self.id);
                let picture = img(shown.image.clone())
                    .debug_selector(move || selector)
                    .w(css(width))
                    .h(css(height))
                    .object_fit(ObjectFit::Fill)
                    .opacity(*opacity);
                button
                    .w(css(wide))
                    .h(css(height))
                    .child(picture)
                    .into_any_element()
            }
            State::Failed(..) => panic!("a failed image is a block"),
        }
    }
}

#[cfg(test)]
mod tests;
