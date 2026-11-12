//! `SessionHandoffCard`: a session-card link's block in an assistant message.

use std::time::Duration;

use ely_gpui_component::{primitives::IconName, theme::ActiveTheme as _};
use gpui::{
    AnyElement, App, Div, ElementId, FocusHandle, FontWeight, Hsla, InteractiveElement as _,
    IntoElement as _, ParentElement as _, Role, StatefulInteractiveElement as _, Styled as _,
    TextStyle, Window, div, white,
};

use super::{
    chip::nowrap,
    flow::{self, Atom},
    link_chip::act,
    select::{self, Part},
};
use crate::{
    appearance_settings::text_size,
    deep_link::Card,
    handoff_card as words,
    i18n::Translator,
    js_date, lucide, motion, press, router,
    theme::Appearance,
    transition::{self, Curve},
    units::css,
};

/// The card's first baseline under its top: the bottom of the icon in its 36px circle.
const BASELINE: f32 = 46.0;

/// What a card's place takes from it, in CSS pixels: its first baseline, height and content widths.
#[derive(Clone, Copy)]
pub struct Metrics {
    pub baseline: f32,
    pub height: f32,
    pub least: f32,
    pub most: f32,
    /// The block's margin above and below: the card's `my-2`.
    pub margin: f32,
}

/// A label's widest character and its whole width, `spacing` after each character.
fn widths(label: &str, size: f32, style: &TextStyle, spacing: f32, window: &Window) -> (f32, f32) {
    let rem = window.rem_size();
    let shaped = window.text_system().shape_line(
        label.to_owned().into(),
        css(size).to_pixels(rem),
        &[style.to_run(label.len())],
        None,
    );
    let zoom = f32::from(rem) / 16.0;
    let wide = |atom: &Atom| atom.advance / zoom + spacing;
    let atoms = flow::atoms(&shaped);
    // A space hangs at a break and counts nothing.
    let seen = atoms
        .iter()
        .filter(|atom| !label[atom.at..].starts_with(flow::hangs));
    let least = seen.map(wide).fold(0.0, f32::max);
    (least, atoms.iter().map(wide).sum())
}

/// The card's content in a cell, whose `overflow-wrap: anywhere` breaks the badge and the short id.
pub(super) fn content(
    (title, badge, when, short): (&str, &str, (&str, bool), &str),
    (solid, weight): (bool, FontWeight),
    window: &Window,
    cx: &App,
) -> (f32, f32) {
    let sizes = [10.0, 11.0, 12.0, 14.0].map(|size| text_size(size, cx));
    let styled = |weight: FontWeight| TextStyle {
        font_weight: weight,
        ..window.text_style()
    };
    let (_, title) = widths(title, sizes[3], &styled(FontWeight::SEMIBOLD), 0.0, window);
    let badge = widths(badge, sizes[0], &styled(FontWeight::MEDIUM), 0.4, window);
    let edge = if solid { 16.0 } else { 18.0 };
    let first = |badge: f32| title + 8.0 + badge + edge;
    let mut small = styled(weight);
    let (_, dot) = widths("\u{b7}", sizes[2], &small, 0.0, window);
    // The last-active words follow the history icon; the never-active ones slant alone.
    let lead = match when.1 {
        true => 18.0,
        false => {
            small.font_style = gpui::FontStyle::Italic;
            0.0
        }
    };
    let (_, when) = widths(when.0, sizes[2], &small, 0.0, window);
    let mono = TextStyle {
        font_family: cx.theme().mono_family.clone(),
        ..styled(weight)
    };
    let short = widths(short, sizes[1], &mono, 0.0, window);
    let second = |short: f32| lead + when + 6.0 + dot + 6.0 + short;
    // The circle, the arrow, two gaps, `p-4` and the border.
    let rest = 122.0;
    (
        rest + first(badge.0).max(second(short.0)),
        rest + first(badge.1).max(second(short.1)),
    )
}

/// `tracking-[0.4px]`: each character as wide as it is shaped, and 0.4px more.
fn tracked(id: &str, label: &str, size: f32, weight: FontWeight, window: &Window) -> Div {
    let mut style = window.text_style();
    style.font_weight = weight;
    let rem = window.rem_size();
    let shaped = window.text_system().shape_line(
        label.to_owned().into(),
        css(size).to_pixels(rem),
        &[style.to_run(label.len())],
        None,
    );
    let starts = label.char_indices().map(|(at, _)| shaped.x_for_index(at));
    let ends = starts.clone().skip(1).chain([shaped.width]);
    let letters = label.char_indices().zip(starts.zip(ends));
    let letters = letters.map(|((at, c), (from, to))| {
        let letter = c.to_string();
        let wide = div().w(to - from + css(0.4).to_pixels(rem));
        let drawn = wide.child(letter.clone()).into_any_element();
        select::space(id, at, &letter, drawn)
    });
    div().flex().children(letters)
}

/// `dispatcherSessionId` and `dispatcherTitle`: the task this view shows, and its stored title.
fn from(cx: &App) -> Option<router::Handoff> {
    let dispatcher = router::active_session(cx).filter(|id| !id.is_empty())?;
    let title = super::session(cx).map(|task| task.title.clone());
    Some(router::Handoff { dispatcher, title })
}

/// The card mounted as `id` with its metrics; `weight` is what it inherits.
pub fn element(
    id: &str,
    card: &Card,
    weight: FontWeight,
    focus: FocusHandle,
    window: &mut Window,
    cx: &mut App,
) -> (AnyElement, Metrics) {
    let element = ElementId::Name(id.to_owned().into());
    let child = |name: &'static str| {
        ElementId::NamedChild(std::sync::Arc::new(element.clone()), name.into())
    };
    let (press, state) = press::state(&element, (&focus, false), window, cx);
    let t = cx.global::<Translator>();
    let (title, (badge, solid)) = (words::title(card, t), words::badge(card.wake, t));
    // `lastActive ? … : …`: an empty value is none.
    let seen = card.last_active.as_deref().filter(|at| !at.is_empty());
    let seen = seen.map(|at| words::last_active(at, js_date::now(), t));
    let never = words::never(t);
    // A text's white space collapses, as `nowrap` and `normal` both draw it.
    let (title, short) = (nowrap(&title), nowrap(&words::short(&card.session_id)));
    let when = seen
        .as_deref()
        .map_or((never.as_str(), false), |seen| (seen, true));
    let upper = badge.to_uppercase();
    let (least, most) = content((&title, &upper, when, &short), (solid, weight), window, cx);
    let a = cx.global::<Appearance>();
    let (ink, paper) = (a.hsl_token("foreground"), a.hsl_token("background"));
    let pick = |rest: Hsla, lit: Hsla| if state.hovered { lit } else { rest };
    let edge = pick(
        a.token("msg-tool-card-border"),
        a.token("msg-assistant-text"),
    );
    let disc = pick(a.hsl_token("content-area"), ink);
    let glyph = pick(ink, paper);
    let desc = a.token("settings-section-desc");
    let arrow = pick(desc, ink);
    let (fill, text, meta) = (
        a.token("msg-tool-card-bg"),
        a.token("msg-tool-card-text"),
        a.token("cmd-palette-item-meta"),
    );
    let (send, send_icon, palette) = (
        a.token("send-btn-bg"),
        a.token("send-btn-icon"),
        a.token("cmd-palette-bg"),
    );
    // `duration-150` is no motion token, so reduced motion leaves it.
    let mut eased = |name, color| {
        let curve = Curve {
            length: Duration::from_millis(150),
            easing: motion::ease_move,
            lerp: transition::mix_srgb,
        };
        transition::keyed(child(name), color, curve, window, cx)
    };
    let (edge, disc, glyph, arrow) = (
        eased("edge", edge),
        eased("disc", disc),
        eased("glyph", glyph),
        eased("arrow", arrow),
    );
    let mono = cx.theme().mono_family.clone();
    let sizes = [10.0, 11.0, 12.0, 14.0].map(|size| text_size(size, cx));
    let part = |name: &str| {
        let selector = format!("{id}-{name}");
        move || selector
    };
    let block = |name: &str| format!("{id}-{name}");
    let label = tracked(
        &block("badge"),
        &upper,
        sizes[0],
        FontWeight::MEDIUM,
        window,
    )
    .debug_selector(part("badge"))
    .flex_none()
    .rounded(css(9999.0))
    .px(css(8.0))
    .py(css(2.0))
    .text_size(css(sizes[0]))
    .font_weight(FontWeight::MEDIUM)
    .line_height(css(16.0));
    let label = match solid {
        true => label.bg(send).text_color(send_icon),
        false => (label.border(css(1.0)).border_color(send))
            .bg(palette)
            .text_color(send),
    };
    let name = div()
        .debug_selector(part("title"))
        .relative()
        .min_w_0()
        .truncate()
        .text_size(css(sizes[3]))
        .font_weight(FontWeight::SEMIBOLD)
        .line_height(css(20.0))
        .text_color(text)
        .child(select::label(&block("title"), &title))
        .child(title);
    let small = || {
        div()
            .relative()
            .text_size(css(sizes[2]))
            .line_height(css(16.0))
    };
    let cut = |said: String, color: Hsla| {
        let under = select::label(&block("when"), &said);
        (small().min_w_0().truncate().text_color(color))
            .child(under)
            .child(said)
    };
    let when = match seen {
        Some(seen) => {
            let icon = (div().debug_selector(part("history")))
                .child(lucide::icon(lucide::history().as_bytes(), 12.0, 2.0).text_color(desc));
            vec![
                // The icon is a place before the words.
                select::space(&block("history"), 0, "", icon.into_any_element()),
                cut(seen, desc).into_any_element(),
            ]
        }
        None => vec![cut(never, meta).italic().into_any_element()],
    };
    let short = div()
        .debug_selector(part("short"))
        .flex_none()
        .font_family(mono)
        .text_size(css(sizes[1]))
        .line_height(css(16.0))
        .text_color(meta)
        .relative()
        .child(select::label(&block("short"), &short))
        .child(short);
    let lines = div()
        .relative()
        .min_w_0()
        .flex_1()
        .flex()
        .flex_col()
        .gap(css(4.0))
        .child(select::card(Part::Lines))
        .child(
            div()
                .relative()
                .min_w_0()
                .flex()
                .items_center()
                .gap(css(8.0))
                .child(select::line())
                .child(name)
                .child(label),
        )
        .child(
            div()
                .relative()
                .min_w_0()
                .flex()
                .items_center()
                .gap(css(6.0))
                .child(select::line())
                .children(when)
                .child(
                    (small().flex_none().text_color(meta))
                        .child(select::label(&block("dot"), "\u{b7}"))
                        .child("\u{b7}"),
                )
                .child(short),
        );
    let round = |size: f32| {
        div()
            .relative()
            .flex_none()
            .size(css(size))
            .flex()
            .items_center()
            .justify_center()
    };
    // `focus-visible:ring-2 ring-foreground ring-offset-2`; the offset is Tailwind's white.
    let ringed = focus.is_focused(window) && window.last_input_was_keyboard();
    let named = format!("{id}-ring");
    let ring = |inset: f32, color: Hsla| {
        let selector = named.clone();
        div()
            .debug_selector(move || selector)
            .absolute()
            .inset(css(-inset))
            .rounded(css(11.0 + inset))
            .border(css(2.0))
            .border_color(color)
    };
    let rings = ringed.then(|| [ring(3.0, white()), ring(5.0, ink)]);
    let (chip, task) = (id.to_owned(), card.session_id.clone());
    let open = move |window: &mut Window, cx: &mut App| {
        act::task(task.clone(), &chip, (false, from(cx)), window, cx)
    };
    let selector = id.to_owned();
    let el = div()
        .id(element.clone())
        .debug_selector(move || selector)
        .track_focus(&focus)
        .role(Role::Button)
        .relative()
        .w_full()
        .flex()
        .items_center()
        .gap(css(14.0))
        .rounded(css(12.0))
        .border(css(1.0))
        .border_color(edge)
        .bg(fill)
        .p(css(16.0))
        .cursor_pointer()
        .font_weight(weight)
        .child(select::card(Part::Card))
        .child(
            (round(36.0).rounded(css(9999.0)).bg(disc))
                .child(select::card(Part::Disc))
                .child(
                    lucide::icon(lucide::corner_down_right().as_bytes(), 18.0, 2.0)
                        .text_color(glyph),
                ),
        )
        .child(lines)
        .child(
            round(24.0)
                .debug_selector(part("arrow"))
                .child(select::card(Part::Arrow))
                .child(lucide::icon(IconName::ArrowUpRight.svg(), 16.0, 2.0).text_color(arrow)),
        )
        .children(rings.into_iter().flatten());
    // An outlined badge's border makes its line, and the card, 2px taller.
    let taller = if solid { 0.0 } else { 1.0 };
    let metrics = Metrics {
        baseline: BASELINE + taller,
        height: 74.0 + 2.0 * taller,
        least,
        most,
        margin: 8.0,
    };
    let el = press::clicks(el, (&element, &focus), open, window, cx);
    (press::listen(el, &press, false).into_any_element(), metrics)
}
