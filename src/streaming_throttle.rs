//! `useStreamingThrottle`: a streamed text, shown at most once per interval of its length.
use crate::streaming_markdown::throttle;

#[cfg(test)]
mod tests;

/// The hook's state and refs; times are `performance.now()`'s milliseconds.
pub struct Throttle {
    shown: String,
    latest: String,
    /// The `enabled` the effect last ran with, none before its first run.
    ran: Option<bool>,
    last: f64,
    due: Option<f64>,
}

impl Throttle {
    pub fn new(value: &str) -> Self {
        Self {
            shown: value.to_owned(),
            latest: value.to_owned(),
            ran: None,
            last: 0.0,
            due: None,
        }
    }

    /// The effect after a render: it runs when the value or the switch changed.
    pub fn render(&mut self, value: &str, enabled: bool, now: f64) {
        if self.ran == Some(enabled) && self.latest == value {
            return;
        }
        self.ran = Some(enabled);
        self.latest.clear();
        self.latest.push_str(value);
        if !enabled {
            // A stream's end shows its last text, and the next stream starts with a frame.
            self.due = None;
            self.shown.clone_from(&self.latest);
            self.last = 0.0;
            return;
        }
        let interval = throttle(value) as f64;
        let elapsed = now - self.last;
        if elapsed >= interval {
            self.fire(now);
        } else {
            self.due = Some(now + (interval - elapsed).max(0.0));
        }
    }

    /// When the timer fires, if one waits.
    pub fn due(&self) -> Option<f64> {
        self.due
    }

    /// `emit` with the latest text: the timer's callback at `now`.
    pub fn fire(&mut self, now: f64) {
        self.due = None;
        self.last = now;
        self.shown.clone_from(&self.latest);
    }

    /// What the hook answers a render with `value`.
    pub fn shown<'a>(&'a self, value: &'a str, enabled: bool) -> &'a str {
        match enabled {
            true => &self.shown,
            false => value,
        }
    }
}
