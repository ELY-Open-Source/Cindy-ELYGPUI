//! The peek as `MainLayout.tsx` and `Sidebar.tsx` drive it: the toggle's hover, the drawer.

use std::{cell::Cell, rc::Rc, time::Duration};

use gpui::{
    AnyElement, Bounds, Context, DispatchPhase, InteractiveElement, IntoElement, MouseMoveEvent,
    ParentElement, Pixels, Styled, Window, canvas, deferred, div, px,
};

use super::{Sidebar, now, peek::State};
use crate::{motion, theme::Appearance};

const SLIDE: Duration = Duration::from_millis(200);

/// Where the toggle button painted, so a pointer over it keeps the peek.
pub type TriggerBounds = Rc<Cell<Bounds<Pixels>>>;

impl Sidebar {
    pub fn trigger_bounds(&self) -> TriggerBounds {
        self.trigger.clone()
    }

    /// The toggle's hover: entering starts the 120ms intent, leaving the grace.
    pub fn trigger_hover(&mut self, hovered: bool, cx: &mut Context<Self>) {
        let at = now(cx);
        if hovered {
            self.peek.trigger_enter(at);
        } else {
            self.peek.trigger_leave(at);
        }
        self.after_peek(cx);
    }

    /// The window lost focus.
    pub fn blur(&mut self, cx: &mut Context<Self>) {
        self.peek.blur(now(cx));
        self.after_peek(cx);
    }

    fn pointer_moved(&mut self, event: &MouseMoveEvent, cx: &mut Context<Self>) {
        let over_trigger = self.trigger.get().contains(&event.position);
        let over_drawer = self.peek.visible() && event.position.x < px(self.resize.width as f32);
        self.peek
            .pointer_moved(over_drawer || over_trigger, over_trigger, now(cx));
        self.after_peek(cx);
    }

    /// Wakes on the next timer, moves the drawer, and leaves the rail on a pin.
    pub(super) fn after_peek(&mut self, cx: &mut Context<Self>) {
        let at = now(cx);
        let before = self.peek.state;
        self.peek.tick(at);
        match self.peek.state {
            State::Peeking => self.drawer.toward_along(1.0, at, SLIDE, motion::peek_in),
            State::PeekClosing => self.drawer.toward(0.0, at, SLIDE),
            State::Pinning => {
                self.drawer = motion::Tween::still(1.0, at);
                if self.rail {
                    self.set_rail(false, cx);
                }
            }
            State::Idle => self.drawer = motion::Tween::still(0.0, at),
        }
        if before != self.peek.state {
            log::debug!(target: "sidebar", "peek {before:?} -> {:?}", self.peek.state);
        }
        self.peek_wake = self.peek.next().map(|due| {
            cx.spawn(async move |this, cx| {
                let wait = due.saturating_duration_since(cx.background_executor().now());
                cx.background_executor().timer(wait).await;
                if let Err(error) = this.update(cx, |this, cx| this.after_peek(cx)) {
                    log::debug!(target: "sidebar", "sidebar gone before the peek timer: {error}");
                }
            })
        });
        cx.notify();
    }

    /// The drawer over the content, with the window-wide move listener the peek needs.
    pub(super) fn drawer(
        &self,
        column: AnyElement,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let listen = self.peek.listening().then(|| {
            let this = cx.entity();
            canvas(
                |_, _, _| {},
                move |_, _, window, _| {
                    window.on_mouse_event(move |e: &MouseMoveEvent, phase, _, cx| {
                        if phase == DispatchPhase::Bubble {
                            this.update(cx, |s, cx| s.pointer_moved(e, cx));
                        }
                    });
                },
            )
            .absolute()
            .size_0()
            .into_any_element()
        });
        if !self.peek.visible() {
            return listen;
        }
        let (shown, moving) = self.drawer.at(now(cx));
        if moving {
            window.request_animation_frame();
        }
        let width = self.resize.width as f32;
        let closing = self.peek.state == State::PeekClosing;
        let appearance = cx.global::<Appearance>();
        let drawer = div()
            .debug_selector(|| "sidebar-peek-drawer".into())
            .absolute()
            .top_0()
            .left(px(-width * (1.0 - shown)))
            .h(window.viewport_size().height)
            .w(px(width))
            .flex()
            .flex_col()
            .overflow_hidden()
            .bg(appearance.hsl_token("sidebar"))
            .border_r_1()
            .border_color(appearance.hsl_token("sidebar-border"))
            .shadow(vec![appearance.shadow_token("shadow-menu")])
            .opacity(if closing { shown } else { 1.0 })
            .child(column);
        let layer = div().absolute().top_0().left_0().children(listen);
        Some(
            layer
                .child(deferred(drawer).with_priority(1))
                .into_any_element(),
        )
    }
}
