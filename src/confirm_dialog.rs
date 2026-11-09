//! `ConfirmDialog`'s `standard` presentation, an `AlertDialog` over a dragging scrim.
#![cfg_attr(
    not(test),
    expect(dead_code, reason = "the interrupted-tasks dialog (8.8c3) opens it")
)]

use std::{rc::Rc, time::Instant};

use ely_gpui_component::{
    primitives::{FocusScope, Takeover, give_back, take_focus},
    shell::drag_region,
};
use gpui::{
    AnyElement, AnyWindowHandle, App, ElementId, Entity, FocusHandle, InteractiveElement as _,
    IntoElement, ParentElement, RenderOnce, Role, SharedString, StatefulInteractiveElement as _,
    Styled as _, Subscription, Window, anchored, deferred, div, point, prelude::FluentBuilder as _,
    px, relative,
};

use crate::{appearance_settings::text_size, motion, theme::Appearance, units::css};

mod button;
mod transition;

use button::{Look, Run};

/// Over the fatal screen (`z-[9999]`), under the toasts (`z-[10100]`).
pub const PRIORITY: usize = 4;

/// Where the dialog is in Radix's `Presence`.
#[derive(Clone, Copy, PartialEq)]
enum Phase {
    Closed,
    Open(Instant),
    /// `data-state=closed`: drawn while it fades out.
    Closing(Instant),
}

struct Stage {
    phase: Phase,
    /// The open dialog's focus, handed back once on close.
    takeover: Option<Entity<Takeover>>,
    /// This frame's close, for Escape.
    close: Option<Run>,
    _escape: Option<Subscription>,
}

/// The `standard` presentation, as the interrupted-tasks dialog sets it up.
#[derive(IntoElement)]
pub struct ConfirmDialog {
    id: &'static str,
    open: bool,
    title: SharedString,
    description: Option<SharedString>,
    content: Option<AnyElement>,
    max_width: f32,
    confirm_text: SharedString,
    cancel_text: SharedString,
    confirm_disabled: bool,
    cancel_first: bool,
    auto_focus_confirm: bool,
    on_close: Run,
    on_confirm: Run,
}

impl ConfirmDialog {
    /// `onOpenChange(false)` is `on_close`; the caller keeps rendering it to play the exit.
    pub fn new(
        id: &'static str,
        open: bool,
        (title, confirm_text, cancel_text): (SharedString, SharedString, SharedString),
        on_close: impl Fn(&mut Window, &mut App) + 'static,
        on_confirm: impl Fn(&mut Window, &mut App) + 'static,
    ) -> Self {
        Self {
            id,
            open,
            title,
            description: None,
            content: None,
            max_width: 400.0,
            confirm_text,
            cancel_text,
            confirm_disabled: false,
            cancel_first: false,
            auto_focus_confirm: false,
            on_close: Rc::new(on_close),
            on_confirm: Rc::new(on_confirm),
        }
    }

    pub fn description(mut self, text: impl Into<SharedString>) -> Self {
        self.description = Some(text.into());
        self
    }

    pub fn content(mut self, content: impl IntoElement) -> Self {
        self.content = Some(content.into_any_element());
        self
    }

    pub fn max_width(mut self, width: f32) -> Self {
        self.max_width = width;
        self
    }

    pub fn confirm_disabled(mut self, disabled: bool) -> Self {
        self.confirm_disabled = disabled;
        self
    }

    pub fn cancel_first(mut self) -> Self {
        self.cancel_first = true;
        self
    }

    pub fn auto_focus_confirm(mut self) -> Self {
        self.auto_focus_confirm = true;
        self
    }
}

fn child(id: &'static str, part: &'static str) -> ElementId {
    ElementId::NamedChild(std::sync::Arc::new(id.into()), part.into())
}

/// Escape closes the open dialog in its own window, as `DismissableLayer` does.
fn watch(stage: &Entity<Stage>, here: AnyWindowHandle, cx: &mut App) -> Subscription {
    let stage = stage.downgrade();
    cx.intercept_keystrokes(move |event, window, cx| {
        if event.keystroke.key != "escape" || window.window_handle() != here {
            return;
        }
        let Some(close) = stage.upgrade().and_then(|s| s.read(cx).close.clone()) else {
            return;
        };
        log::info!(target: "confirm-dialog", "closed by Escape");
        close(window, cx);
    })
}

/// Moves to this frame's phase; a close from outside hands focus back too.
fn advance(stage: &Entity<Stage>, open: bool, now: Instant, window: &mut Window, cx: &mut App) {
    let (phase, exit) = (stage.read(cx).phase, motion::length(motion::EXIT, cx));
    let next = match (phase, open) {
        (Phase::Open(_), true) => return,
        (Phase::Closed | Phase::Closing(_), true) => Phase::Open(now),
        (Phase::Open(_), false) => Phase::Closing(now),
        (Phase::Closing(at), false) if now - at < exit => return,
        (Phase::Closing(_), false) => Phase::Closed,
        (Phase::Closed, false) => return,
    };
    let takeover = stage.update(cx, |s, _| {
        s.phase = next;
        s.close = None;
        s._escape = None;
        s.takeover.take()
    });
    if let Some(takeover) = takeover {
        give_back(&takeover, window, cx);
    }
}

impl RenderOnce for ConfirmDialog {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let stage = window.use_keyed_state(child(self.id, "stage"), cx, |_, _| Stage {
            phase: Phase::Closed,
            takeover: None,
            close: None,
            _escape: None,
        });
        let now = cx.background_executor().now();
        advance(&stage, self.open, now, window, cx);
        let phase = stage.read(cx).phase;
        if phase == Phase::Closed {
            return div().into_any_element();
        }
        let keyed = |key: &'static str, window: &mut Window, cx: &mut App| {
            let key = child(self.id, key);
            window
                .use_keyed_state(key, cx, |_, cx| cx.focus_handle().tab_stop(true))
                .read(cx)
                .clone()
        };
        let (confirm_focus, cancel_focus) =
            (keyed("confirm", window, cx), keyed("cancel", window, cx));
        let scope = match phase {
            Phase::Open(_) => Some(open_focus(
                &self,
                &stage,
                (&confirm_focus, &cancel_focus),
                window,
                cx,
            )),
            _ => None,
        };
        let progress = |at: Instant, length| {
            let length = motion::length(length, cx);
            match length.is_zero() {
                true => 1.0,
                false => ((now - at).as_secs_f32() / length.as_secs_f32()).min(1.0),
            }
        };
        let (shown, rise) = match phase {
            Phase::Open(at) => {
                let eased = motion::motion_ease_out(progress(at, motion::ENTER));
                (eased, 1.0 - eased)
            }
            Phase::Closing(at) => (
                1.0 - motion::motion_ease_in(progress(at, motion::EXIT)),
                0.0,
            ),
            Phase::Closed => unreachable!(),
        };
        if shown < 1.0 || matches!(phase, Phase::Closing(_)) {
            window.request_animation_frame();
        }
        let close = stage.read(cx).close.clone();
        let actions = actions(&self, close, (&confirm_focus, &cancel_focus), window, cx);
        let max_width = css(self.max_width);
        let panel = panel(self, rise, window, cx).child(actions);
        let panel = match scope {
            // Radix's `FocusScope trapped`: Tab and Shift-Tab wrap inside.
            Some(scope) => FocusScope::new(&scope)
                .trap()
                .w_full()
                .max_w(max_width)
                .child(panel)
                .into_any_element(),
            None => panel.into_any_element(),
        };
        let viewport = window.viewport_size();
        let scrim = drag_region("confirm-dialog-scrim", window, cx)
            .debug_selector(|| "confirm-dialog-scrim".into())
            .occlude()
            .absolute()
            .inset_0()
            .bg(cx.global::<Appearance>().token("overlay-modal"));
        let layer = div()
            .relative()
            .w(viewport.width)
            .h(viewport.height)
            .opacity(shown)
            .child(scrim)
            .child(
                div()
                    .absolute()
                    .inset_0()
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(panel),
            );
        deferred(anchored().position(point(px(0.0), px(0.0))).child(layer))
            .with_priority(PRIORITY)
            .into_any_element()
    }
}

/// `onOpenAutoFocus`: the confirm if asked and enabled, else Cancel, else the panel.
fn open_focus(
    dialog: &ConfirmDialog,
    stage: &Entity<Stage>,
    (confirm, cancel): (&FocusHandle, &FocusHandle),
    window: &mut Window,
    cx: &mut App,
) -> FocusHandle {
    let takeover = take_focus(child(dialog.id, "focus"), window, cx);
    let scope = takeover.read(cx).focus.clone();
    if stage.read(cx).takeover.is_none() {
        log::info!(target: "confirm-dialog", "{}: opened", dialog.id);
        match (dialog.auto_focus_confirm, dialog.confirm_disabled) {
            (true, false) => window.focus(confirm, cx),
            (true, true) => {}
            (false, _) => window.focus(cancel, cx),
        }
        let here = window.window_handle();
        let escape = watch(stage, here, cx);
        stage.update(cx, |s, _| s._escape = Some(escape));
    }
    let (on_close, closing) = (dialog.on_close.clone(), stage.downgrade());
    let close: Run = Rc::new(move |window, cx| {
        let Some(stage) = closing.upgrade() else {
            return;
        };
        let Some(takeover) = stage.update(cx, |s, _| {
            s.close = None;
            s.takeover.take()
        }) else {
            return;
        };
        give_back(&takeover, window, cx);
        on_close(window, cx);
    });
    stage.update(cx, |s, _| {
        s.takeover = Some(takeover);
        s.close = Some(close);
    });
    scope
}

/// `modal-panel`: at most `maxWidth` by 85vh; the body scrolls.
fn panel(
    dialog: ConfirmDialog,
    rise: f32,
    window: &mut Window,
    cx: &mut App,
) -> gpui::Stateful<gpui::Div> {
    let appearance = cx.global::<Appearance>();
    let max_height = window.viewport_size().height * 0.85;
    let body = (dialog.description.is_some() || dialog.content.is_some()).then(|| {
        let top = if dialog.description.is_some() {
            8.0
        } else {
            12.0
        };
        let has_description = dialog.description.is_some();
        div()
            .id(child(dialog.id, "body"))
            .debug_selector(|| "confirm-dialog-body".into())
            .mt(css(top))
            .flex_shrink(1.0)
            .min_h_0()
            .overflow_y_scroll()
            .when_some(dialog.description, |el, text| {
                el.child(
                    div()
                        .text_size(css(text_size(16.0, cx)))
                        .line_height(css(text_size(24.0, cx)))
                        .text_color(appearance.token("confirm-desc"))
                        .child(text),
                )
            })
            .when_some(dialog.content, |el, content| {
                el.child(
                    div()
                        .when(has_description, |el| el.mt(css(12.0)))
                        .child(content),
                )
            })
    });
    div()
        .id(child(dialog.id, "panel"))
        .debug_selector(|| "confirm-dialog-panel".into())
        .role(Role::AlertDialog)
        .aria_label(dialog.title.clone())
        .occlude()
        .relative()
        .top(css(8.0 * rise))
        .flex()
        .flex_col()
        .w_full()
        .max_w(css(dialog.max_width))
        .max_h(max_height)
        .p(css(16.0))
        .rounded(css(12.0))
        .border(css(1.0))
        .border_color(appearance.token("border-default"))
        .bg(appearance.token("confirm-bg"))
        .shadow(vec![appearance.shadow_token("shadow-menu", cx)])
        .line_height(relative(1.5))
        .child(
            div()
                .flex_none()
                .text_size(css(text_size(18.0, cx)))
                .line_height(css(text_size(28.0, cx)))
                .font_weight(gpui::FontWeight::MEDIUM)
                .text_color(appearance.token("confirm-title"))
                .child(dialog.title),
        )
        .children(body)
}

/// `Cancel` and `Action`: the handler, then the close; inert while it fades.
fn actions(
    dialog: &ConfirmDialog,
    close: Option<Run>,
    (confirm_focus, cancel_focus): (&FocusHandle, &FocusHandle),
    window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    let (on_confirm, confirm_close) = (dialog.on_confirm.clone(), close.clone());
    let confirm = button::button(
        child(dialog.id, "confirm"),
        (dialog.confirm_text.clone(), Look::Primary),
        (confirm_focus, dialog.confirm_disabled),
        Rc::new(move |window, cx| {
            if let Some(close) = &confirm_close {
                log::info!(target: "confirm-dialog", "confirmed");
                on_confirm(window, cx);
                close(window, cx);
            }
        }),
        window,
        cx,
    )
    .debug_selector(|| "confirm-dialog-confirm".into());
    let cancel = button::button(
        child(dialog.id, "cancel"),
        (dialog.cancel_text.clone(), Look::Secondary),
        (cancel_focus, false),
        Rc::new(move |window, cx| {
            if let Some(close) = &close {
                log::info!(target: "confirm-dialog", "cancelled");
                close(window, cx);
            }
        }),
        window,
        cx,
    )
    .debug_selector(|| "confirm-dialog-cancel".into());
    let (first, second) = match dialog.cancel_first {
        true => (cancel, confirm),
        false => (confirm, cancel),
    };
    div()
        .mt(css(24.0))
        .flex_none()
        .flex()
        .flex_wrap()
        .justify_end()
        .gap(css(10.0))
        .child(first)
        .child(second)
        .into_any_element()
}

#[cfg(test)]
mod tests;
