//! The route's body: nothing, `SessionEntryError` or the view.

use ely_gpui_component::{
    buttons::{Button, ButtonVariant},
    theme::ControlSize,
};
use gpui::{
    AnyElement, App, InteractiveElement as _, IntoElement as _, ParentElement as _, Role,
    StatefulInteractiveElement as _, Styled as _, div,
};

use super::{Failure, Gate};
use crate::{
    appearance_settings::text_size, i18n::Translator, router, theme::Appearance, units::css,
};

/// The view's body, which 9.3c lays out.
pub fn element(cx: &App) -> Option<AnyElement> {
    let failure = match super::gate(cx)? {
        Gate::Waiting => return None,
        Gate::Open => {
            let view = div().debug_selector(|| "session-view".into());
            return Some(view.flex_1().min_h_0().into_any_element());
        }
        Gate::Failed(failure) => failure,
    };
    let t = cx.global::<Translator>();
    let reason = match failure {
        Failure::Missing => "missing",
        Failure::Failed => "failed",
    };
    let appearance = cx.global::<Appearance>();
    let surface = crate::wallpaper::clear(appearance.token("surface"), cx);
    let back = Button::new("session-entry-back", t.t("ccAgent.sessionEntry.back", &[]))
        .variant(ButtonVariant::Secondary)
        .size(ControlSize::Md)
        .on_click(|_, _, cx| router::navigate("/cc-agent", cx));
    let retry = (failure == Failure::Failed).then(|| {
        Button::new(
            "session-entry-retry",
            t.t("ccAgent.sessionEntry.retry", &[]),
        )
        .variant(ButtonVariant::Secondary)
        .size(ControlSize::Md)
        .on_click(|_, _, cx| super::retry(cx))
    });
    let buttons = div()
        .mt(css(16.0))
        .flex()
        .flex_wrap()
        .justify_center()
        .gap(css(8.0))
        .child(
            div()
                .debug_selector(|| "session-entry-back".into())
                .child(back),
        )
        .children(retry.map(|b| {
            div()
                .debug_selector(|| "session-entry-retry".into())
                .child(b)
        }));
    let text = div()
        .debug_selector(move || format!("session-entry-{reason}"))
        .text_size(css(text_size(14.0, cx)))
        .line_height(css(24.0))
        .text_color(appearance.token("text-primary"))
        .child(t.t(&format!("ccAgent.sessionEntry.{reason}"), &[]));
    let section = div()
        .id("session-entry")
        .role(Role::Alert)
        .w_full()
        .max_w(css(448.0))
        .text_center()
        .child(text)
        .child(buttons);
    let main = div()
        .flex()
        .flex_1()
        .min_h_0()
        .items_center()
        .justify_center()
        .bg(surface)
        .p(css(24.0))
        .child(section);
    Some(main.into_any_element())
}
