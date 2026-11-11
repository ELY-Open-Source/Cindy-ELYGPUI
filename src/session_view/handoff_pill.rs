//! `HandoffSourcePill`: over a task a handoff card opened, the way back to its dispatcher.

use ely_gpui_component::primitives::IconName;
use gpui::{
    AnyElement, App, Div, ElementId, FocusHandle, Hsla, InteractiveElement as _, IntoElement as _,
    ParentElement as _, Role, Stateful, StatefulInteractiveElement as _, Styled as _, Window, div,
};

use super::{SessionView, chip::nowrap, link_chip::act};
use crate::{
    appearance_settings::text_size,
    handoff_card as words,
    i18n::Translator,
    lucide, motion, press, router,
    theme::Appearance,
    transition::{self, Curve},
    units::css,
};

/// The route's handoff, until this content's pill is dismissed.
fn shown(cx: &App) -> Option<router::Handoff> {
    let view = cx.global::<SessionView>().shown.as_ref()?;
    let content = view.content.as_ref()?;
    router::handoff(cx).filter(|_| !content.dismissed).cloned()
}

/// `setHandoffPillDismissed(true)`.
fn dismiss(cx: &mut App) {
    let shown = cx.global_mut::<SessionView>().shown.as_mut();
    if let Some(content) = shown.and_then(|shown| shown.content.as_mut()) {
        log::info!(target: "sessionView", "handoff pill dismissed");
        content.dismissed = true;
    }
}

/// `transition-colors`, at the default duration `--motion-fast`.
fn curve(cx: &App) -> Curve<Hsla> {
    Curve {
        length: motion::length(motion::FAST, cx),
        easing: motion::ease_move,
        lerp: transition::mix_srgb,
    }
}

/// One of the pill's buttons: its Tab stop, hover and ring, and the color its hover eases to.
struct Button {
    id: ElementId,
    focus: FocusHandle,
    press: gpui::Entity<press::Press>,
    hovered: bool,
    ink: Hsla,
    ringed: bool,
}

fn button(
    name: &'static str,
    (rest, lit): (Hsla, Hsla),
    window: &mut Window,
    cx: &mut App,
) -> Button {
    let id = ElementId::Name(name.into());
    let child =
        |part: &'static str| ElementId::NamedChild(std::sync::Arc::new(id.clone()), part.into());
    let focus =
        window.use_keyed_state(child("focus"), cx, |_, cx| cx.focus_handle().tab_stop(true));
    let focus = focus.read(cx).clone();
    let (press, state) = press::state(&id, (&focus, false), window, cx);
    let curve = curve(cx);
    let ink = if state.hovered { lit } else { rest };
    Button {
        ink: transition::keyed(child("ink"), ink, curve, window, cx),
        ringed: focus.is_focused(window) && window.last_input_was_keyboard(),
        hovered: state.hovered,
        id,
        focus,
        press,
    }
}

impl Button {
    /// The button on `el`; `focus-visible:ring-2 ring-foreground` stands 2px past its box.
    fn element(
        self,
        el: Div,
        (radius, lit): (f32, Hsla),
        open: impl Fn(&mut Window, &mut App) + Clone + 'static,
        window: &mut Window,
        cx: &mut App,
    ) -> Stateful<Div> {
        let name = format!("{}-ring", self.id);
        let ring = self.ringed.then(|| {
            div()
                .debug_selector(move || name)
                .absolute()
                .inset(css(-2.0))
                .rounded(css(radius + if radius > 0.0 { 2.0 } else { 0.0 }))
                .border(css(2.0))
                .border_color(lit)
        });
        let name = self.id.to_string();
        let el = el
            .id(self.id.clone())
            .debug_selector(move || name)
            .track_focus(&self.focus)
            .role(Role::Button)
            .relative()
            .cursor_pointer()
            .text_color(self.ink)
            .children(ring);
        let el = press::clicks(el, (&self.id, &self.focus), open, window, cx);
        press::listen(el, &self.press, false)
    }
}

/// The pill's row, `shrink-0 px-4 pt-3`, at most `width` wide; none without a handoff.
pub fn element(width: f32, window: &mut Window, cx: &mut App) -> Option<AnyElement> {
    let from = shown(cx)?;
    let t = cx.global::<Translator>();
    // `truncate` is `nowrap`: the line's white space collapses.
    let line = nowrap(&words::pill(&from.dispatcher, from.title.as_deref(), t));
    let aria = words::dismiss(t);
    let a = cx.global::<Appearance>();
    let (muted, ink) = (a.hsl_token("muted-foreground"), a.hsl_token("foreground"));
    let (edge, fill, lit) = (
        a.token("cmd-palette-border"),
        a.hsl_token("content-area"),
        a.token("cmd-palette-bg"),
    );
    let size = text_size(13.0, cx);
    let back = button("handoff-pill-return", (muted, ink), window, cx);
    let close = button("handoff-pill-dismiss", (muted, ink), window, cx);
    let glyph = back.ink;
    let face =
        div()
            .h_full()
            .min_w_0()
            .flex_1()
            .flex()
            .items_center()
            .gap(css(8.0))
            .px(css(12.0))
            .child(div().flex_none().child(
                lucide::icon(lucide::corner_up_left().as_bytes(), 14.0, 2.0).text_color(glyph),
            ))
            .child(
                div()
                    .debug_selector(|| "handoff-pill-line".into())
                    .min_w_0()
                    .truncate()
                    .child(line),
            );
    let dispatcher = from.dispatcher.clone();
    let back = back.element(
        face,
        (0.0, ink),
        move |_, cx| act::back(dispatcher.clone(), cx),
        window,
        cx,
    );
    // `hover:bg-[var(--cmd-palette-bg)]` eases in from the transparent rest.
    let wash = if close.hovered { lit } else { lit.opacity(0.0) };
    let name = ElementId::Name("handoff-pill-dismiss-fill".into());
    let (cross, over) = (
        close.ink,
        transition::keyed(name, wash, curve(cx), window, cx),
    );
    let face = div()
        .mr(css(8.0))
        .flex_none()
        .size(css(28.0))
        .flex()
        .items_center()
        .justify_center()
        .rounded(css(8.0))
        .bg(over)
        .child(lucide::icon(IconName::X.svg(), 14.0, 2.0).text_color(cross));
    let close = close
        .element(face, (8.0, ink), |_, cx| dismiss(cx), window, cx)
        .aria_label(aria);
    let pill = div()
        .debug_selector(|| "handoff-pill".into())
        .flex()
        .h(css(40.0))
        .w_full()
        .items_center()
        .rounded(css(12.0))
        .border(css(1.0))
        .border_color(edge)
        .bg(fill)
        .text_size(css(size))
        .line_height(css(size))
        .child(back)
        .child(close);
    let row = div()
        .flex_none()
        .px(css(16.0))
        .pt(css(12.0))
        .child(div().mx_auto().w_full().max_w(css(width)).child(pill));
    Some(row.into_any_element())
}

#[cfg(test)]
mod tests;
