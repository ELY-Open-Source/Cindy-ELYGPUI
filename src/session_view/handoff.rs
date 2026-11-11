//! `SessionHandoffCard`: a session-card link's block in an assistant message.

use std::time::Duration;

use ely_gpui_component::{primitives::IconName, theme::ActiveTheme as _};
use gpui::{
    AnyElement, App, ClickEvent, Div, ElementId, FocusHandle, FontWeight, Hsla,
    InteractiveElement as _, IntoElement as _, KeyboardButton, ParentElement as _, Role,
    StatefulInteractiveElement as _, Styled as _, Window, div, white,
};

use super::link_chip::act;
use crate::{
    appearance_settings::text_size,
    deep_link::Card,
    handoff_card as words,
    i18n::Translator,
    js_date, lucide, motion, press,
    theme::Appearance,
    transition::{self, Curve},
    units::css,
};

/// The card's first baseline under its top: the bottom of the icon in its 36px circle.
const BASELINE: f32 = 46.0;

/// A Space held on the card: a native button clicks when it lifts, unless something ended it.
#[derive(Clone, Copy, Default)]
struct Space {
    held: bool,
    /// Another key or a modifier came with it, so gpui's own key-up click will not come.
    mixed: bool,
}

/// `tracking-[0.4px]`: each character as wide as it is shaped, and 0.4px more.
fn tracked(label: &str, size: f32, weight: FontWeight, window: &Window) -> Div {
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
    let letters = label.chars().zip(starts.zip(ends)).map(|(c, (from, to))| {
        div()
            .flex_none()
            .w(to - from + css(0.4).to_pixels(rem))
            .child(c.to_string())
    });
    div().flex().children(letters)
}

/// The card mounted as `id`, and its first baseline in CSS pixels; `weight` is what it inherits.
pub fn element(
    id: &str,
    card: &Card,
    weight: FontWeight,
    focus: FocusHandle,
    window: &mut Window,
    cx: &mut App,
) -> (AnyElement, f32) {
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
    let label = tracked(&badge.to_uppercase(), sizes[0], FontWeight::MEDIUM, window)
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
        .min_w_0()
        .truncate()
        .text_size(css(sizes[3]))
        .font_weight(FontWeight::SEMIBOLD)
        .line_height(css(20.0))
        .text_color(text)
        .child(title);
    let small = || div().text_size(css(sizes[2])).line_height(css(16.0));
    let when = match seen {
        Some(seen) => vec![
            (div().debug_selector(part("history")).flex_none())
                .child(lucide::icon(lucide::history().as_bytes(), 12.0, 2.0).text_color(desc))
                .into_any_element(),
            (small().min_w_0().truncate().text_color(desc).child(seen)).into_any_element(),
        ],
        None => vec![
            (small().min_w_0().truncate().italic().text_color(meta))
                .child(never)
                .into_any_element(),
        ],
    };
    let short = div()
        .debug_selector(part("short"))
        .flex_none()
        .font_family(mono)
        .text_size(css(sizes[1]))
        .line_height(css(16.0))
        .text_color(meta)
        .child(words::short(&card.session_id));
    let lines = div()
        .min_w_0()
        .flex_1()
        .flex()
        .flex_col()
        .gap(css(4.0))
        .child(
            div()
                .min_w_0()
                .flex()
                .items_center()
                .gap(css(8.0))
                .child(name)
                .child(label),
        )
        .child(
            div()
                .min_w_0()
                .flex()
                .items_center()
                .gap(css(6.0))
                .children(when)
                .child(small().flex_none().text_color(meta).child("\u{b7}"))
                .child(short),
        );
    let round = |size: f32| {
        div()
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
    let space = window.use_keyed_state(child("space"), cx, |_, _| Space::default());
    if !focus.is_focused(window) {
        space.update(cx, |space, _| *space = Space::default());
    }
    let (chip, task) = (id.to_owned(), card.session_id.clone());
    // Every activation ends a held Space, as a native button's does.
    let open = {
        let space = space.clone();
        move |window: &mut Window, cx: &mut App| {
            space.update(cx, |space, _| *space = Space::default());
            act::task(task.clone(), &chip, false, window, cx)
        }
    };
    let (clicked, entered, released) = (open.clone(), open.clone(), open);
    let (down, up, clean) = (space.clone(), space.clone(), space);
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
        // gpui's key-up click tells a clean Space from one the focus left meanwhile.
        .on_click(move |event, window, cx| {
            let fresh = match event {
                ClickEvent::Mouse(_) => true,
                ClickEvent::Keyboard(key) => {
                    key.button == KeyboardButton::Space && clean.read(cx).held
                }
                _ => false,
            };
            if fresh {
                clicked(window, cx);
            }
        })
        .on_key_down(move |event, window, cx| {
            let stroke = &event.keystroke;
            match stroke.key.as_str() {
                "enter" => entered(window, cx),
                "space" => down.update(cx, |space, _| {
                    space.mixed |= stroke.modifiers.modified();
                    space.held = true;
                }),
                _ => down.update(cx, |space, _| space.mixed = space.held),
            }
        })
        .on_key_up(move |event, window, cx| {
            let stroke = &event.keystroke;
            let Space { held, mixed } = *up.read(cx);
            if stroke.key != "space" {
                return up.update(cx, |space, _| space.mixed = space.held);
            }
            if held && (mixed || stroke.modifiers.modified()) {
                return released(window, cx);
            }
            // The clean release is gpui's click, in this same dispatch.
            let up = up.clone();
            cx.defer(move |cx| up.update(cx, |space, _| *space = Space::default()));
        })
        .child(round(36.0).rounded(css(9999.0)).bg(disc).child(
            lucide::icon(lucide::corner_down_right().as_bytes(), 18.0, 2.0).text_color(glyph),
        ))
        .child(lines)
        .child(
            round(24.0)
                .debug_selector(part("arrow"))
                .child(lucide::icon(IconName::ArrowUpRight.svg(), 16.0, 2.0).text_color(arrow)),
        )
        .children(rings.into_iter().flatten());
    // An outlined badge's border makes its line, and the card, 2px taller.
    let baseline = BASELINE + if solid { 0.0 } else { 1.0 };
    (
        press::listen(el, &press, false).into_any_element(),
        baseline,
    )
}
