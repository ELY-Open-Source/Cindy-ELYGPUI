//! `RootDivider` and `ColumnDivider` gestures from `LayoutRoot.tsx`, without the pointer plumbing.

use std::collections::HashMap;

use super::geometry::{
    Entry, Ledger, Measured, Presence, Relay, Seam, align_expanded_pane_fractions,
    chat_shrink_plan, commit_root, normalize_sub_min_fractions, pane_min_height_px,
    side_room_share,
};
use crate::layout_tree::{Layout, MIN_SPLIT_CHILD_FRACTION};

const CHAT: &str = "chat-main";

/// A root divider drag from press to release; shares are visible shares.
#[derive(Debug)]
pub struct RootDrag {
    split_id: String,
    left: usize,
    right: usize,
    scale: f64,
    left_id: String,
    right_id: String,
    start_left: f64,
    start_right: f64,
    min: f64,
    max: f64,
    available: f64,
    relay: Option<Relay>,
    base: Option<Layout>,
    live_base: HashMap<String, f64>,
}

fn chat_side<'e, 'a>(
    left: &'e Entry<'a>,
    right: &'e Entry<'a>,
) -> Option<(&'e Entry<'a>, &'e Entry<'a>)> {
    if left.node.contains_kind(CHAT) {
        Some((left, right))
    } else if right.node.contains_kind(CHAT) {
        Some((right, left))
    } else {
        None
    }
}

impl RootDrag {
    /// `onPointerDown`: aligns the ledger with the screen, then fixes how far each side can go.
    pub fn start(
        layout: &Layout,
        presence: &Presence,
        seam: Seam,
        available: f64,
        preserve_chat_width: bool,
        measured: Measured,
    ) -> Option<Self> {
        let ledger = Ledger::root(layout, presence)?;
        let (left, right) = (*ledger.find(seam.left)?, *ledger.find(seam.right)?);
        let siblings: Vec<Entry> = ledger.entries().copied().collect();
        let aligned = preserve_chat_width
            .then(|| align_expanded_pane_fractions(layout, available, presence, measured))
            .flatten()
            .or_else(|| normalize_sub_min_fractions(layout, available, &|k| presence.visible(k)));
        let aligned_ledger = aligned.as_ref().and_then(|a| Ledger::root(a, presence));
        let aligned_left = aligned_ledger
            .as_ref()
            .and_then(|l| l.find(seam.left).copied());
        let aligned_right = aligned_ledger
            .as_ref()
            .and_then(|l| l.find(seam.right).copied());
        let both = aligned_left.zip(aligned_right);
        let (drag_left, drag_right) = both.unwrap_or((left, right));
        let drag_siblings: Vec<Entry> = match (&aligned_ledger, both) {
            (Some(l), Some(_)) => l.entries().copied().collect(),
            _ => siblings,
        };
        let plan = chat_side(&drag_left, &drag_right).and_then(|(chat, other)| {
            chat_shrink_plan(chat, other, available, &drag_siblings, seam.scale, measured)
                .map(|p| (chat.tree_index, p))
        });
        let mut min = -side_room_share(&drag_left, available, seam.scale, measured);
        let mut max = side_room_share(&drag_right, available, seam.scale, measured);
        match &plan {
            Some((chat, (room, _))) if *chat == drag_left.tree_index => min = -room,
            Some((_, (room, _))) => max = *room,
            None => {}
        }
        let live_base = match &aligned_ledger {
            Some(l) if preserve_chat_width => l
                .entries()
                .map(|e| (e.node.id().to_owned(), e.share))
                .collect(),
            _ => HashMap::new(),
        };
        let start_left = aligned_left.map_or(left.share, |e| e.share);
        let start_right = aligned_right.map_or(right.share, |e| e.share);
        let (left_id, right_id) = (left.node.id().to_owned(), right.node.id().to_owned());
        drop(aligned_ledger);
        Some(RootDrag {
            split_id: seam.split_id.to_owned(),
            left: seam.left,
            right: seam.right,
            scale: seam.scale,
            left_id,
            right_id,
            start_left,
            start_right,
            min,
            max,
            available,
            relay: plan.map(|(_, (_, relay))| relay),
            base: both.is_some().then_some(aligned).flatten(),
            live_base,
        })
    }

    /// The clamped move for a pointer `dx` pixels from the press.
    pub fn delta(&self, dx: f64) -> f64 {
        self.max.min(self.min.max(dx / self.available))
    }

    /// Shares to paint while dragging.
    pub fn live(&self, delta: f64) -> HashMap<String, f64> {
        let mut live = self.live_base.clone();
        live.insert(self.left_id.clone(), self.start_left + delta);
        live.insert(self.right_id.clone(), self.start_right - delta);
        live
    }

    /// Release: writes the move into the tree it started from.
    pub fn commit(
        &self,
        current: &Layout,
        delta: f64,
        presence: &Presence,
        available: f64,
    ) -> Option<Layout> {
        if delta == 0.0 {
            return None;
        }
        let seam = Seam {
            split_id: &self.split_id,
            left: self.left,
            right: self.right,
            scale: self.scale,
        };
        commit_root(
            self.base.as_ref().unwrap_or(current),
            seam,
            delta,
            self.relay.as_ref(),
            presence,
            available,
        )
    }
}

/// `onDoubleClick` on a root divider: both neighbors take the mean of their shares.
pub fn equalize_root(
    current: &Layout,
    presence: &Presence,
    seam: Seam,
    available: f64,
    preserve_chat_width: bool,
    measured: Measured,
) -> Option<Layout> {
    let ledger = Ledger::root(current, presence)?;
    let (left, right) = (*ledger.find(seam.left)?, *ledger.find(seam.right)?);
    let siblings: Vec<Entry> = ledger.entries().copied().collect();
    let plan = chat_side(&left, &right).and_then(|(chat, other)| {
        chat_shrink_plan(chat, other, available, &siblings, seam.scale, measured)
            .map(|p| (chat.tree_index, p))
    });
    let aligned = preserve_chat_width
        .then(|| align_expanded_pane_fractions(current, available, presence, measured))
        .flatten();
    let aligned_ledger = aligned.as_ref().and_then(|a| Ledger::root(a, presence));
    let share = |entry: &Entry| {
        aligned_ledger
            .as_ref()
            .and_then(|l| l.find(entry.tree_index))
            .map_or(entry.share, |e| e.share)
    };
    let (left_share, right_share) = (share(&left), share(&right));
    let mut delta = (left_share + right_share) / 2.0 - left_share;
    if let Some((chat, (room, _))) = &plan {
        let chat_left = *chat == left.tree_index;
        if (chat_left && delta < 0.0) || (!chat_left && delta > 0.0) {
            delta = if chat_left {
                delta.max(-room)
            } else {
                delta.min(*room)
            };
        }
    }
    let relay = plan.as_ref().map(|(_, (_, relay))| relay);
    commit_root(
        aligned.as_ref().unwrap_or(current),
        seam,
        delta,
        relay,
        presence,
        available,
    )
}

/// A divider between two cells of a column split; `height` is the split's painted height.
#[derive(Debug)]
pub struct ColumnDrag {
    split_id: String,
    top: usize,
    bottom: usize,
    scale: f64,
    top_id: String,
    bottom_id: String,
    start_top: f64,
    start_bottom: f64,
    top_room: f64,
    bottom_room: f64,
    height: f64,
}

impl ColumnDrag {
    pub fn start(
        split_id: &str,
        top: &Entry,
        bottom: &Entry,
        scale: f64,
        height: f64,
    ) -> Option<Self> {
        if height <= 0.0 {
            return None;
        }
        let room = |e: &Entry| {
            let floor = if scale > 0.0 {
                (e.fraction - MIN_SPLIT_CHILD_FRACTION) / scale
            } else {
                0.0
            };
            0.0f64.max((e.share - pane_min_height_px(e.node) / height).min(floor))
        };
        Some(ColumnDrag {
            split_id: split_id.to_owned(),
            top: top.tree_index,
            bottom: bottom.tree_index,
            scale,
            top_id: top.node.id().to_owned(),
            bottom_id: bottom.node.id().to_owned(),
            start_top: top.share,
            start_bottom: bottom.share,
            top_room: room(top),
            bottom_room: room(bottom),
            height,
        })
    }

    pub fn delta(&self, dy: f64) -> f64 {
        self.bottom_room.min((-self.top_room).max(dy / self.height))
    }

    pub fn live(&self, delta: f64) -> HashMap<String, f64> {
        HashMap::from([
            (self.top_id.clone(), self.start_top + delta),
            (self.bottom_id.clone(), self.start_bottom - delta),
        ])
    }

    pub fn commit(&self, current: &Layout, delta: f64) -> Option<Layout> {
        commit_column(
            current,
            &self.split_id,
            self.top,
            self.bottom,
            delta * self.scale,
        )
    }
}

/// `ColumnDivider` double-click: the two cells split their space evenly.
pub fn equalize_column(
    current: &Layout,
    split_id: &str,
    top: &Entry,
    bottom: &Entry,
    scale: f64,
) -> Option<Layout> {
    let amount = (top.share + bottom.share) / 2.0 - top.share;
    commit_column(
        current,
        split_id,
        top.tree_index,
        bottom.tree_index,
        amount * scale,
    )
}

fn commit_column(
    current: &Layout,
    split_id: &str,
    top: usize,
    bottom: usize,
    tree_amount: f64,
) -> Option<Layout> {
    if tree_amount == 0.0 {
        return None;
    }
    let op = if tree_amount > 0.0 {
        current.transfer_split_fraction(split_id, bottom, top, tree_amount)
    } else {
        current.transfer_split_fraction(split_id, top, bottom, -tree_amount)
    };
    op.map_err(|reason| log::debug!(target: "layout", "divider commit refused: {reason}"))
        .ok()
}
