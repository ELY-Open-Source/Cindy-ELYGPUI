//! `LayoutRoot.tsx`'s sizing math. Widths here are the ones the view paints, so they stand in for
//! Cindy's DOM measurements.

use std::collections::HashMap;

use crate::cindy_json::js_round;
use crate::layout_tree::{Direction, Layout, MIN_SPLIT_CHILD_FRACTION, Node};

pub const CHAT_MIN_PX: f64 = 400.0;
pub const NON_CHAT_FLOOR_PX: f64 = 120.0;
const GRID_PANE_MIN_HEIGHT_PX: f64 = 120.0;
const CHAT_MIN_HEIGHT_PX: f64 = 200.0;
const CHAT: &str = "chat-main";
const RIGHT_TABS: &str = "right-tabs";

/// Which kinds render: mounted (registered) and, of those, parked at zero size.
pub struct Presence<'a> {
    pub mounted: &'a dyn Fn(&str) -> bool,
    pub parked: &'a dyn Fn(&str) -> bool,
}

impl Presence<'_> {
    pub fn visible(&self, kind: &str) -> bool {
        (self.mounted)(kind) && !(self.parked)(kind)
    }

    fn node_visible(&self, node: &Node) -> bool {
        match node {
            Node::Pane(pane) => self.visible(&pane.panel_kind),
            Node::Split(split) => split.children.iter().any(|c| self.node_visible(&c.node)),
        }
    }

    fn node_mounted(&self, node: &Node) -> bool {
        match node {
            Node::Pane(pane) => (self.mounted)(&pane.panel_kind),
            Node::Split(split) => split.children.iter().any(|c| self.node_mounted(&c.node)),
        }
    }
}

/// A split child as the screen sees it; `share` splits 100% among the visible ones.
#[derive(Clone, Copy, Debug)]
pub struct Entry<'a> {
    pub tree_index: usize,
    pub fraction: f64,
    pub share: f64,
    pub parked: bool,
    pub node: &'a Node,
}

/// `activeSplitLedger`: mounted children in tree order; `scale` is the visible fractions' sum.
pub struct Ledger<'a> {
    pub mounted: Vec<Entry<'a>>,
    pub scale: f64,
}

impl<'a> Ledger<'a> {
    pub fn new(children: &'a [crate::layout_tree::Child], presence: &Presence) -> Self {
        let mounted: Vec<_> = children
            .iter()
            .enumerate()
            .filter(|(_, c)| presence.node_mounted(&c.node))
            .collect();
        let visible = mounted
            .iter()
            .filter(|(_, c)| presence.node_visible(&c.node))
            .count();
        let scale = mounted
            .iter()
            .filter(|(_, c)| presence.node_visible(&c.node))
            .fold(0.0, |sum, (_, c)| sum + c.fraction);
        let mounted = mounted
            .into_iter()
            .map(|(tree_index, c)| {
                let parked = !presence.node_visible(&c.node);
                let share = match () {
                    _ if parked => 0.0,
                    _ if scale > 0.0 => c.fraction / scale,
                    _ => 1.0 / visible.max(1) as f64,
                };
                Entry {
                    tree_index,
                    fraction: c.fraction,
                    share,
                    parked,
                    node: &c.node,
                }
            })
            .collect();
        Ledger { mounted, scale }
    }

    /// The root row's ledger, when the content is a row split.
    pub fn root(layout: &'a Layout, presence: &Presence) -> Option<Self> {
        match &layout.content {
            Node::Split(split) if split.direction == Direction::Row => {
                Some(Self::new(&split.children, presence))
            }
            _ => None,
        }
    }

    pub fn entries(&self) -> impl Iterator<Item = &Entry<'a>> {
        self.mounted.iter().filter(|e| !e.parked)
    }

    pub fn find(&self, tree_index: usize) -> Option<&Entry<'a>> {
        self.entries().find(|e| e.tree_index == tree_index)
    }
}

/// `paneMinPx`: only chat has a real floor; others keep a grab-able width.
pub fn pane_min_px(node: &Node) -> f64 {
    match node {
        Node::Pane(pane) if pane.panel_kind == CHAT => CHAT_MIN_PX,
        Node::Pane(_) => NON_CHAT_FLOOR_PX,
        Node::Split(split) => {
            let mins = split.children.iter().map(|c| pane_min_px(&c.node));
            match split.direction {
                Direction::Row => mins.fold(0.0, |sum, v| sum + v),
                Direction::Column => mins.fold(NON_CHAT_FLOOR_PX, f64::max),
            }
        }
    }
}

/// `paneMinHeightPx`.
pub fn pane_min_height_px(node: &Node) -> f64 {
    match node {
        Node::Pane(pane) if pane.panel_kind == CHAT => CHAT_MIN_HEIGHT_PX,
        Node::Pane(_) => GRID_PANE_MIN_HEIGHT_PX,
        Node::Split(split) => {
            let mins = split.children.iter().map(|c| pane_min_height_px(&c.node));
            match split.direction {
                Direction::Column => mins.fold(0.0, |sum, v| sum + v),
                Direction::Row => mins.fold(GRID_PANE_MIN_HEIGHT_PX, f64::max),
            }
        }
    }
}

fn root_children_mut(layout: &mut Layout) -> &mut Vec<crate::layout_tree::Child> {
    let Node::Split(split) = &mut layout.content else {
        unreachable!("checked as a split")
    };
    &mut split.children
}

/// `normalizeSubMinFractions`: lifts panes below their floor, chat pays; none when nothing to fix.
pub fn normalize_sub_min_fractions(
    layout: &Layout,
    avail: f64,
    registered: &dyn Fn(&str) -> bool,
) -> Option<Layout> {
    let Node::Split(split) = &layout.content else {
        return None;
    };
    if split.direction != Direction::Row {
        return None;
    }
    let children = &split.children;
    let chat = children.iter().position(|c| c.node.contains_kind(CHAT))?;
    fn shown(node: &Node, registered: &dyn Fn(&str) -> bool) -> bool {
        match node {
            Node::Pane(pane) => registered(&pane.panel_kind),
            Node::Split(split) => split.children.iter().any(|c| shown(&c.node, registered)),
        }
    }
    let scale = children
        .iter()
        .filter(|c| shown(&c.node, registered))
        .fold(0.0, |sum, c| sum + c.fraction);
    if scale <= 0.0 {
        return None;
    }
    let mut needed = 0.0;
    let mut bumps = Vec::new();
    for (index, child) in children.iter().enumerate() {
        if child.node.contains_kind(CHAT) || !shown(&child.node, registered) {
            continue;
        }
        let min_share = pane_min_px(&child.node) / avail;
        let share = child.fraction / scale;
        if share >= min_share {
            continue;
        }
        bumps.push((index, min_share));
        needed += min_share - share;
    }
    if bumps.is_empty() {
        return None;
    }
    let chat_after = children[chat].fraction / scale - needed;
    if chat_after * avail < CHAT_MIN_PX {
        return None;
    }
    let mut next = layout.clone();
    let children = root_children_mut(&mut next);
    for (index, target) in bumps {
        children[index].fraction = target * scale;
    }
    children[chat].fraction = chat_after * scale;
    Some(next)
}

/// CSS `clamp()`: the floor wins over the ceiling.
fn clamp(min: f64, value: f64, max: f64) -> f64 {
    min.max(value.min(max))
}

/// The root row's painted widths by node id, as `computeRootWidths` and the flex row give them:
/// one 1px divider between visible children, chat takes the rest, collapsed panes paint nothing.
pub fn root_widths(
    layout: &Layout,
    avail: f64,
    live: Option<&HashMap<String, f64>>,
    presence: &Presence,
    collapsed: &dyn Fn(&str) -> bool,
) -> HashMap<String, f64> {
    let mut widths = HashMap::new();
    let Some(ledger) = Ledger::root(layout, presence) else {
        return widths;
    };
    let mut used = 0.0;
    let mut chat = None;
    for entry in ledger.entries() {
        let node = entry.node;
        if node.contains_kind(CHAT) {
            chat = Some(node.id());
            continue;
        }
        let share = live
            .and_then(|l| l.get(node.id()))
            .copied()
            .unwrap_or(entry.share);
        let saved = match node {
            Node::Pane(pane) if live.is_none() => pane.preferred_width,
            _ => None,
        };
        let width = match node {
            Node::Pane(pane) if collapsed(&pane.panel_kind) => 0.0,
            _ => clamp(
                pane_min_px(node),
                saved.unwrap_or(share * avail),
                avail - CHAT_MIN_PX,
            ),
        };
        used += width;
        widths.insert(node.id().to_owned(), width);
    }
    let dividers = ledger.entries().count().saturating_sub(1) as f64;
    if let Some(chat) = chat {
        widths.insert(chat.to_owned(), (avail - used - dividers).max(CHAT_MIN_PX));
    }
    widths
}

/// Painted width; none when the node is not on screen. A collapsed pane is `Some(0)`.
pub type Measured<'a> = &'a dyn Fn(&Node) -> Option<f64>;

fn measured_positive(measured: Measured, node: &Node) -> Option<f64> {
    measured(node).filter(|w| *w > 0.0)
}

/// `alignExpandedPaneFractions`: rebase the ledger on the widths on screen.
pub fn align_expanded_pane_fractions(
    layout: &Layout,
    available: f64,
    presence: &Presence,
    measured: Measured,
) -> Option<Layout> {
    if available <= 0.0 {
        return None;
    }
    let ledger = Ledger::root(layout, presence)?;
    let chat = *ledger.entries().find(|e| e.node.contains_kind(CHAT))?;
    let widths = ledger
        .entries()
        .filter(|e| e.tree_index != chat.tree_index)
        .map(|e| Some((e.tree_index, measured_positive(measured, e.node)?)))
        .collect::<Option<Vec<_>>>()?;
    let mut next = layout.clone();
    let children = root_children_mut(&mut next);
    let mut used = 0.0;
    for (index, width) in widths {
        let fraction = MIN_SPLIT_CHILD_FRACTION.max(width / available * ledger.scale);
        children[index].fraction = fraction;
        used += fraction;
    }
    let remaining = ledger.scale - used;
    if remaining < MIN_SPLIT_CHILD_FRACTION {
        return None;
    }
    children[chat.tree_index].fraction = remaining;
    Some(next)
}

/// Where a chat-shrinking commit takes its fraction from: chat first, then collapsed siblings.
#[derive(Clone, Debug, PartialEq)]
pub struct Relay {
    pub sources: Vec<usize>,
    pub receiver: usize,
}

/// How far one side may give, in visible share: its painted width over its floor, and its
/// ledger room over `MIN_SPLIT_CHILD_FRACTION`.
pub fn side_room_share(entry: &Entry, available: f64, scale: f64, measured: Measured) -> f64 {
    if available <= 0.0 {
        return 0.0;
    }
    let basis = measured_positive(measured, entry.node).unwrap_or(entry.share * available);
    let px_room = basis - pane_min_px(entry.node);
    let floor_room = if scale > 0.0 {
        (entry.fraction - MIN_SPLIT_CHILD_FRACTION) / scale
    } else {
        0.0
    };
    0.0f64.max((px_room / available).min(floor_room))
}

/// `chatShrinkPlan`: chat may give down to 400px, its ledger topped up by zero-width siblings.
pub fn chat_shrink_plan(
    chat: &Entry,
    other: &Entry,
    available: f64,
    siblings: &[Entry],
    scale: f64,
    measured: Measured,
) -> Option<(f64, Relay)> {
    if available <= 0.0 {
        return None;
    }
    let width = measured(chat.node).filter(|w| *w >= CHAT_MIN_PX)?;
    let floor_room = |e: &Entry| {
        if scale > 0.0 {
            0.0f64.max((e.fraction - MIN_SPLIT_CHILD_FRACTION) / scale)
        } else {
            0.0
        }
    };
    let mut room = floor_room(chat);
    let mut sources = vec![chat.tree_index];
    for sibling in siblings {
        if sibling.tree_index == chat.tree_index || sibling.tree_index == other.tree_index {
            continue;
        }
        if measured(sibling.node).is_some_and(|w| w < 1.0) {
            room += floor_room(sibling);
            sources.push(sibling.tree_index);
        }
    }
    let px_room = 0.0f64.max(width - CHAT_MIN_PX) / available;
    Some((
        px_room.min(room),
        Relay {
            sources,
            receiver: other.tree_index,
        },
    ))
}

/// A root divider between two visible children, by tree index.
#[derive(Clone, Copy, Debug)]
pub struct Seam<'s> {
    pub split_id: &'s str,
    pub left: usize,
    pub right: usize,
    /// The rendered ledger's scale (`shareScale`).
    pub scale: f64,
}

/// The `RootDivider` commit: `amount` is visible share toward the left. None when nothing moves
/// or the tree refuses it.
pub fn commit_root(
    current: &Layout,
    seam: Seam,
    amount: f64,
    relay: Option<&Relay>,
    presence: &Presence,
    available: f64,
) -> Option<Layout> {
    let tree_amount = amount * seam.scale;
    if tree_amount == 0.0 {
        return None;
    }
    let compressing_chat = relay.is_some_and(|r| {
        if r.receiver == seam.right {
            tree_amount < 0.0
        } else {
            tree_amount > 0.0
        }
    });
    let op = match relay {
        Some(r) if compressing_chat => current.transfer_split_fraction_relay(
            seam.split_id,
            &r.sources,
            r.receiver,
            tree_amount.abs(),
        ),
        _ if tree_amount > 0.0 => {
            current.transfer_split_fraction(seam.split_id, seam.right, seam.left, tree_amount)
        }
        _ => current.transfer_split_fraction(seam.split_id, seam.left, seam.right, -tree_amount),
    };
    let mut next = op
        .map_err(|reason| log::debug!(target: "layout", "divider commit refused: {reason}"))
        .ok()?;
    let Node::Split(split) = &current.content else {
        return Some(next);
    };
    let touches_right_tabs = [seam.left, seam.right].iter().any(|i| {
        split
            .children
            .get(*i)
            .is_some_and(|c| c.node.is_pane_of(RIGHT_TABS))
    });
    if touches_right_tabs && available > 0.0 {
        let target = Ledger::root(&next, presence).and_then(|l| {
            l.entries()
                .find(|e| e.node.is_pane_of(RIGHT_TABS))
                .map(|e| (e.tree_index, e.share))
        });
        if let Some((index, share)) = target {
            let width = NON_CHAT_FLOOR_PX.max(
                (share * available)
                    .min(available - CHAT_MIN_PX)
                    .min(16000.0),
            );
            if let Node::Pane(pane) = &mut root_children_mut(&mut next)[index].node {
                pane.preferred_width = Some(js_round(width));
            }
        }
    }
    Some(next)
}
