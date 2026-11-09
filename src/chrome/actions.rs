//! `ChromeActions.tsx`: the cluster beside the traffic lights, over the sidebar and header.

use std::time::{Duration, Instant};

use ely_gpui_component::primitives::IconName;
use gpui::{
    AnyElement, App, Context, Entity, InteractiveElement, IntoElement, ParentElement, Render,
    SharedString, StatefulInteractiveElement, Styled, Window, canvas, deferred, div, svg,
};

use crate::units::css;
use crate::{i18n::Translator, motion::Tween, sidebar::Sidebar, theme::Appearance, tip::tip};

/// `CHROME_ACTIONS_GEOMETRY`.
pub const DEFAULT_LEFT: f32 = 8.0;
pub const MAC_TRAFFIC_LIGHT_LEFT: f32 = 78.0;
pub const CLUSTER_WIDTH: f32 = 124.0;
const SHIFT: Duration = Duration::from_millis(250);

pub struct ChromeActions {
    sidebar: Entity<Sidebar>,
    /// None until the first frame, which takes its place without a slide.
    left: Option<Tween>,
}

impl ChromeActions {
    pub fn new(sidebar: Entity<Sidebar>, cx: &mut Context<Self>) -> Self {
        cx.observe(&sidebar, |_, _, cx| cx.notify()).detach();
        cx.observe_global::<crate::browse_history::History>(|_, cx| cx.notify())
            .detach();
        ChromeActions {
            sidebar,
            left: None,
        }
    }

    /// x 78 beside the mac traffic lights, 8 in mac fullscreen and elsewhere.
    fn step_left(&mut self, window: &mut Window, now: Instant) -> f32 {
        let mac = cfg!(target_os = "macos") && !window.is_fullscreen();
        let target = if mac {
            MAC_TRAFFIC_LIGHT_LEFT
        } else {
            DEFAULT_LEFT
        };
        let left = self.left.get_or_insert(Tween::still(target, now));
        left.toward(target, now, SHIFT);
        let (x, moving) = left.at(now);
        if moving {
            window.request_animation_frame();
        }
        x
    }

    fn toggle(&self, cx: &App) -> AnyElement {
        let appearance = cx.global::<Appearance>();
        let sidebar = self.sidebar.read(cx);
        let collapsed = sidebar.seen(cx).collapsed;
        let label = match collapsed {
            true => "contentHeader.expandSidebar",
            false => "contentHeader.collapseSidebar",
        };
        let label = SharedString::from(cx.global::<Translator>().t(label, &[]));
        let bounds = sidebar.trigger_bounds();
        let (hovered, clicked) = (self.sidebar.clone(), self.sidebar.clone());
        div()
            .id("sidebar-toggle")
            .debug_selector(|| "sidebar-toggle".into())
            .relative()
            .flex()
            .items_center()
            .justify_center()
            .size(css(28.0))
            .rounded_full()
            .hover(|s| s.bg(appearance.hsl_token("titlebar-button-hover")))
            .aria_label(label.clone())
            .aria_expanded(!collapsed)
            .tooltip(tip(label))
            .on_hover(move |hover, _, cx| hovered.update(cx, |s, cx| s.trigger_hover(*hover, cx)))
            .on_click(move |_, _, cx| clicked.update(cx, |s, cx| s.toggle(cx)))
            .child(
                svg()
                    .data(IconName::PanelLeft.svg())
                    .size(css(15.0))
                    .text_color(appearance.hsl_token("titlebar-icon")),
            )
            .child(
                canvas(move |b, _, _| bounds.set(b), |_, _, _, _| {})
                    .absolute()
                    .size_full(),
            )
            .into_any_element()
    }
}

impl Render for ChromeActions {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let left = self.step_left(window, cx.background_executor().now());
        let cluster = div()
            .absolute()
            .top_0()
            .left(css(left))
            .h(css(super::TOP_ROW))
            .flex()
            .items_center()
            .gap_1()
            .child(self.toggle(cx))
            .children(super::history::buttons(window, cx));
        div()
            .absolute()
            .top_0()
            .left_0()
            .child(deferred(cluster).with_priority(2))
    }
}
