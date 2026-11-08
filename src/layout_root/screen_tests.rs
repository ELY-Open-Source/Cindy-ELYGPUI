//! The view driven by mouse events, as `rootDividerDrag.test.tsx` and `LayoutRoot.test.tsx` do.

use std::time::Duration;

use gpui::{
    Modifiers, MouseButton, MouseDownEvent, MouseUpEvent, TestAppContext, VisualTestContext,
    WindowAppearance, point, px, size,
};
use serde_json::json;

use super::{LayoutRoot, SidebarSeen};
use crate::layout_tree::{Layout, LayoutStore, Node};
use crate::{os_appearance::Initial, theme::Appearance, theme::Mode};

pub(super) fn tree(chat: f64, right: f64) -> Layout {
    tree_in("row", chat, right)
}

pub(super) fn tree_in(direction: &str, chat: f64, right: f64) -> Layout {
    let raw = json!({
        "schemaVersion": 1,
        "sidebar": { "type": "pane", "id": "sessions", "panelKind": "session-list", "edge": "left" },
        "content": { "type": "split", "id": "root", "direction": direction, "children": [
            { "fraction": chat, "node": { "type": "pane", "id": "chat", "panelKind": "chat-main" } },
            { "fraction": right, "node": { "type": "pane", "id": "right", "panelKind": "right-tabs" } },
        ] },
        "float": [],
    });
    Layout::coerce(Some(&raw)).0
}

/// A window `width` wide holding only the content area, over a store in `dir`.
pub(super) fn open<'a>(
    cx: &'a mut TestAppContext,
    dir: &std::path::Path,
    layout: Layout,
    width: f32,
) -> &'a mut VisualTestContext {
    open_beside(cx, dir, layout, width, 0.0)
}

/// The content area right of a `sidebar` wide column, as the main window lays it out.
struct Row {
    sidebar: f32,
    content: gpui::Entity<LayoutRoot>,
}

impl gpui::Render for Row {
    fn render(
        &mut self,
        _: &mut gpui::Window,
        _: &mut gpui::Context<Self>,
    ) -> impl gpui::IntoElement {
        use gpui::{ParentElement as _, Styled as _};
        gpui::div()
            .size_full()
            .flex()
            .child(gpui::div().flex_none().w(px(self.sidebar)))
            .child(self.content.clone())
    }
}

pub(super) fn open_beside<'a>(
    cx: &'a mut TestAppContext,
    dir: &std::path::Path,
    layout: Layout,
    width: f32,
    sidebar: f32,
) -> &'a mut VisualTestContext {
    cx.update(|cx| {
        let initial = Initial {
            os: WindowAppearance::Light,
            gpui_agrees: true,
        };
        cx.set_global(Appearance::new(Mode::System, None, initial, None).unwrap());
        cx.set_global(crate::i18n::Translator::new("en").unwrap());
        cx.set_global(crate::toast::Toasts::default());
        let mut store = LayoutStore::open(dir);
        store.set(layout).unwrap();
        cx.set_global(store);
    });
    let (_, vcx) = cx.add_window_view(|_, cx| {
        use gpui::AppContext as _;
        Row {
            sidebar,
            content: cx.new(|cx| {
                LayoutRoot::new(
                    Box::new(move |_| SidebarSeen {
                        width: sidebar,
                        visible: sidebar > 0.0,
                    }),
                    cx,
                )
            }),
        }
    });
    vcx.simulate_resize(size(px(width), px(900.0)));
    vcx.run_until_parked();
    vcx
}

pub(super) fn fractions(cx: &mut VisualTestContext) -> (Vec<f64>, Option<f64>) {
    cx.update(|_, cx| {
        let Node::Split(split) = &cx.global::<LayoutStore>().layout().content else {
            panic!("row")
        };
        let Node::Pane(right) = &split.children[1].node else {
            panic!("pane")
        };
        (
            split.children.iter().map(|c| c.fraction).collect(),
            right.preferred_width,
        )
    })
}

pub(super) fn saved(dir: &std::path::Path) -> serde_json::Value {
    serde_json::from_str(&std::fs::read_to_string(dir.join("layout.v1.json")).unwrap()).unwrap()
}

#[gpui::test]
fn a_divider_drag_saves_once_on_release(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let cx = open(cx, dir.path(), tree(0.5, 0.5), 1660.0);
    let none = Modifiers::none();
    let seam = point(px(829.5), px(400.0));
    cx.simulate_mouse_down(seam, MouseButton::Left, none);
    cx.simulate_mouse_move(point(px(829.5 - 166.0), px(400.0)), MouseButton::Left, none);
    assert_eq!(
        fractions(cx).0,
        vec![0.5, 0.5],
        "nothing is written while dragging"
    );
    cx.simulate_mouse_up(point(px(829.5 - 166.0), px(400.0)), MouseButton::Left, none);
    let (after, preferred) = fractions(cx);
    assert!(
        (after[0] - 0.4).abs() < 1e-6 && (after[1] - 0.6).abs() < 1e-6,
        "{after:?}"
    );
    assert_eq!(preferred, Some(996.0));
    assert_eq!(
        saved(dir.path())["content"]["children"][1]["node"]["preferredWidth"],
        996
    );

    cx.simulate_mouse_move(point(px(0.0), px(400.0)), None, none);
    cx.run_until_parked();
    assert_eq!(
        fractions(cx).0,
        after,
        "a move after release changes nothing"
    );
}

#[gpui::test]
fn a_double_click_evens_the_neighbors(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let cx = open(cx, dir.path(), tree(0.7, 0.3), 1660.0);
    let at = point(px(1660.0 * 0.7 - 0.5), px(400.0));
    let modifiers = Modifiers::none();
    cx.simulate_event(MouseDownEvent {
        position: at,
        modifiers,
        button: MouseButton::Left,
        click_count: 2,
        first_mouse: false,
    });
    cx.simulate_event(MouseUpEvent {
        position: at,
        modifiers,
        button: MouseButton::Left,
        click_count: 2,
    });
    let (after, preferred) = fractions(cx);
    assert!((after[0] - 0.5).abs() < 1e-9, "{after:?}");
    assert_eq!(preferred, Some(830.0));
}

#[gpui::test]
fn a_starved_pane_heals_after_the_width_settles(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let cx = open(cx, dir.path(), tree(0.9, 0.1), 800.0);
    cx.executor().advance_clock(Duration::from_millis(119));
    assert_eq!(fractions(cx).0, vec![0.9, 0.1]);
    cx.executor().advance_clock(Duration::from_millis(1));
    cx.run_until_parked();
    let after = fractions(cx).0;
    assert!((after[1] * 800.0 - 120.0).abs() < 1e-6, "{after:?}");
}

#[gpui::test]
fn self_heal_waits_while_a_divider_is_held(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let cx = open(cx, dir.path(), tree(0.9, 0.1), 800.0);
    let seam = point(px(679.5), px(400.0));
    cx.simulate_mouse_down(seam, MouseButton::Left, Modifiers::none());
    cx.executor().advance_clock(Duration::from_millis(500));
    cx.run_until_parked();
    assert_eq!(fractions(cx).0, vec![0.9, 0.1]);
    cx.simulate_mouse_up(seam, MouseButton::Left, Modifiers::none());
    cx.executor().advance_clock(Duration::from_millis(120));
    cx.run_until_parked();
    assert!((fractions(cx).0[1] * 800.0 - 120.0).abs() < 1e-6);
}

#[gpui::test]
fn a_root_that_is_not_a_row_has_still_dividers(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let cx = open(cx, dir.path(), tree_in("column", 0.5, 0.5), 1660.0);
    let none = Modifiers::none();
    for y in (0..900).step_by(2) {
        let (from, to) = (
            point(px(100.0), px(y as f32)),
            point(px(100.0), px(y as f32 + 50.0)),
        );
        cx.simulate_mouse_down(from, MouseButton::Left, none);
        cx.simulate_mouse_move(to, MouseButton::Left, none);
        cx.simulate_mouse_up(to, MouseButton::Left, none);
    }
    assert_eq!(fractions(cx).0, vec![0.5, 0.5]);
}
