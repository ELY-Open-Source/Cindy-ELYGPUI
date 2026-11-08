//! `ContentHeader.tsx`: the chat column's 46px band that drags the window.

use std::time::Duration;

use ely_gpui_component::shell::drag_region;
use gpui::{
    AnyElement, Context, InteractiveElement, IntoElement, MouseButton, ParentElement, Styled,
    Window, div,
};

use super::LayoutRoot;
use crate::units::css;
use crate::{chrome, motion::Tween, theme::Appearance};

const INSET_TRANSITION: Duration = Duration::from_millis(250);
/// `pl-2`.
const PADDING: f32 = 8.0;
/// `MAC_RIGHT_TOGGLE_WIDTH`: room for the mac right sidebar toggle.
const MAC_RIGHT_TOGGLE: f32 = 36.0;
/// The cluster's width and an 8px gap before the title.
const CLUSTER_ROOM: f32 = chrome::actions::CLUSTER_WIDTH + 8.0;

impl LayoutRoot {
    /// Moves the traffic-light inset toward where this frame wants it.
    pub(super) fn step_header(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let inset =
            cfg!(target_os = "macos") && !window.is_fullscreen() && !(self.sidebar)(cx).visible;
        let now = cx.background_executor().now();
        let target = if inset {
            chrome::TRAFFIC_LIGHT_INSET
        } else {
            PADDING
        };
        let inset = self.header_inset.get_or_insert(Tween::still(target, now));
        inset.toward(target, now, INSET_TRANSITION);
        let mut moving = inset.at(now).1;
        let seen = (self.sidebar)(cx);
        let room = if seen.collapsed || !seen.visible {
            CLUSTER_ROOM
        } else {
            0.0
        };
        let spacer = self.header_spacer.get_or_insert(Tween::still(room, now));
        spacer.toward(room, now, INSET_TRANSITION);
        moving |= spacer.at(now).1;
        if moving {
            window.request_animation_frame();
        }
    }

    /// The header, or none where Cindy hides it: macOS, sidebar shown, nothing injected and no
    /// right sidebar toggle.
    pub(super) fn content_header(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let mac = cfg!(target_os = "macos");
        let sidebar_visible = (self.sidebar)(cx).visible;
        let right_sidebar_available = false;
        if mac && sidebar_visible && !right_sidebar_available {
            return None;
        }
        let now = cx.background_executor().now();
        let stepped = "stepped before the panes render";
        let (padding, _) = self.header_inset.expect(stepped).at(now);
        let (room, _) = self.header_spacer.expect(stepped).at(now);
        let right = match mac {
            true => right_sidebar_available.then_some(MAC_RIGHT_TOGGLE),
            false => Some(chrome::CONTROLS_WIDTH),
        };
        let border = cx.global::<Appearance>().hsl_token("titlebar-border");
        let band = drag_region("content-header", window, cx)
            .debug_selector(|| "content-header".into())
            .relative()
            .flex()
            .flex_none()
            .items_center()
            .w_full()
            .h(css(chrome::TOP_ROW))
            .border_b(css(1.0))
            .border_color(border)
            .pl(css(padding))
            .child(div().flex_none().h_full().w(css(room)))
            .child(
                div()
                    .debug_selector(|| "content-header-slot".into())
                    .flex()
                    .h_full()
                    .min_w_0()
                    .flex_1()
                    .items_center(),
            )
            .children(right.map(|width| div().flex_none().h_full().w(css(width))));
        // The band drags the window; a long press here never drags the panel.
        let held = div()
            .flex_none()
            .w_full()
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation());
        Some(held.child(band).into_any_element())
    }
}
