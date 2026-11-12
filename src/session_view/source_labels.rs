//! What stands over the bubble: `MessageSourceLabels`' row and the goal badge.

use ely_gpui_component::primitives::IconName;
use gpui::{
    AnyElement, App, ElementId, FontWeight, InteractiveElement as _, IntoElement,
    ParentElement as _, RenderOnce, Role, StatefulInteractiveElement as _, Styled as _, Window,
    div, prelude::FluentBuilder as _, relative,
};

use super::{chip, select};
use crate::{
    appearance_settings::text_size,
    chat_message::merge::{Fields, get},
    i18n::Translator,
    lucide, motion, press,
    rich_answer::js::truthy,
    router, sessions,
    source_label::{self, Icon, Label},
    theme::Appearance,
    tip,
    transition::{self, Curve},
    units::css,
};

/// `MessageSourceLabelShell`: plain words, or a button when a click opens something.
#[derive(IntoElement)]
struct Shell {
    key: String,
    label: Label,
}

impl RenderOnce for Shell {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let Label {
            kind,
            icon,
            text,
            tip,
            open,
        } = self.label;
        let name = format!("{}-source-{kind}", self.key);
        let id = ElementId::Name(name.clone().into());
        let child = |name: &'static str| {
            ElementId::NamedChild(std::sync::Arc::new(id.clone()), name.into())
        };
        let focus = window
            .use_keyed_state(child("focus"), cx, |_, cx| cx.focus_handle())
            .read(cx)
            .clone();
        let (press, state) = press::state(&id, (&focus, false), window, cx);
        let appearance = cx.global::<Appearance>();
        let rest = appearance.token("cmd-palette-item-meta");
        let lit = appearance.hsl_token("foreground");
        let tint = appearance.token("bot-avatar-violet-bg");
        let ink = appearance.token("text-primary");
        let curve = Curve {
            length: motion::length(motion::FAST, cx),
            easing: motion::ease_move,
            lerp: transition::mix_srgb,
        };
        let acts = open.is_some();
        let color = if acts && state.hovered { lit } else { rest };
        let color = transition::keyed(child("color"), color, curve, window, cx);
        let lucide = |name: IconName| {
            lucide::icon(name.svg(), 11.0, 1.75)
                .text_color(color)
                .into_any_element()
        };
        let icon = match icon {
            Icon::Hook => lucide(IconName::MessageSquare),
            Icon::Session => lucide(IconName::Send),
            Icon::Scheduler => lucide(IconName::Timer),
            Icon::Plugin => lucide(IconName::Puzzle),
            Icon::Phone => lucide(IconName::Smartphone),
            Icon::Computer => lucide(IconName::Monitor),
            // `BotAvatar` at `h-3.5 w-3.5 text-10 select-none`, a teammate with no emoji yet.
            Icon::Bot(name) => div()
                .flex()
                .flex_none()
                .items_center()
                .justify_center()
                .overflow_hidden()
                .size(css(14.0))
                .rounded(css(9999.0))
                .bg(tint)
                .text_size(css(text_size(10.0, cx)))
                .line_height(relative(1.0))
                .font_weight(FontWeight::MEDIUM)
                .text_color(ink)
                .child(source_label::initial(&name))
                .relative()
                .child(select::still(9999.0, 9999.0))
                .into_any_element(),
        };
        let (words, text) = (name.clone(), chip::nowrap(&text));
        let el = div()
            .id(id)
            .debug_selector(move || name)
            .flex()
            .items_center()
            .gap(css(4.0))
            .max_w_full()
            .text_size(css(text_size(11.0, cx)))
            .line_height(relative(1.5))
            .text_color(color)
            .child(icon)
            .child(
                div()
                    .relative()
                    .min_w_0()
                    .truncate()
                    .when(!acts, |el| el.child(select::label(&words, &text)))
                    .child(text.clone()),
            )
            .when(acts, |el| el.relative().child(select::still(0.0, 0.0)))
            .when_some(tip, |el, tip| el.tooltip(tip::tip(tip)));
        let Some(route) = open else {
            return el;
        };
        let el = el
            .track_focus(&focus.tab_stop(true))
            .role(Role::Button)
            .cursor_pointer()
            .on_click(move |_, _, cx| {
                log::info!(target: "sessionView", "source label {kind} opens {route}");
                router::navigate(&route, cx);
            });
        press::listen(el, &press, false)
    }
}

/// The form the labels stand over; a hook card names its own channel, so it draws no automation.
#[derive(Clone, Copy, PartialEq)]
pub(super) enum Over {
    Bubble,
    Orca,
    Hook,
}

/// `MessageSourceLabels` over a message: set right over a bubble, left over a card.
pub(super) fn row(key: &str, message: &Fields, over: Over, cx: &App) -> Option<AnyElement> {
    let live = |id: &str| sessions::held_title(id, cx);
    let automation = over != Over::Hook;
    let labels = source_label::labels(message, automation, live, cx.global::<Translator>());
    if labels.is_empty() {
        return None;
    }
    let shells = labels.into_iter().map(|label| Shell {
        key: key.to_owned(),
        label,
    });
    let selector = format!("{key}-sources");
    let row = div()
        .debug_selector(move || selector)
        .flex()
        .flex_wrap()
        .items_center()
        .map(|el| match over {
            Over::Bubble => el.justify_end(),
            Over::Orca | Over::Hook => el.justify_start(),
        })
        .max_w_full()
        .gap_x(css(12.0))
        .gap_y(css(2.0))
        .children(shells);
    Some(row.into_any_element())
}

/// The `/goal` badge: the goal was set, or updated.
pub(super) fn goal(key: &str, message: &Fields, cx: &App) -> Option<AnyElement> {
    let badge = get(message, "goalBadge")?;
    let updated = truthy(badge.obj().and_then(|badge| badge.get("updated")));
    let label = match updated {
        true => "goal.objectiveBadgeUpdated",
        false => "goal.objectiveBadge",
    };
    let label = cx.global::<Translator>().t(label, &[]);
    let appearance = cx.global::<Appearance>();
    let color = appearance.token("text-secondary");
    let selector = format!("{key}-goal");
    let badge = div()
        .debug_selector(move || selector)
        .flex()
        .items_center()
        .gap(css(4.0))
        .max_w_full()
        .rounded(css(9999.0))
        .px(css(8.0))
        .py(css(2.0))
        .text_size(css(text_size(11.0, cx)))
        .line_height(relative(1.5))
        .font_weight(FontWeight::MEDIUM)
        .bg(appearance.token("surface-chip"))
        .text_color(color)
        .child(lucide::icon(IconName::Target.svg(), 11.0, 2.0).text_color(color))
        .child(
            div()
                .relative()
                .min_w_0()
                .truncate()
                .child(select::label(&format!("{key}-goal"), &label))
                .child(label),
        );
    Some(badge.into_any_element())
}

#[cfg(test)]
mod tests;
