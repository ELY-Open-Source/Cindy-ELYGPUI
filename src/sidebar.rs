//! Cindy's left sidebar frame: `Sidebar.tsx`, with the width, rail and hidden state
//! `MainLayout.tsx` keeps for it.

mod peek;
mod peek_view;
mod resize;
#[cfg(test)]
mod tests;

use std::time::{Duration, Instant};

use gpui::{
    App, BorrowAppContext, Context, CursorStyle, DispatchPhase, InteractiveElement, IntoElement,
    MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, ParentElement, Render,
    StatefulInteractiveElement, Styled, Window, canvas, div, prelude::FluentBuilder,
};

use crate::units::{self, css};
use crate::{
    chrome,
    cindy_json::js_truthy,
    layout_root::SidebarSeen,
    layout_tree::LayoutStore,
    motion::{self, Tween},
    preferences::Preferences,
    theme::Appearance,
};
use resize::{RAIL_WIDTH, Resize, WIDTH_KEY};

const RAIL_KEY: &str = "sidebar-rail";
const CARD_MODE_KEY: &str = "sidebar.cardMode";
const WIDTH_TRANSITION: Duration = Duration::from_millis(250);
const FADE: Duration = Duration::from_millis(200);
const SESSION_LIST: &str = "session-list";

pub struct Sidebar {
    resize: Resize,
    rail: bool,
    collapsed: bool,
    width: Tween,
    fade: Tween,
    handle_hovered: bool,
    peek: peek::Peek,
    peek_wake: Option<gpui::Task<()>>,
    drawer: Tween,
    trigger: peek_view::TriggerBounds,
    left_peek: bool,
}

fn now(cx: &App) -> Instant {
    cx.background_executor().now()
}

/// `readPanelCollapsed('session-list')`: the tree's flag, truthy as JavaScript reads it.
fn collapsed_in_tree(cx: &App) -> bool {
    let layout = cx.global::<LayoutStore>().layout();
    let panes = layout.walk_panes();
    let pane = panes.iter().find(|p| p.panel_kind == SESSION_LIST);
    js_truthy(pane.and_then(|p| p.collapsed.as_ref()))
}

impl Sidebar {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let prefs = cx.global::<Preferences>();
        let text = |key| {
            prefs.text(key).unwrap_or_else(|e| {
                log::warn!(target: "sidebar", "{e:#}");
                None
            })
        };
        let resize = Resize::new(text(WIDTH_KEY));
        let rail = text(RAIL_KEY) == Some("true");
        let collapsed = collapsed_in_tree(cx);
        log::info!(target: "sidebar", "width {} rail {rail} collapsed {collapsed}", resize.width);
        let at = now(cx);
        let mut sidebar = Sidebar {
            resize,
            rail,
            collapsed,
            width: Tween::still(0.0, at),
            fade: Tween::still(if collapsed { 0.0 } else { 1.0 }, at),
            handle_hovered: false,
            peek: peek::Peek::new(collapsed),
            peek_wake: None,
            drawer: Tween::still(0.0, at),
            trigger: Default::default(),
            left_peek: false,
        };
        sidebar.width = Tween::still(sidebar.target(), at);
        sidebar
    }

    /// `visualWidth`: hidden is 0, a drag follows the pointer, the rail is 78px.
    fn target(&self) -> f32 {
        let width = match () {
            _ if self.collapsed => 0.0,
            _ if self.resize.dragging => self.resize.width,
            _ if self.rail => RAIL_WIDTH,
            _ => self.resize.width,
        };
        width as f32
    }

    /// `contentWidth`: the content keeps its width while the column clips it.
    fn content_width(&self) -> f32 {
        let width = if !self.resize.dragging && self.rail {
            RAIL_WIDTH
        } else {
            self.resize.width
        };
        width as f32
    }

    /// The painted width, which the content area leaves, and `sidebarVisible`
    /// (`!isSidebarCollapsed && !isRailMode`).
    pub fn seen(&self, cx: &App) -> SidebarSeen {
        SidebarSeen {
            width: self.width.at(now(cx)).0,
            visible: !self.collapsed && !self.rail,
            collapsed: self.collapsed,
        }
    }

    /// `handleToggleSidebar`: the tree keeps the hidden state.
    pub fn toggle(&mut self, cx: &mut Context<Self>) {
        self.collapsed = !self.collapsed;
        log::info!(target: "sidebar", "collapsed {}", self.collapsed);
        let collapsed = self.collapsed;
        let was_peeking = self.peek.visible();
        self.peek.set_collapsed(collapsed, now(cx));
        self.after_peek(cx);
        self.left_peek = was_peeking && !self.peek.visible();
        let saved = cx.update_global::<LayoutStore, _>(|store, _| {
            let id = store.layout().sidebar.id.clone();
            store
                .layout()
                .set_pane_collapsed(&id, collapsed)
                .and_then(|next| store.set(next))
        });
        match saved {
            Ok(true) => {}
            Ok(false) => log::warn!(target: "sidebar", "collapse kept in memory only"),
            Err(reason) => log::warn!(target: "sidebar", "collapse not saved: {reason}"),
        }
        self.retarget(cx);
    }

    fn retarget(&mut self, cx: &mut Context<Self>) {
        let at = now(cx);
        // `justLeftPeek`: the drawer's exit frame takes no transition.
        let length = if self.resize.dragging || std::mem::take(&mut self.left_peek) {
            Duration::ZERO
        } else {
            motion::length(WIDTH_TRANSITION, cx)
        };
        self.width.toward(self.target(), at, length);
        self.fade.toward(
            if self.collapsed { 0.0 } else { 1.0 },
            at,
            motion::length(FADE, cx),
        );
        cx.notify();
    }

    fn set_rail(&mut self, on: bool, cx: &mut Context<Self>) {
        self.rail = on;
        let value = if on { "true" } else { "false" };
        if let Err(error) = cx.update_global::<Preferences, _>(|p, _| p.set(RAIL_KEY, value)) {
            log::warn!(target: "sidebar", "{error:#}");
        }
    }

    fn apply(&mut self, effect: resize::Effect, cx: &mut Context<Self>) {
        if let Some(on) = effect.rail {
            self.set_rail(on, cx);
        }
        if let Some(width) = effect.persist {
            let stored = width.to_string();
            if let Err(error) = cx.update_global::<Preferences, _>(|p, _| p.set(WIDTH_KEY, &stored))
            {
                log::warn!(target: "sidebar", "{error:#}");
            }
        }
        self.retarget(cx);
    }

    fn stored_width(cx: &App) -> Option<String> {
        cx.global::<Preferences>()
            .text(WIDTH_KEY)
            .ok()
            .flatten()
            .map(str::to_owned)
    }

    fn handle_down(&mut self, event: &MouseDownEvent, cx: &mut Context<Self>) {
        cx.stop_propagation();
        if event.click_count >= 2 {
            let effect = self.resize.reset(self.rail);
            log::info!(target: "sidebar", "width reset");
            return self.apply(effect, cx);
        }
        self.resize
            .start(f64::from(units::to_css(event.position.x, cx)), self.rail);
        self.retarget(cx);
    }

    fn handle_move(&mut self, x: f32, cx: &mut Context<Self>) {
        let card = cx
            .global::<Preferences>()
            .text(CARD_MODE_KEY)
            .ok()
            .flatten();
        let effect = self
            .resize
            .moved(f64::from(x), matches!(card, Some("card" | "true")));
        self.apply(effect, cx);
    }

    fn handle_up(&mut self, cx: &mut Context<Self>) {
        let effect = self.resize.up(Self::stored_width(cx).as_deref());
        log::info!(target: "sidebar", "width {} rail {}", self.resize.width, self.rail);
        self.apply(effect, cx);
    }
}

impl Render for Sidebar {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let at = now(cx);
        let (width, moving) = self.width.at(at);
        let (opacity, fading) = self.fade.at(at);
        if moving || fading {
            window.request_animation_frame();
        }
        let appearance = cx.global::<Appearance>();
        let line = if self.handle_hovered {
            appearance.hsl_token("sidebar-action-icon")
        } else {
            gpui::transparent_black()
        };
        let handle = div()
            .id("sidebar-resize")
            .absolute()
            .right_0()
            .top_0()
            .h_full()
            .w(css(4.0))
            .cursor(CursorStyle::ResizeLeftRight)
            .on_hover(cx.listener(|this, hovered: &bool, _, cx| {
                this.handle_hovered = *hovered;
                cx.notify();
            }))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, e, _, cx| this.handle_down(e, cx)),
            )
            .child(
                div()
                    .absolute()
                    .right_0()
                    .top_0()
                    .h_full()
                    .w(css(1.0))
                    .bg(line),
            );
        let peeking = self.peek.visible();
        let content_width = if peeking {
            self.resize.width as f32
        } else {
            self.content_width()
        };
        let column = div()
            .flex()
            .flex_col()
            .flex_1()
            .min_h_0()
            .w(css(content_width))
            .opacity(if peeking { 1.0 } else { opacity })
            .child(
                ely_gpui_component::shell::drag_region("sidebar-top-row", window, cx)
                    .flex_none()
                    .w_full()
                    .h(css(chrome::TOP_ROW)),
            )
            .children(crate::sessions::list(cx))
            .into_any_element();
        let (column, drawer) = match peeking {
            true => (div().into_any_element(), self.drawer(column, window, cx)),
            false => (column, self.drawer(div().into_any_element(), window, cx)),
        };
        let appearance = cx.global::<Appearance>();
        let this = cx.entity();
        let dragging = self.resize.dragging;
        div()
            .id("sidebar")
            .debug_selector(|| "sidebar".into())
            .relative()
            .flex_none()
            .flex()
            .flex_col()
            .h_full()
            .w(css(width))
            .when(!peeking, |el| {
                el.overflow_hidden()
                    .bg(crate::window_material::sidebar(false, cx))
            })
            .when(!self.collapsed && !peeking, |el| {
                el.border_r(css(1.0))
                    .border_color(appearance.hsl_token("sidebar-border"))
            })
            .when(!peeking, |el| el.child(column))
            .children(drawer)
            .when(!self.collapsed && !peeking, |el| el.child(handle))
            .when(dragging, |el| {
                el.child(
                    canvas(
                        |_, _, _| {},
                        move |_, _, window, _| {
                            window.set_window_cursor_style(CursorStyle::ResizeLeftRight);
                            let mover = this.clone();
                            window.on_mouse_event(move |e: &MouseMoveEvent, phase, _, cx| {
                                if phase == DispatchPhase::Bubble {
                                    mover.update(cx, |s, cx| {
                                        s.handle_move(units::to_css(e.position.x, cx), cx)
                                    });
                                }
                            });
                            window.on_mouse_event(move |e: &MouseUpEvent, phase, _, cx| {
                                if phase == DispatchPhase::Bubble && e.button == MouseButton::Left {
                                    this.update(cx, |s, cx| s.handle_up(cx));
                                }
                            });
                        },
                    )
                    .absolute()
                    .size_0(),
                )
            })
    }
}
