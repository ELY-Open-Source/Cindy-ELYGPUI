#![cfg_attr(
    not(test),
    expect(dead_code, reason = "the tree's editors land in 6.3 to 6.7")
)]

use super::{
    CHAT_MAIN, Child, Direction, FRACTION_TOLERANCE, Layout, MIN_SPLIT_CHILD_FRACTION, Node, Pane,
    SESSION_LIST, Split,
};
use serde_json::Value;

use crate::cindy_json::js_number;

/// A tab group, named by its edge of the chat area.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TabGroup {
    Left,
    Right,
    Top,
    Bottom,
}

/// A pane as `walkPanes` yields it; the sidebar's `collapsed` is unchecked.
#[derive(Clone, Debug, PartialEq)]
pub struct PaneRef<'a> {
    pub id: &'a str,
    pub panel_kind: &'a str,
    pub collapsed: Option<Value>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SplitChildRef {
    pub split_id: String,
    pub child_index: usize,
    pub fraction: f64,
}

const TAB_GROUP_KIND_PREFIX: &str = "tab-group:";
pub const CHAT_COLUMN_SPLIT_ID: &str = "chat-column";
const TAB_GROUP_SIDE_FRACTION: f64 = 0.25;
const TAB_GROUP_STACK_FRACTION: f64 = 0.35;

impl TabGroup {
    pub const ALL: [TabGroup; 4] = [
        TabGroup::Left,
        TabGroup::Right,
        TabGroup::Top,
        TabGroup::Bottom,
    ];

    pub fn id(self) -> &'static str {
        match self {
            TabGroup::Left => "left",
            TabGroup::Right => "right",
            TabGroup::Top => "top",
            TabGroup::Bottom => "bottom",
        }
    }

    /// `tabGroupPanelKind`: the right group keeps the old `right-tabs`.
    pub fn panel_kind(self) -> String {
        match self {
            TabGroup::Right => "right-tabs".into(),
            group => format!("{TAB_GROUP_KIND_PREFIX}{}", group.id()),
        }
    }

    /// `tabGroupIdFromPanelKind`.
    pub fn from_panel_kind(kind: &str) -> Option<Self> {
        if kind == "right-tabs" {
            return Some(TabGroup::Right);
        }
        let id = kind.strip_prefix(TAB_GROUP_KIND_PREFIX)?;
        Self::ALL
            .into_iter()
            .find(|g| g.id() == id && *g != TabGroup::Right)
    }
}

fn panes<'a>(node: &'a Node, out: &mut Vec<PaneRef<'a>>) {
    match node {
        Node::Pane(pane) => out.push(PaneRef {
            id: &pane.id,
            panel_kind: &pane.panel_kind,
            collapsed: pane.collapsed.map(Value::Bool),
        }),
        Node::Split(split) => split.children.iter().for_each(|c| panes(&c.node, out)),
    }
}

fn panes_mut<'a>(node: &'a mut Node, out: &mut Vec<&'a mut Pane>) {
    match node {
        Node::Pane(pane) => out.push(pane),
        Node::Split(split) => split
            .children
            .iter_mut()
            .for_each(|c| panes_mut(&mut c.node, out)),
    }
}

fn split_mut<'a>(node: &'a mut Node, id: &str) -> Option<&'a mut Split> {
    let Node::Split(split) = node else {
        return None;
    };
    if split.id == id {
        return Some(split);
    }
    split
        .children
        .iter_mut()
        .find_map(|c| split_mut(&mut c.node, id))
}

fn contains_kind(node: &Node, kind: &str) -> bool {
    match node {
        Node::Pane(pane) => pane.panel_kind == kind,
        Node::Split(split) => split.children.iter().any(|c| contains_kind(&c.node, kind)),
    }
}

fn split_child_by_kind(node: &Node, kind: &str) -> Option<SplitChildRef> {
    let Node::Split(split) = node else {
        return None;
    };
    split
        .children
        .iter()
        .enumerate()
        .find_map(|(i, child)| match &child.node {
            Node::Pane(pane) if pane.panel_kind == kind => Some(SplitChildRef {
                split_id: split.id.clone(),
                child_index: i,
                fraction: child.fraction,
            }),
            nested => split_child_by_kind(nested, kind),
        })
}

/// The splits and child indices from `node` down to the pane of `kind`.
fn path_to(node: &Node, kind: &str) -> Option<Vec<(Direction, usize)>> {
    match node {
        Node::Pane(pane) => (pane.panel_kind == kind).then(Vec::new),
        Node::Split(split) => split.children.iter().enumerate().find_map(|(i, child)| {
            let mut rest = path_to(&child.node, kind)?;
            rest.insert(0, (split.direction, i));
            Some(rest)
        }),
    }
}

fn normalize(split: &mut Split) {
    let sum = split.children.iter().fold(0.0, |acc, c| acc + c.fraction);
    if sum.is_finite() && sum > 0.0 {
        split.children.iter_mut().for_each(|c| c.fraction /= sum);
    }
}

fn root_row(layout: &mut Layout) -> &mut Split {
    let Node::Split(split) = &mut layout.content else {
        unreachable!("checked as a split")
    };
    split
}

impl Layout {
    fn finish(self, op: &str) -> Result<Layout, String> {
        self.validate()
            .map_err(|reason| format!("{op} rejected: {reason}"))?;
        Ok(self)
    }

    /// `walkPanes`: the sidebar, then content depth first; float is empty in v1.
    pub fn walk_panes(&self) -> Vec<PaneRef<'_>> {
        let mut out = vec![PaneRef {
            id: &self.sidebar.id,
            panel_kind: SESSION_LIST,
            collapsed: self.sidebar.rest.get("collapsed").cloned(),
        }];
        panes(&self.content, &mut out);
        out
    }

    /// `findSplitChildByPanelKind`.
    pub fn split_child_by_kind(&self, kind: &str) -> Option<SplitChildRef> {
        split_child_by_kind(&self.content, kind)
    }

    /// The root child holding `kind`, when the root is a row split.
    fn root_child_with(&self, kind: &str) -> Option<usize> {
        match &self.content {
            Node::Split(split) if split.direction == Direction::Row => split
                .children
                .iter()
                .position(|c| contains_kind(&c.node, kind)),
            _ => None,
        }
    }

    /// `setPaneCollapsed`.
    pub fn set_pane_collapsed(&self, pane_id: &str, collapsed: bool) -> Result<Layout, String> {
        if !self.walk_panes().iter().any(|p| p.id == pane_id) {
            return Err(format!("pane not found: {pane_id}"));
        }
        let mut next = self.clone();
        if next.sidebar.id == pane_id {
            next.sidebar
                .rest
                .insert("collapsed".into(), collapsed.into());
        }
        let mut panes = Vec::new();
        panes_mut(&mut next.content, &mut panes);
        panes
            .into_iter()
            .filter(|p| p.id == pane_id)
            .for_each(|p| p.collapsed = Some(collapsed));
        next.finish("setPaneCollapsed")
    }

    /// `setSplitChildFraction`: the child takes `fraction`, its siblings share the rest.
    pub fn set_split_child_fraction(
        &self,
        split_id: &str,
        index: usize,
        fraction: f64,
    ) -> Result<Layout, String> {
        if !(fraction.is_finite() && fraction > 0.0 && fraction < 1.0) {
            return Err("fraction must be between 0 and 1 (exclusive)".into());
        }
        let mut next = self.clone();
        let split =
            split_mut(&mut next.content, split_id).ok_or(format!("split not found: {split_id}"))?;
        if index >= split.children.len() {
            return Err(format!("child index out of range: {index}"));
        }
        let siblings = split.children.iter().enumerate();
        let sibling_sum = siblings.fold(0.0, |acc, (i, c)| {
            acc + if i == index { 0.0 } else { c.fraction }
        });
        let remainder = 1.0 - fraction;
        for (i, child) in split.children.iter_mut().enumerate() {
            child.fraction = if i == index {
                fraction
            } else {
                child.fraction / sibling_sum * remainder
            };
        }
        normalize(split);
        next.finish("setSplitChildFraction")
    }

    /// `insertRootSplitPane` as tab groups use it: the rest give way evenly.
    fn insert_root_pane(&self, pane: Pane, index: usize) -> Result<Layout, String> {
        let mut next = self.clone();
        let split = root_row(&mut next);
        let fraction = TAB_GROUP_SIDE_FRACTION;
        split
            .children
            .iter_mut()
            .for_each(|c| c.fraction *= 1.0 - fraction);
        split
            .children
            .insert(index, Child::new(fraction, Node::Pane(pane)));
        normalize(split);
        next.finish("insertRootSplitPane")
    }

    /// `swapRootSplitChildrenByKind`: whole root children trade places, fractions with them.
    pub fn swap_root_children_by_kind(&self, a: &str, b: &str) -> Result<Layout, String> {
        if !matches!(self.content, Node::Split(_)) {
            return Err("content is not a split".into());
        }
        let (i, j) = match (self.root_child_with(a), self.root_child_with(b)) {
            (Some(i), Some(j)) => (i, j),
            (None, _) => return Err(format!("pane kind not found: {a}")),
            (_, None) => return Err(format!("pane kind not found: {b}")),
        };
        if i == j {
            return Err("cannot swap within the same root child".into());
        }
        let mut next = self.clone();
        root_row(&mut next).children.swap(i, j);
        next.finish("swapRootSplitChildrenByKind")
    }

    /// `paneEdgeRelativeToChat`: the edge of the chat area where the pane of `kind` sits.
    pub fn pane_edge_relative_to_chat(&self, kind: &str) -> Option<TabGroup> {
        let to_pane = path_to(&self.content, kind)?;
        let to_chat = path_to(&self.content, CHAT_MAIN)?;
        let ((direction, a), (_, b)) =
            to_pane.into_iter().zip(to_chat).find(|(a, b)| a.1 != b.1)?;
        Some(match (direction, a < b) {
            (Direction::Row, true) => TabGroup::Left,
            (Direction::Row, false) => TabGroup::Right,
            (Direction::Column, true) => TabGroup::Top,
            (Direction::Column, false) => TabGroup::Bottom,
        })
    }

    /// `ensureTabGroupPane`: puts the group's pane beside, above or below the chat area.
    pub fn ensure_tab_group_pane(&self, group: TabGroup) -> Result<Layout, String> {
        let kind = group.panel_kind();
        if self.split_child_by_kind(&kind).is_some() {
            return Err(format!("tab group already present: {}", group.id()));
        }
        if !matches!(&self.content, Node::Split(s) if s.direction == Direction::Row) {
            return Err("content root must be a row split".into());
        }
        let chat_index = self
            .root_child_with(CHAT_MAIN)
            .ok_or("chat-main not found")?;
        let pane = Pane::new(&format!("tab-group-{}", group.id()), &kind);
        match group {
            TabGroup::Left => return self.insert_root_pane(pane, chat_index),
            TabGroup::Right => return self.insert_root_pane(pane, chat_index + 1),
            TabGroup::Top | TabGroup::Bottom => {}
        }
        let top = group == TabGroup::Top;
        let mut next = self.clone();
        let root_child = &mut root_row(&mut next).children[chat_index];
        match &mut root_child.node {
            Node::Pane(chat) => {
                let chat = Child::new(1.0 - TAB_GROUP_STACK_FRACTION, Node::Pane(chat.clone()));
                let group = Child::new(TAB_GROUP_STACK_FRACTION, Node::Pane(pane));
                root_child.node = Node::Split(Split {
                    id: CHAT_COLUMN_SPLIT_ID.into(),
                    direction: Direction::Column,
                    children: if top {
                        vec![group, chat]
                    } else {
                        vec![chat, group]
                    },
                    extra: Default::default(),
                });
            }
            Node::Split(column) if column.direction == Direction::Column => {
                column
                    .children
                    .iter_mut()
                    .for_each(|c| c.fraction *= 1.0 - TAB_GROUP_STACK_FRACTION);
                let at = if top { 0 } else { column.children.len() };
                column
                    .children
                    .insert(at, Child::new(TAB_GROUP_STACK_FRACTION, Node::Pane(pane)));
                normalize(column);
            }
            Node::Split(_) => return Err("chat container must be a column split".into()),
        }
        next.finish("ensureTabGroupPane")
    }

    /// `transferSplitFraction`: moves `amount` between two children; others keep theirs.
    pub fn transfer_split_fraction(
        &self,
        split_id: &str,
        from: usize,
        to: usize,
        amount: f64,
    ) -> Result<Layout, String> {
        if !(amount.is_finite() && amount != 0.0) {
            return Err("amount must be a non-zero finite number".into());
        }
        if from == to {
            return Err("invalid child indices".into());
        }
        let mut next = self.clone();
        let split =
            split_mut(&mut next.content, split_id).ok_or(format!("split not found: {split_id}"))?;
        let len = split.children.len();
        if from >= len || to >= len {
            return Err("child index out of range".into());
        }
        let floor = MIN_SPLIT_CHILD_FRACTION - FRACTION_TOLERANCE;
        if split.children[from].fraction - amount < floor
            || split.children[to].fraction + amount < floor
        {
            return Err(format!(
                "transfer would shrink a child below {MIN_SPLIT_CHILD_FRACTION}"
            ));
        }
        split.children[from].fraction -= amount;
        split.children[to].fraction += amount;
        normalize(split);
        next.finish("transferSplitFraction")
    }

    /// `transferSplitFractionRelay`: `sources` pay `amount` in turn, each down to the floor.
    pub fn transfer_split_fraction_relay(
        &self,
        split_id: &str,
        sources: &[usize],
        to: usize,
        amount: f64,
    ) -> Result<Layout, String> {
        if !(amount.is_finite() && amount > 0.0) {
            return Err("amount must be a positive finite number".into());
        }
        if sources.is_empty() {
            return Err("sources must be a non-empty array".into());
        }
        for (i, source) in sources.iter().enumerate() {
            if *source == to {
                return Err("toIndex must not appear in sources".into());
            }
            if sources[..i].contains(source) {
                return Err(format!("duplicate source index: {source}"));
            }
        }
        let mut next = self.clone();
        let split =
            split_mut(&mut next.content, split_id).ok_or(format!("split not found: {split_id}"))?;
        let len = split.children.len();
        if to >= len || sources.iter().any(|s| *s >= len) {
            return Err("child index out of range".into());
        }
        let floor = MIN_SPLIT_CHILD_FRACTION - FRACTION_TOLERANCE;
        let mut remaining = amount;
        for source in sources {
            if remaining <= FRACTION_TOLERANCE {
                break;
            }
            let child = &mut split.children[*source];
            let take = remaining.min(child.fraction - floor);
            if take > 0.0 {
                child.fraction -= take;
                remaining -= take;
            }
        }
        if remaining > FRACTION_TOLERANCE {
            return Err(format!(
                "sources cannot cover the transfer, short by {}",
                js_number(remaining)
            ));
        }
        split.children[to].fraction += amount - remaining;
        normalize(split);
        next.finish("transferSplitFractionRelay")
    }
}
