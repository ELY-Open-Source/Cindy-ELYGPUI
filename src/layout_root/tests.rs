//! Cases from Cindy's `rootDividerDrag.test.tsx`, `normalizeSubMinFractions.test.tsx` and
//! `LayoutRoot.test.tsx`, with the widths they mock as `measured`.

use std::collections::HashMap;

use serde_json::{Value, json};

use super::divider::{ColumnDrag, RootDrag, equalize_column, equalize_root};
use super::geometry::{Ledger, Presence, Seam, normalize_sub_min_fractions, root_widths};
use crate::layout_tree::{Layout, Node};

const AVAIL: f64 = 1660.0;

fn builtin(kind: &str) -> bool {
    matches!(kind, "session-list" | "chat-main" | "right-tabs")
}

fn never(_: &str) -> bool {
    false
}

fn tree(children: Value) -> Layout {
    let raw = json!({
        "schemaVersion": 1,
        "sidebar": { "type": "pane", "id": "sessions", "panelKind": "session-list", "edge": "left" },
        "content": { "type": "split", "id": "root", "direction": "row", "children": children },
        "float": [],
    });
    let (layout, fallback) = Layout::coerce(Some(&raw));
    assert_eq!(fallback, None);
    layout
}

fn pane(fraction: f64, id: &str, kind: &str) -> Value {
    json!({ "fraction": fraction, "node": { "type": "pane", "id": id, "panelKind": kind } })
}

fn residue() -> Layout {
    tree(json!([
        pane(0.22, "ghost-project-opener", "ghost:project-opener"),
        pane(0.4589135021784424, "chat", "chat-main"),
        pane(0.32108649782155757, "right", "right-tabs"),
    ]))
}

fn fractions(layout: &Layout) -> Vec<f64> {
    let Node::Split(split) = &layout.content else {
        panic!("row split")
    };
    split.children.iter().map(|c| c.fraction).collect()
}

fn close(a: f64, b: f64, digits: i32) {
    assert!((a - b).abs() < 0.5 * 10f64.powi(-digits), "{a} != {b}");
}

/// A drag of `dx` pixels on root divider `seam`, released; jsdom measures nothing.
fn drag(
    layout: &Layout,
    presence: &Presence,
    seam: usize,
    dx: f64,
    measured: &dyn Fn(&Node) -> Option<f64>,
) -> Layout {
    let ledger = Ledger::root(layout, presence).unwrap();
    let entries: Vec<_> = ledger.entries().collect();
    let seam = Seam {
        split_id: "root",
        left: entries[seam].tree_index,
        right: entries[seam + 1].tree_index,
        scale: ledger.scale,
    };
    let drag = RootDrag::start(layout, presence, seam, AVAIL, false, measured).unwrap();
    drag.commit(layout, drag.delta(dx), presence, AVAIL)
        .expect("the tree takes the drag")
}

fn by_kind(widths: &[(&'static str, f64)]) -> impl Fn(&Node) -> Option<f64> {
    let widths: HashMap<_, _> = widths.iter().copied().collect();
    move |node| match node {
        Node::Pane(pane) => widths.get(pane.panel_kind.as_str()).copied(),
        Node::Split(_) => None,
    }
}

#[test]
fn drags_on_visible_shares() {
    let presence = Presence {
        mounted: &builtin,
        parked: &never,
    };
    let unmeasured = |_: &Node| None;
    let painted = |layout: &Layout| root_widths(layout, AVAIL, None, &presence, &never);

    let clean = tree(json!([
        pane(0.5, "chat", "chat-main"),
        pane(0.5, "right", "right-tabs")
    ]));
    let widths = painted(&clean);
    let measured = move |n: &Node| widths.get(n.id()).copied();
    let moved = drag(&clean, &presence, 0, -AVAIL * 0.1, &measured);
    close(fractions(&moved)[0], 0.4, 6);
    close(fractions(&moved)[1], 0.6, 6);

    let residue = residue();
    close(painted(&residue)["right"], 683.0, 0);
    let after = fractions(&drag(&residue, &presence, 0, -1000.0, &unmeasured));
    let scale = after[1] + after[2];
    close(after[1] / scale * AVAIL, 400.0, 0);
    close(after[2] / scale * AVAIL, AVAIL - 400.0, 0);
    assert_eq!(after[0], 0.22);
    close(after.iter().sum(), 1.0, 6);
    let after = fractions(&drag(&residue, &presence, 0, 1000.0, &unmeasured));
    close(after[2] / (after[1] + after[2]) * AVAIL, 120.0, 0);

    let after = fractions(&drag(&clean, &presence, 0, -1000.0, &unmeasured));
    close(after[0] * AVAIL, 400.0, 0);

    let squeezed = tree(json!([
        pane(0.6, "chat", "chat-main"),
        pane(0.4, "right", "right-tabs")
    ]));
    let collapsed = by_kind(&[("chat-main", 1200.0), ("right-tabs", 0.0)]);
    let after = fractions(&drag(&squeezed, &presence, 0, -1000.0, &collapsed));
    close(after[0], 0.6 - 800.0 / AVAIL, 3);
}

#[test]
fn a_side_gives_from_its_painted_width() {
    let ghosts = |kind: &str| builtin(kind) || kind.starts_with("ghost:");
    let presence = Presence {
        mounted: &ghosts,
        parked: &never,
    };
    let layout = tree(json!([
        pane(0.5, "chat", "chat-main"),
        pane(0.2, "ghost-a", "ghost:a"),
        pane(0.3, "right", "right-tabs")
    ]));
    let measured = by_kind(&[
        ("chat-main", 829.0),
        ("ghost:a", 332.0),
        ("right-tabs", 300.0),
    ]);
    let after = fractions(&drag(&layout, &presence, 1, 1000.0, &measured));
    close(after[2], 0.3 - 180.0 / AVAIL, 9);
}

#[test]
fn starts_from_the_width_on_screen_before_self_heal() {
    let presence = Presence {
        mounted: &builtin,
        parked: &never,
    };
    let layout = tree(json!([
        pane(0.9, "chat", "chat-main"),
        pane(0.1, "right", "right-tabs")
    ]));
    let ledger = Ledger::root(&layout, &presence).unwrap();
    let seam = Seam {
        split_id: "root",
        left: 0,
        right: 1,
        scale: ledger.scale,
    };
    let measured = by_kind(&[("chat-main", 680.0), ("right-tabs", 120.0)]);
    let drag = RootDrag::start(&layout, &presence, seam, 800.0, false, &measured).unwrap();
    let delta = drag.delta(-8.0);
    close(drag.live(delta)["right"] * 100.0, 16.0, 6);
    let after = fractions(&drag.commit(&layout, delta, &presence, 800.0).unwrap());
    close(after[0], 0.84, 6);
    close(after[1], 0.16, 6);
}

#[test]
fn relays_chat_shrink_through_collapsed_siblings() {
    let ghosts = |kind: &str| builtin(kind) || kind.starts_with("ghost:");
    let presence = Presence {
        mounted: &ghosts,
        parked: &never,
    };
    let installed = tree(json!([
        pane(0.4, "chat", "chat-main"),
        pane(0.2, "ghost-repro", "ghost:repro"),
        pane(0.4, "right", "right-tabs"),
    ]));
    let measured = by_kind(&[
        ("chat-main", 1328.0),
        ("right-tabs", 0.0),
        ("ghost:repro", 332.0),
    ]);
    let after = fractions(&drag(&installed, &presence, 0, -1000.0, &measured));
    close(after[0], 0.05, 3);
    close(after[1], 0.759, 3);
    close(after[2], 0.191, 3);
    close(after.iter().sum(), 1.0, 6);
    close((1.0 - after[1]) * AVAIL, 400.0, 0);

    let frozen = tree(json!([
        pane(0.206, "right", "right-tabs"),
        pane(0.122, "ghost-pr", "ghost:pr"),
        pane(0.05, "chat", "chat-main"),
        pane(0.622, "ghost-canvas", "ghost:canvas"),
    ]));
    let measured = by_kind(&[
        ("right-tabs", 0.0),
        ("ghost:pr", 202.0),
        ("ghost:canvas", 1033.0),
        ("chat-main", 425.0),
    ]);
    let after = fractions(&drag(&frozen, &presence, 2, -1000.0, &measured));
    let moved = 25.0 / AVAIL;
    close(after[2], 0.05, 6);
    close(after[0], 0.206 - moved, 6);
    close(after[3], 0.622 + moved, 6);
}

#[test]
fn saves_the_right_tabs_width_and_equalizes_on_double_click() {
    let presence = Presence {
        mounted: &builtin,
        parked: &never,
    };
    let layout = tree(json!([
        pane(0.7, "chat", "chat-main"),
        pane(0.3, "right", "right-tabs")
    ]));
    let seam = Seam {
        split_id: "root",
        left: 0,
        right: 1,
        scale: 1.0,
    };
    let unmeasured = |_: &Node| None;
    let even = equalize_root(&layout, &presence, seam, AVAIL, false, &unmeasured).unwrap();
    close(fractions(&even)[0], 0.5, 9);
    let Node::Split(split) = &even.content else {
        unreachable!()
    };
    let Node::Pane(right) = &split.children[1].node else {
        unreachable!()
    };
    assert_eq!(right.preferred_width, Some(830.0));
}

#[test]
fn heals_only_starved_visible_panes() {
    let all = |_: &str| true;
    let starved = tree(json!([
        pane(0.607, "chat", "chat-main"),
        { "fraction": 0.112, "node": { "type": "pane", "id": "demo-hello", "panelKind": "ghost:hello", "minWidth": 240 } },
        pane(0.281, "right", "right-tabs"),
    ]));
    let fixed = fractions(&normalize_sub_min_fractions(&starved, 900.0, &all).unwrap());
    close(fixed[1] * 900.0, 120.0, 5);
    close(fixed[2], 0.281, 5);
    close(fixed.iter().sum(), 1.0, 5);
    assert!(fixed[0] * 900.0 >= 400.0);
    assert!(normalize_sub_min_fractions(&starved, 1400.0, &all).is_none());
    assert!(normalize_sub_min_fractions(&starved, 460.0, &all).is_none());
    assert!(normalize_sub_min_fractions(&starved, 900.0, &|k| k != "ghost:hello").is_none());

    let hide_gone = |k: &str| k != "ghost:gone";
    let gone = |right: f64, residue: f64| {
        tree(json!([
            pane(1.0 - right - residue, "chat", "chat-main"),
            pane(residue, "demo-gone", "ghost:gone"),
            pane(right, "right", "right-tabs")
        ]))
    };
    assert!(normalize_sub_min_fractions(&gone(0.12, 0.43), 900.0, &hide_gone).is_none());
    let fixed =
        fractions(&normalize_sub_min_fractions(&gone(0.05, 0.4), 1400.0, &hide_gone).unwrap());
    let scale = fixed[0] + fixed[2];
    close(fixed[2] / scale * 1400.0, 120.0, 5);
    assert_eq!(fixed[1], 0.4);

    let mut grid = tree(json!([
        pane(0.35, "chat", "chat-main"),
        { "fraction": 0.6, "node": { "type": "split", "id": "hidden-grid", "direction": "column",
            "children": [pane(0.5, "hidden", "ghost:hidden"), pane(0.5, "spare", "ghost:hidden")] } },
        pane(0.05, "right", "right-tabs"),
    ]));
    let Node::Split(root) = &mut grid.content else {
        unreachable!()
    };
    let Node::Split(hidden) = &mut root.children[1].node else {
        unreachable!()
    };
    hidden.children.pop();
    hidden.children[0].fraction = 1.0;
    assert!(normalize_sub_min_fractions(&grid, 2000.0, &builtin).is_none());
}

#[test]
fn column_cells_keep_their_floors() {
    let presence = Presence {
        mounted: &|k| builtin(k) || k == "tab-group:bottom",
        parked: &never,
    };
    let layout = tree(json!([
        { "fraction": 0.6, "node": { "type": "split", "id": "chat-column", "direction": "column",
            "children": [pane(0.65, "chat", "chat-main"), pane(0.35, "tab-group-bottom", "tab-group:bottom")] } },
        pane(0.4, "right", "right-tabs"),
    ]));
    let Node::Split(root) = &layout.content else {
        unreachable!()
    };
    let Node::Split(column) = &root.children[0].node else {
        unreachable!()
    };
    let ledger = Ledger::new(&column.children, &presence);
    let (top, bottom) = (ledger.mounted[0], ledger.mounted[1]);
    let drag = ColumnDrag::start("chat-column", &top, &bottom, ledger.scale, 1000.0).unwrap();
    let delta = drag.delta(1000.0);
    close(delta, 0.35 - 0.12, 9);
    close(drag.live(delta)["tab-group-bottom"], 0.12, 9);
    let after = drag.commit(&layout, delta).unwrap();
    let Node::Split(root) = &after.content else {
        unreachable!()
    };
    let Node::Split(column) = &root.children[0].node else {
        unreachable!()
    };
    close(column.children[1].fraction, 0.12, 9);
    let even = equalize_column(&layout, "chat-column", &top, &bottom, ledger.scale).unwrap();
    let Node::Split(root) = &even.content else {
        unreachable!()
    };
    let Node::Split(column) = &root.children[0].node else {
        unreachable!()
    };
    close(column.children[0].fraction, 0.5, 9);
}
