//! `ChatImageView`'s `user-attached` face over the bubble; its click and menu come with 9.13.

use std::{collections::HashMap, path::Path, sync::Arc};

use anyhow::{Context as _, Result, bail};
use ely_gpui_component::{
    buttons::{Button, ButtonVariant},
    motion::Spinner,
    primitives::IconName,
    theme::{ActiveTheme as _, ControlSize, IconSize, TextSize},
};
use gpui::{
    AnyElement, App, AppContext as _, ElementId, FontWeight, InteractiveElement as _,
    IntoElement as _, ParentElement as _, RenderImage, SMOOTH_SVG_SCALE_FACTOR,
    StatefulInteractiveElement as _, Styled as _, SvgRenderer, Window, div, img,
    prelude::FluentBuilder as _,
};
use image::{Frame, ImageFormat, RgbaImage, imageops};

use super::{SessionView, chip, flow::draw::shape};
use crate::{
    appearance_settings::text_size,
    chat_message::merge::{Fields, get},
    i18n::Translator,
    local_file, lucide,
    media_store::{self, Failure},
    motion,
    rich_answer::js::Js,
    sessions,
    theme::Appearance,
    tip,
    transition::{self, Curve},
    units::{Zoom, css},
};

/// A decoded image: its natural size, and a long image's square top.
#[derive(Clone)]
pub struct Shown {
    pub image: Arc<RenderImage>,
    pub natural: (u32, u32),
    top: Option<Arc<RenderImage>>,
}

/// Where an image's load stands.
#[derive(Clone)]
pub enum Load {
    Loading,
    Ready(Shown),
    /// `useImageLoadFailure`'s `missing`: the read said the file is not there.
    Missing,
    Unavailable,
    /// Bytes that were read and did not decode: `readable`, so a retry is offered.
    Undecoded,
}

/// The mounted content's loads: by address, an image's own, and each image's hook revision.
#[derive(Default)]
pub struct Held(
    HashMap<String, Load>,
    HashMap<String, (String, Load)>,
    HashMap<String, u64>,
);

fn held_mut(cx: &mut App) -> Option<&mut Held> {
    let shown = cx.global_mut::<SessionView>().shown.as_mut();
    shown?.content.as_mut().map(|content| &mut content.images)
}

pub fn held(url: &str, cx: &App) -> Option<Load> {
    let shown = cx.global::<SessionView>().shown.as_ref();
    let content = shown.and_then(|shown| shown.content.as_ref());
    content.and_then(|content| content.images.0.get(url).cloned())
}

/// `imageSize.height / imageSize.width > 3`.
fn long((width, height): (u32, u32)) -> bool {
    width > 0 && f64::from(height) / f64::from(width) > 3.0
}

/// The square top of each frame, which `object-fit: cover` at `object-position: top` shows.
fn top(image: &RenderImage, width: u32) -> Result<RenderImage> {
    let frame = |n: usize| {
        let size = image.size(n);
        let bytes = image.as_bytes(n).context("a frame has bytes")?.to_vec();
        let whole = RgbaImage::from_raw(size.width.0 as u32, size.height.0 as u32, bytes);
        let whole = whole.context("a frame fills its size")?;
        let square = imageops::crop_imm(&whole, 0, 0, width, width).to_image();
        anyhow::Ok(Frame::from_parts(square, 0, 0, image.delay(n)))
    };
    let frames: Result<Vec<Frame>> = (0..image.frame_count()).map(frame).collect();
    Ok(RenderImage::new(frames?))
}

/// The bytes decoded by what they are, as Chromium sniffs an image; `vector` is an SVG's type.
fn decode(bytes: Vec<u8>, vector: bool, svg: SvgRenderer) -> Result<Shown> {
    let format = match image::guess_format(&bytes) {
        _ if vector => gpui::ImageFormat::Svg,
        Ok(ImageFormat::Png) => gpui::ImageFormat::Png,
        Ok(ImageFormat::Jpeg) => gpui::ImageFormat::Jpeg,
        Ok(ImageFormat::Gif) => gpui::ImageFormat::Gif,
        Ok(ImageFormat::WebP) => gpui::ImageFormat::Webp,
        Ok(ImageFormat::Bmp) => gpui::ImageFormat::Bmp,
        Ok(ImageFormat::Ico) => gpui::ImageFormat::Ico,
        Ok(other) => bail!("no decoder for {other:?}"),
        Err(error) => bail!(error),
    };
    let image = gpui::Image::from_bytes(format, bytes).to_image_data(svg)?;
    let size = image.size(0);
    // gpui draws an SVG at twice its size.
    let scale = if vector { SMOOTH_SVG_SCALE_FACTOR } else { 1.0 };
    let natural = |pixels: i32| (pixels as f32 / scale).round() as u32;
    let natural = (natural(size.width.0), natural(size.height.0));
    let top = match long(natural) {
        true => Some(Arc::new(top(&image, size.width.0 as u32)?)),
        false => None,
    };
    Ok(Shown {
        image,
        natural,
        top,
    })
}

/// A local file's read and decode. Cindy probes the stores alone, so each failure is unavailable.
fn local(url: &str, svg: SvgRenderer) -> Load {
    let served = match local_file::read(url) {
        Ok(served) => served,
        Err(status) => {
            log::warn!(target: "sessionView", "image {url} answered {status}");
            return Load::Unavailable;
        }
    };
    match decode(served.bytes, served.mime == "image/svg+xml", svg) {
        Ok(shown) => {
            let (width, height) = shown.natural;
            log::info!(target: "sessionView", "image {url} loaded: {width}x{height}");
            Load::Ready(shown)
        }
        Err(error) => {
            log::error!(target: "sessionView", "image {url} not decoded: {error:#}");
            Load::Unavailable
        }
    }
}

/// The read and the decode, off the main thread.
fn load(user_data: &Path, url: &str, svg: SvgRenderer) -> Load {
    if url.starts_with("xdt-file://") {
        return local(url, svg);
    }
    let bytes = match media_store::read(user_data, url) {
        Ok(bytes) => bytes,
        Err(Failure::Missing) => {
            log::warn!(target: "sessionView", "image {url} is missing");
            return Load::Missing;
        }
        Err(Failure::Unavailable(error)) => {
            log::error!(target: "sessionView", "image {url} not read: {error}");
            return Load::Unavailable;
        }
    };
    match decode(bytes, false, svg) {
        Ok(shown) => {
            let (width, height) = shown.natural;
            log::info!(target: "sessionView", "image {url} loaded: {width}x{height}");
            Load::Ready(shown)
        }
        Err(error) => {
            log::error!(target: "sessionView", "image {url} not decoded: {error:#}");
            Load::Undecoded
        }
    }
}

/// One load off the main thread; `slot` finds where its answer goes, if that still stands.
fn start(url: String, slot: impl Fn(&mut Held) -> Option<&mut Load> + 'static, cx: &mut App) {
    let (user_data, svg) = (sessions::user_data(cx), cx.svg_renderer());
    let loading = cx.background_spawn(async move { load(&user_data, &url, svg) });
    cx.spawn(async move |cx| {
        let load = loading.await;
        cx.update(|cx| {
            if let Some(slot) = held_mut(cx).and_then(&slot) {
                *slot = load;
                cx.refresh_windows();
            }
        });
    })
    .detach();
}

/// Starts the loads the frame lacked, each address once.
pub fn fetch(urls: Vec<String>, cx: &mut App) {
    for url in urls {
        let Some(held) = held_mut(cx) else {
            return;
        };
        if held.0.contains_key(&url) {
            continue;
        }
        held.0.insert(url.clone(), Load::Loading);
        let key = url.clone();
        start(url, move |held| held.0.get_mut(&key), cx);
    }
}

/// The load the image mounted as `id` started with its own Retry, while it shows `url`.
pub fn own(id: &str, url: &str, cx: &App) -> Option<Load> {
    let shown = cx.global::<SessionView>().shown.as_ref();
    let content = shown.and_then(|shown| shown.content.as_ref());
    let (held, load) = content?.images.1.get(id)?;
    (held == url).then(|| load.clone())
}

/// The message `key` drew these images: a Retry's load of any other of its images unmounts.
pub fn mounted(key: &str, drawn: &HashMap<String, Option<String>>, cx: &mut App) {
    let Some(held) = held_mut(cx) else {
        return;
    };
    let prefix = format!("{key}-g");
    held.1.retain(|id, (url, _)| {
        !id.starts_with(&prefix)
            || drawn
                .get(id)
                .is_some_and(|shown| shown.as_ref() == Some(url))
    });
    held.2
        .retain(|id, _| !id.starts_with(&prefix) || drawn.contains_key(id));
}

/// The image mounted as `id` mounts anew: its own load of `url` stands, loading.
fn remount(id: &str, url: &str, cx: &mut App) -> bool {
    let held = held_mut(cx);
    held.is_some_and(|held| {
        let own = (url.to_owned(), Load::Loading);
        held.1.insert(id.to_owned(), own);
        true
    })
}

/// Starts the load a remount set; it lands only while this image still shows this address.
pub fn reload(id: &str, url: &str, cx: &mut App) {
    let (key, address) = (id.to_owned(), url.to_owned());
    start(
        url.to_owned(),
        move |held| {
            let (shown, load) = held.1.get_mut(&key)?;
            (*shown == address).then_some(load)
        },
        cx,
    );
}

/// `WorkspaceMediaSrc`'s `key={revision}`: an image whose revision grew mounts anew.
pub fn revised(id: &str, url: &str, revision: u64, cx: &mut App) -> bool {
    let Some(held) = held_mut(cx) else {
        return false;
    };
    let before = held.2.insert(id.to_owned(), revision);
    if before.is_none_or(|before| before == revision) {
        return false;
    }
    log::info!(target: "sessionView", "image {url} was found for {id}: it loads anew");
    remount(id, url, cx)
}

/// `retry`: the image mounted as `id` alone mounts anew, so its load starts over.
pub fn retry(id: &str, url: &str, cx: &mut App) {
    if !remount(id, url, cx) {
        return;
    }
    log::info!(target: "sessionView", "image {url} is tried again for {id}");
    reload(id, url, cx);
    cx.refresh_windows();
}

/// `useImageLoadFailure`'s `focus` listener: every failed image is tried again.
pub fn focused(cx: &mut App) {
    let Some(held) = held_mut(cx) else {
        return;
    };
    let before = held.0.len() + held.1.len();
    let stands = |load: &Load| matches!(load, Load::Loading | Load::Ready(_));
    held.0.retain(|_, load| stands(load));
    held.1.retain(|_, (_, load)| stands(load));
    if held.0.len() + held.1.len() < before {
        log::info!(target: "sessionView", "failed images are tried again on focus");
        cx.refresh_windows();
    }
}

/// An image's address and `originalName`.
fn named(image: &Js) -> Option<(&str, &str)> {
    let image = image.obj()?;
    Some((image.get("url")?.str()?, image.get("originalName")?.str()?))
}

/// The message's `images`.
pub fn images(message: &Fields) -> Vec<(&str, &str)> {
    let list = get(message, "images").and_then(Js::arr).unwrap_or_default();
    list.iter().filter_map(named).collect()
}

/// `maxWidth: 280, maxHeight: 180` on the natural size, its ratio kept.
fn fitted((width, height): (u32, u32)) -> (f32, f32) {
    let (width, height) = (f64::from(width), f64::from(height));
    let (wide, tall) = (width > 280.0, height > 180.0);
    let fitted = match 280.0 * height <= 180.0 * width {
        true if wide => (280.0, 280.0 * height / width),
        false if tall => (180.0 * width / height, 180.0),
        _ => (width, height),
    };
    (fitted.0 as f32, fitted.1 as f32)
}

/// `ChatLoadingImage`'s frame while the image loads.
pub fn loading(id: &str, cx: &App) -> AnyElement {
    let appearance = cx.global::<Appearance>();
    let label = cx
        .global::<Translator>()
        .t("chat.imageMissing.loading", &[]);
    let (selector, spinner) = (format!("{id}-loading"), format!("{id}-spinner"));
    div()
        .id(ElementId::Name(selector.clone().into()))
        .debug_selector(move || selector)
        .aria_label(label)
        .self_start()
        .flex()
        .items_center()
        .justify_center()
        .w(css(280.0))
        .max_w_full()
        .min_h(css(180.0))
        .rounded(css(12.0))
        .bg(appearance.token("surface-chip"))
        .child(
            Spinner::new(ElementId::Name(spinner.into()))
                .size(IconSize::Md)
                .color(appearance.token("text-tertiary")),
        )
        .into_any_element()
}

/// What an `ImageMissingPlaceholder` shows beside its file's name.
pub struct Failed {
    pub missing: bool,
    /// A block, `maxWidth` wide where there is room, in place of a flex item sized by its content.
    pub block: bool,
    /// The address `onRetry` loads again.
    pub retry: Option<String>,
}

/// The height of the card's Retry, Cindy's `sm` button.
pub const RETRY: f32 = 28.0;

/// Retry's width in CSS pixels: Ely's small button about its label, as Ely lays it out.
pub fn retry_width(window: &Window, cx: &App) -> f32 {
    let (theme, rem) = (cx.theme(), window.rem_size());
    let mut style = window.text_style();
    style.font_size = theme.text_size(TextSize::Sm).into();
    style.font_weight = FontWeight::MEDIUM;
    let label = cx.global::<Translator>().t("commonUi.retry", &[]);
    // gpui lays a text out to the whole pixel above its width.
    let label = shape(&label, &style, window).width.ceil();
    let sides = theme.control_padding(ControlSize::Sm).to_pixels(rem) * 2.0;
    // `border_1` is a window pixel a side.
    f32::from(label + sides + gpui::px(2.0)) / cx.global::<Zoom>().0
}

/// `ImageMissingPlaceholder`.
pub fn failed(id: &str, name: &str, card: Failed, window: &Window, cx: &App) -> AnyElement {
    let Failed {
        missing,
        block,
        retry,
    } = card;
    // gpui sizes a truncated line by what it kept: a name past the box fills it, as in CSS.
    let (rem, small) = (window.rem_size(), css(text_size(11.0, cx)));
    // `truncate` draws the name on one line; the tip keeps it as stored.
    let shown = chip::nowrap(name);
    let run = window.text_style().to_run(shown.len());
    let line = window.text_system();
    let line = line.shape_line(shown.clone().into(), small.to_pixels(rem), &[run], None);
    let fills = line.width > css(246.0).to_pixels(rem);
    let appearance = cx.global::<Appearance>();
    let muted = appearance.token("text-tertiary");
    let key = match missing {
        true => "chat.imageMissing.title",
        false => "chat.imageMissing.unavailable",
    };
    let (selector, file) = (format!("{id}-failed"), format!("{id}-name"));
    let name = name.to_owned();
    div()
        .debug_selector(move || selector)
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .gap(css(8.0))
        .max_w(css(280.0))
        .when(fills || block, |el| el.w(css(280.0)))
        .when(block, |el| el.max_w_full())
        .min_h(css(140.0))
        .rounded(css(12.0))
        .border(css(1.0))
        .border_dashed()
        .border_color(appearance.hsl_token("border"))
        .bg(appearance.hsl_token("muted"))
        .p(css(16.0))
        .text_color(appearance.hsl_token("muted-foreground"))
        .child(lucide::icon(IconName::ImageOff.svg(), 32.0, 2.0).text_color(muted))
        .child(
            div()
                .text_size(css(12.0))
                .line_height(css(16.0))
                .font_weight(FontWeight::MEDIUM)
                .child(cx.global::<Translator>().t(key, &[])),
        )
        .child(
            div()
                .id(ElementId::Name(file.clone().into()))
                .debug_selector(move || file)
                .max_w_full()
                .truncate()
                .text_size(small)
                .text_color(muted)
                .child(shown)
                .tooltip(tip::mono(name.clone())),
        )
        .children(retry.map(|url| {
            let t = cx.global::<Translator>();
            let label = t.t("commonUi.retry", &[]);
            let mounted = id.to_owned();
            let again = Button::new(ElementId::Name(format!("{id}-retry").into()), label.clone())
                .variant(ButtonVariant::Secondary)
                .size(ControlSize::Sm)
                .name(format!("{label}: {name}"))
                .on_click(move |_, _, cx| {
                    cx.stop_propagation();
                    self::retry(&mounted, &url, cx);
                });
            let selector = format!("{id}-retry");
            div()
                .debug_selector(move || selector)
                .flex()
                .flex_none()
                .items_center()
                .h(css(RETRY))
                .child(again)
        }))
        .into_any_element()
}

/// The loaded image at `opacity`: within 280 by 180, or a long image's 240 square and its hint.
fn ready(id: &str, name: &str, shown: &Shown, opacity: f32, cx: &App) -> AnyElement {
    let picture = |image: &Arc<RenderImage>| {
        let hovered = ElementId::Name(format!("{id}-picture").into());
        img(image.clone())
            .id(hovered)
            .aria_label(name.to_owned())
            .rounded(css(12.0))
            .cursor_pointer()
            .opacity(opacity)
    };
    let selector = id.to_owned();
    let Some(top) = &shown.top else {
        let (width, height) = fitted(shown.natural);
        return picture(&shown.image)
            .debug_selector(move || selector)
            .w(css(width))
            .h(css(height))
            .into_any_element();
    };
    let appearance = cx.global::<Appearance>();
    let hint = format!("{id}-hint");
    let pill = div()
        .debug_selector(move || hint)
        .absolute()
        .left(css(8.0))
        .bottom(css(8.0))
        .max_w(css(224.0))
        .rounded(css(9999.0))
        .bg(appearance.token("surface-elevated"))
        .px(css(8.0))
        .py(css(4.0))
        .text_size(css(text_size(12.0, cx)))
        .line_height(gpui::relative(1.33))
        .text_color(appearance.token("text-primary"))
        .child(
            cx.global::<Translator>()
                .t("chat.media.longImagePreview", &[]),
        );
    div()
        .debug_selector(move || selector)
        .relative()
        .size(css(240.0))
        .child(picture(top).size_full())
        .child(pill)
        .into_any_element()
}

/// One image by where its load stands; `wants` gathers the loads not yet asked.
fn image(
    id: String,
    (url, name): (&str, &str),
    wants: &mut Vec<String>,
    window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    let load = held(url, cx).unwrap_or_else(|| {
        wants.push(url.to_owned());
        Load::Loading
    });
    let element = ElementId::Name(id.clone().into());
    let child = |name: &'static str| {
        ElementId::NamedChild(std::sync::Arc::new(element.clone()), name.into())
    };
    // `animate-fade-in` once the image has loaded.
    let fade = Curve {
        length: motion::length(motion::FAST, cx),
        easing: motion::motion_ease_out,
        lerp: transition::lerp,
    };
    let loaded = f32::from(u8::from(matches!(load, Load::Ready(_))));
    let shown = transition::keyed(child("fade"), loaded, fade, window, cx);
    // `hover:opacity-90 transition-opacity`; hover ignores key presses.
    let hovered = window.use_keyed_state(child("hover"), cx, |_, _| false);
    let dim = Curve {
        length: motion::length(motion::FAST, cx),
        easing: motion::ease_move,
        lerp: transition::lerp,
    };
    let over = if *hovered.read(cx) { 0.9 } else { 1.0 };
    let over = transition::keyed(child("dim"), over, dim, window, cx);
    let face = match &load {
        Load::Loading => return loading(&id, cx),
        Load::Missing | Load::Unavailable | Load::Undecoded => {
            // The user's card offers no retry before 9.13.
            let card = Failed {
                missing: matches!(load, Load::Missing),
                block: false,
                retry: None,
            };
            return failed(&id, name, card, window, cx);
        }
        Load::Ready(image) => ready(&id, name, image, shown * over, cx),
    };
    div()
        .id(element)
        .hover_listener_mode(gpui::HoverListenerMode::InputModalityIndependent)
        .on_hover(move |over, _, cx| {
            hovered.update(cx, |hovered, cx| {
                *hovered = *over;
                cx.notify();
            })
        })
        .child(face)
        .into_any_element()
}

/// `imageAttachmentNodes`: the message's images, each a child of its column.
pub fn elements(
    key: &str,
    images: &[(&str, &str)],
    window: &mut Window,
    cx: &mut App,
) -> Vec<AnyElement> {
    let mut wants = Vec::new();
    let mut one = |(n, at): (usize, &(&str, &str))| {
        image(format!("{key}-image-{n}"), *at, &mut wants, window, cx)
    };
    let drawn = images.iter().enumerate().map(&mut one).collect();
    if !wants.is_empty() {
        cx.defer(move |cx| fetch(wants, cx));
    }
    drawn
}

#[cfg(test)]
pub(super) mod tests;
