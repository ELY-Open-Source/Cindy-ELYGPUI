//! `SessionHistoryButtons` on `ChromeIconButton`: back and forward through the task stack.

use ely_gpui_component::primitives::IconName;
use gpui::{
    AnyElement, App, BoxShadow, ElementId, InteractiveElement as _, IntoElement as _,
    ParentElement as _, Role, Stateful, StatefulInteractiveElement as _, Styled as _, Window, div,
    point, prelude::FluentBuilder as _, px,
};

use crate::{
    app_shortcuts::AppShortcuts,
    browse_history,
    i18n::Translator,
    lucide, motion, press,
    theme::Appearance,
    tip::tip,
    transition::{self, Curve},
    units::{css, window_px},
};

/// Back, then forward; at an end the button greys and its tip says why.
pub fn buttons(window: &mut Window, cx: &mut App) -> [AnyElement; 2] {
    let (back, forward) = browse_history::can(cx);
    [
        button(Hop::Back, back, window, cx),
        button(Hop::Forward, forward, window, cx),
    ]
}

#[derive(Clone, Copy)]
enum Hop {
    Back,
    Forward,
}

impl Hop {
    fn keys(self) -> (&'static str, &'static str, &'static str, isize) {
        match self {
            Hop::Back => (
                "history-back",
                "session-history-back",
                "titleBar.historyBack",
                -1,
            ),
            Hop::Forward => (
                "history-forward",
                "session-history-forward",
                "titleBar.historyForward",
                1,
            ),
        }
    }
}

fn child(id: &ElementId, part: &'static str) -> ElementId {
    ElementId::NamedChild(std::sync::Arc::new(id.clone()), part.into())
}

/// The accessible name and the tip; a disabled button names why.
fn names(hop: Hop, enabled: bool, cx: &App) -> (String, String) {
    let (_, shortcut, label_key, _) = hop.keys();
    let t = cx.global::<Translator>();
    if !enabled {
        let unavailable = t.t(&format!("{label_key}Unavailable"), &[]);
        return (unavailable.clone(), unavailable);
    }
    let label = t.t(label_key, &[]);
    let shown = match cx.global::<AppShortcuts>().display(shortcut) {
        keys if keys.is_empty() => label.clone(),
        keys => format!("{label} ({keys})"),
    };
    (label, shown)
}

fn button(hop: Hop, enabled: bool, window: &mut Window, cx: &mut App) -> AnyElement {
    let (name, _, _, direction) = hop.keys();
    let id = ElementId::Name(name.into());
    let (aria, shown) = names(hop, enabled, cx);
    let focus = window
        .use_keyed_state(child(&id, "focus"), cx, |_, cx| {
            cx.focus_handle().tab_stop(true)
        })
        .read(cx)
        .clone();
    let (press, state) = press::state(&id, (&focus, !enabled), window, cx);
    let appearance = cx.global::<Appearance>();
    let (icon, hover, primary) = (
        appearance.hsl_token("titlebar-icon"),
        appearance.token("surface-hover"),
        appearance.token("text-primary"),
    );
    let ring = BoxShadow {
        color: appearance.token("focus-ring"),
        offset: point(px(0.0), px(0.0)),
        blur_radius: px(0.0),
        spread_radius: window_px(2.0, cx),
        inset: false,
    };
    let lit = state.hovered || state.pressed;
    let fast = motion::length(motion::FAST, cx);
    let face = transition::keyed(
        child(&id, "fill"),
        if lit {
            hover
        } else {
            gpui::transparent_black()
        },
        Curve {
            length: fast,
            easing: motion::ease,
            lerp: transition::mix_srgb,
        },
        window,
        cx,
    );
    let fg = transition::keyed(
        child(&id, "fg"),
        if state.hovered { primary } else { icon },
        Curve {
            length: fast,
            easing: motion::ease_move,
            lerp: transition::mix_srgb,
        },
        window,
        cx,
    );
    // `:active` pulls the face in 1px; reduced motion keeps it put.
    let pressed_in = state.pressed && !cx.global::<motion::ReducedMotion>().0;
    let inset = transition::keyed(
        child(&id, "inset"),
        if pressed_in { 1.0 } else { 0.0 },
        Curve {
            length: motion::length(motion::INSTANT, cx),
            easing: motion::motion_ease_out,
            lerp: transition::lerp,
        },
        window,
        cx,
    );
    let selector = name.to_owned();
    let face_selector = format!("{name}-face");
    let el: Stateful<gpui::Div> = div()
        .id(id)
        .debug_selector(move || selector.clone())
        .track_focus(&focus)
        .role(Role::Button)
        .aria_label(aria)
        .relative()
        .flex()
        .size(css(28.0))
        .items_center()
        .justify_center()
        .rounded(css(9999.0))
        .tooltip(tip(shown))
        .child(
            div()
                .debug_selector(move || face_selector.clone())
                .absolute()
                .inset(css(inset))
                .rounded(css(9999.0))
                .bg(face),
        )
        .child(
            div()
                .relative()
                .flex()
                .when(!enabled, |el| el.opacity(0.4))
                .child(lucide::icon(icon_of(hop).svg(), 15.0, 2.0).text_color(fg)),
        );
    press::listen(el, &press, !enabled)
        .map(|el| match enabled {
            true => el.cursor_pointer().on_click(move |_, _, cx| {
                browse_history::go(direction, cx);
            }),
            // The `span` wrapper's Tab stop and ring; gpui's div has no `aria-disabled`.
            false => el.focus_visible(|s| s.shadow(vec![ring])),
        })
        .into_any_element()
}

fn icon_of(hop: Hop) -> IconName {
    match hop {
        Hop::Back => IconName::ArrowLeft,
        Hop::Forward => IconName::ArrowRight,
    }
}

#[cfg(test)]
mod tests {
    use super::{Hop, names};
    use crate::{app_shortcuts::AppShortcuts, i18n::Translator};

    #[gpui::test]
    fn a_disabled_button_names_its_reason(cx: &mut gpui::TestAppContext) {
        let dir = tempfile::tempdir().unwrap();
        cx.update(|cx| {
            cx.set_global(Translator::new("en").unwrap());
            cx.set_global(AppShortcuts::load(dir.path()));
            let keys = cx.global::<AppShortcuts>().display("session-history-back");
            assert_eq!(
                names(Hop::Back, true, cx),
                (
                    "Back to previous task".into(),
                    format!("Back to previous task ({keys})")
                )
            );
            let off = "No next task".to_owned();
            assert_eq!(names(Hop::Forward, false, cx), (off.clone(), off));
        });
    }
}
