//! `useSidebarPeek.ts`: the hidden sidebar slides out while the toggle is hovered.

use std::time::{Duration, Instant};

pub const OPEN_DELAY: Duration = Duration::from_millis(120);
pub const CLOSE_GRACE: Duration = Duration::from_millis(160);
pub const CLOSE_ANIM: Duration = Duration::from_millis(200);
pub const PIN_FREEZE: Duration = Duration::from_millis(300);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum State {
    Idle,
    Peeking,
    PeekClosing,
    Pinning,
}

/// The state with its four timers, as deadlines on the caller's clock.
#[derive(Debug)]
pub struct Peek {
    pub state: State,
    hover_locked: bool,
    collapsed: bool,
    open_at: Option<Instant>,
    grace_at: Option<Instant>,
    anim_at: Option<Instant>,
    pin_at: Option<Instant>,
}

impl Peek {
    pub fn new(collapsed: bool) -> Self {
        Peek {
            state: State::Idle,
            hover_locked: false,
            collapsed,
            open_at: None,
            grace_at: None,
            anim_at: None,
            pin_at: None,
        }
    }

    /// `isPeekVisible`: the sidebar draws as a drawer.
    pub fn visible(&self) -> bool {
        self.state != State::Idle
    }

    fn clear_all(&mut self) {
        (self.open_at, self.grace_at, self.anim_at, self.pin_at) = (None, None, None, None);
    }

    /// Pointer moves matter while peeking and while a hide holds the hover lock.
    pub fn listening(&self) -> bool {
        self.state == State::Peeking || self.hover_locked
    }

    /// The earliest timer, for the caller to wake on.
    pub fn next(&self) -> Option<Instant> {
        [self.open_at, self.grace_at, self.anim_at, self.pin_at]
            .into_iter()
            .flatten()
            .min()
    }

    /// Fires every timer due by `now`.
    pub fn tick(&mut self, now: Instant) {
        while let Some(at) = self.next().filter(|at| *at <= now) {
            if self.open_at == Some(at) {
                self.open_at = None;
                let open = self.collapsed && !self.hover_locked;
                if open && matches!(self.state, State::Idle | State::PeekClosing) {
                    self.anim_at = None;
                    self.state = State::Peeking;
                }
            } else if self.grace_at == Some(at) {
                self.grace_at = None;
                self.begin_close(at);
            } else if self.anim_at == Some(at) {
                self.anim_at = None;
                self.state = State::Idle;
            } else {
                self.pin_at = None;
                self.state = State::Idle;
            }
        }
    }

    /// `beginClose`: slide out at once.
    pub fn begin_close(&mut self, now: Instant) {
        if self.state != State::Peeking {
            return;
        }
        self.grace_at = None;
        self.state = State::PeekClosing;
        self.anim_at = Some(now + CLOSE_ANIM);
    }

    /// `scheduleClose`: a return within 160ms cancels it.
    fn schedule_close(&mut self, now: Instant) {
        if self.state == State::Peeking && self.grace_at.is_none() {
            self.grace_at = Some(now + CLOSE_GRACE);
        }
    }

    pub fn trigger_enter(&mut self, now: Instant) {
        if !self.collapsed || self.hover_locked {
            return;
        }
        match self.state {
            State::Peeking => self.grace_at = None,
            State::Pinning => {}
            _ if self.open_at.is_none() => self.open_at = Some(now + OPEN_DELAY),
            _ => {}
        }
    }

    pub fn trigger_leave(&mut self, now: Instant) {
        self.hover_locked = false;
        self.open_at = None;
        self.schedule_close(now);
    }

    /// A hide locks hover until the pointer leaves the toggle; a show while peeking pins.
    pub fn set_collapsed(&mut self, collapsed: bool, now: Instant) {
        if collapsed == self.collapsed {
            return;
        }
        self.collapsed = collapsed;
        if collapsed {
            self.hover_locked = true;
            self.open_at = None;
            if self.state == State::Pinning {
                self.pin_at = None;
                self.state = State::Idle;
            }
            return;
        }
        self.hover_locked = false;
        if matches!(self.state, State::Peeking | State::PeekClosing) {
            self.clear_all();
            self.state = State::Pinning;
            self.pin_at = Some(now + PIN_FREEZE);
        }
    }

    /// Any pointer move: while peeking, off the drawer and toggle closes; a lock opens off the toggle.
    pub fn pointer_moved(
        &mut self,
        over_drawer_or_trigger: bool,
        over_trigger: bool,
        now: Instant,
    ) {
        if self.state == State::Peeking {
            if over_drawer_or_trigger {
                self.grace_at = None;
            } else {
                self.schedule_close(now);
            }
        }
        if self.hover_locked && !over_trigger {
            self.hover_locked = false;
        }
    }

    /// The window lost focus while peeking.
    pub fn blur(&mut self, now: Instant) {
        self.begin_close(now);
    }
}

#[cfg(test)]
mod tests;
