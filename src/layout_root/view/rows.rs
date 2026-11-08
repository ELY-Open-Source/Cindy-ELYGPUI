//! The tree as elements: `NodeView`, the flattened root and the divider line.

use gpui::{
    AnyElement, App, Context, InteractiveElement, IntoElement, MouseButton, MouseDownEvent,
    ParentElement, Styled, Window, div, prelude::FluentBuilder, relative,
};

use super::{CHAT, LayoutRoot, PRESENCE, SeamAt, panel_drag};
use crate::layout_root::geometry::{CHAT_MIN_PX, Ledger};
use crate::layout_tree::{Direction, Layout, Node, Pane, Split};
use crate::theme::Appearance;
use crate::units::css;

impl LayoutRoot {
    /// A pane: its panel paints here once its chapter lands. Its body is a drag root.
    fn pane(&self, pane: &Pane, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let kind = pane.panel_kind.clone();
        let header = (pane.panel_kind == CHAT)
            .then(|| self.content_header(window, cx))
            .flatten();
        div()
            .id(gpui::ElementId::Name(pane.id.clone().into()))
            .relative()
            .size_full()
            .flex()
            .flex_col()
            .when(pane.panel_kind == CHAT, |el| {
                el.bg(cx.global::<Appearance>().hsl_token("content-area"))
            })
            .children(header)
            .child(panel_drag::recorder(&self.rects, true, &pane.panel_kind))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, event: &MouseDownEvent, window, cx| {
                    this.panel_press(
                        &kind,
                        event.position,
                        window.window_handle().window_id(),
                        cx,
                    )
                }),
            )
            .into_any_element()
    }

    /// `NodeView`: a pane, or a split laid out by visible shares with its dividers.
    fn node(&self, node: &Node, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let split = match node {
            Node::Pane(pane) => return self.pane(pane, window, cx),
            Node::Split(split) => split,
        };
        let ledger = Ledger::new(&split.children, &PRESENCE);
        let row = split.direction == Direction::Row;
        let mut items = Vec::new();
        let mut previous: Option<usize> = None;
        for entry in &ledger.mounted {
            if let (Some(first), false) = (previous, entry.parked) {
                let at = SeamAt {
                    split_id: split.id.clone(),
                    first,
                    second: entry.tree_index,
                    scale: ledger.scale,
                    direction: split.direction,
                };
                items.push(if row {
                    static_divider(true, cx)
                } else {
                    self.divider(at, cx)
                });
            }
            let share = self
                .live
                .as_ref()
                .and_then(|l| l.get(entry.node.id()))
                .copied()
                .unwrap_or(entry.share);
            let cell = div()
                .flex()
                .flex_none()
                .min_w_0()
                .min_h_0()
                .overflow_hidden();
            let cell = match (entry.parked, row) {
                (true, true) => cell.h_full().w_0(),
                (true, false) => cell.w_full().h_0(),
                (false, true) => cell.h_full().flex_basis(relative(share as f32)),
                (false, false) => cell.w_full().flex_basis(relative(share as f32)),
            };
            items.push(
                cell.child(self.node(entry.node, window, cx))
                    .into_any_element(),
            );
            if !entry.parked {
                previous = Some(entry.tree_index);
            }
        }
        let el = div().flex().flex_1().min_w_0().min_h_0();
        if row { el.flex_row() } else { el.flex_col() }
            .children(items)
            .into_any_element()
    }

    /// The root split, flattened into the content area. A row's chat flexes and the rest take
    /// their widths; any other root keeps still dividers and lets its panes size themselves.
    pub(super) fn root_row(
        &self,
        layout: &Layout,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        let Node::Split(root) = &layout.content else {
            return vec![self.node(&layout.content, window, cx)];
        };
        let Some(ledger) = Ledger::root(layout, &PRESENCE) else {
            return self.root_other(root, window, cx);
        };
        let widths = self.widths(layout, true);
        let split_id = layout.content.id().to_owned();
        let mut items = Vec::new();
        let mut previous: Option<usize> = None;
        for entry in &ledger.mounted {
            let cell = div().flex().h_full().min_h_0().overflow_hidden();
            if entry.parked {
                items.push(
                    cell.flex_none()
                        .w_0()
                        .min_w_0()
                        .child(self.node(entry.node, window, cx))
                        .into_any_element(),
                );
                continue;
            }
            if let Some(first) = previous {
                let at = SeamAt {
                    split_id: split_id.clone(),
                    first,
                    second: entry.tree_index,
                    scale: ledger.scale,
                    direction: Direction::Row,
                };
                items.push(self.divider(at, cx));
            }
            let cell = match entry.node {
                Node::Split(split) => {
                    cell.relative()
                        .child(panel_drag::recorder(&self.rects, false, &split.id))
                }
                Node::Pane(_) => cell,
            };
            let cell = if entry.node.contains_kind(CHAT) {
                cell.flex_1().min_w(css(CHAT_MIN_PX as f32))
            } else {
                cell.flex_none().min_w_0().w(css(
                    widths.get(entry.node.id()).copied().unwrap_or(0.0) as f32,
                ))
            };
            items.push(
                cell.child(self.node(entry.node, window, cx))
                    .into_any_element(),
            );
            previous = Some(entry.tree_index);
        }
        items
    }

    fn root_other(
        &self,
        root: &Split,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        let ledger = Ledger::new(&root.children, &PRESENCE);
        let mut items = Vec::new();
        let mut previous = false;
        for entry in &ledger.mounted {
            if entry.parked {
                let cell = div()
                    .flex()
                    .flex_none()
                    .h_full()
                    .w_0()
                    .min_w_0()
                    .overflow_hidden();
                items.push(
                    cell.child(self.node(entry.node, window, cx))
                        .into_any_element(),
                );
                continue;
            }
            if previous {
                items.push(static_divider(root.direction == Direction::Row, cx));
            }
            items.push(self.node(entry.node, window, cx));
            previous = true;
        }
        items
    }
}

/// A divider that stays still: nested rows and a root that is not a row, as Cindy's do.
pub(super) fn static_divider(row: bool, cx: &App) -> AnyElement {
    line(row, false, cx).into_any_element()
}

/// `.layout-divider-v` / `-h`: on Cindy's own family the ink lies over the surface.
pub(super) fn line(row: bool, hovered: bool, cx: &App) -> gpui::Div {
    let appearance = cx.global::<Appearance>();
    let line = div().flex_none().relative();
    let line = if row {
        line.w(css(1.0)).h_full()
    } else {
        line.h(css(1.0)).w_full()
    };
    let ink = if hovered {
        appearance.hsl_token("sidebar-action-icon")
    } else {
        appearance.token("border-default")
    };
    let cindy = crate::window_material::cindy_theme(appearance);
    line.when(cindy && !hovered, |l| l.bg(appearance.token("surface")))
        .child(div().absolute().size_full().bg(ink))
}
