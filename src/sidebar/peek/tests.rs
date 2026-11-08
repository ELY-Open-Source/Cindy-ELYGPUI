//! Cindy's `useSidebarPeek.test.ts`, case for case.

use std::time::{Duration, Instant};

use super::{CLOSE_ANIM, CLOSE_GRACE, OPEN_DELAY, PIN_FREEZE, Peek, State};

struct Clock {
    now: Instant,
    peek: Peek,
}

impl Clock {
    fn new(collapsed: bool) -> Self {
        Clock {
            now: Instant::now(),
            peek: Peek::new(collapsed),
        }
    }

    fn advance(&mut self, by: Duration) {
        self.now += by;
        self.peek.tick(self.now);
    }

    fn ms(&mut self, ms: u64) {
        self.advance(Duration::from_millis(ms));
    }

    fn opened(collapsed: bool) -> Self {
        let mut c = Clock::new(collapsed);
        c.peek.trigger_enter(c.now);
        c.advance(OPEN_DELAY);
        c
    }
}

#[test]
fn a_short_hover_never_opens() {
    let mut c = Clock::new(true);
    c.peek.trigger_enter(c.now);
    c.advance(OPEN_DELAY - Duration::from_millis(20));
    c.peek.trigger_leave(c.now);
    c.ms(1000);
    assert_eq!(c.peek.state, State::Idle);
}

#[test]
fn opens_after_120ms_and_closes_through_the_grace_and_the_slide() {
    let mut c = Clock::opened(true);
    assert_eq!(c.peek.state, State::Peeking);
    assert!(c.peek.visible());
    c.peek.pointer_moved(false, false, c.now);
    c.advance(CLOSE_GRACE);
    assert_eq!(c.peek.state, State::PeekClosing);
    c.advance(CLOSE_ANIM);
    assert_eq!(c.peek.state, State::Idle);
    assert!(!c.peek.visible());
}

#[test]
fn entering_the_drawer_within_the_grace_keeps_it() {
    let mut c = Clock::opened(true);
    c.peek.trigger_leave(c.now);
    c.advance(CLOSE_GRACE - Duration::from_millis(40));
    c.peek.pointer_moved(true, false, c.now);
    c.ms(5000);
    assert_eq!(c.peek.state, State::Peeking);
}

#[test]
fn leaving_the_drawer_for_the_toggle_keeps_it() {
    let mut c = Clock::opened(true);
    c.peek.pointer_moved(true, true, c.now);
    c.ms(5000);
    assert_eq!(c.peek.state, State::Peeking);
}

#[test]
fn showing_while_peeking_pins() {
    let mut c = Clock::opened(true);
    c.peek.set_collapsed(false, c.now);
    assert_eq!(c.peek.state, State::Pinning);
    assert!(c.peek.visible());
    c.advance(PIN_FREEZE);
    assert_eq!(c.peek.state, State::Idle);
}

#[test]
fn a_hide_locks_hover_until_the_pointer_leaves_the_toggle() {
    let mut c = Clock::new(false);
    c.peek.set_collapsed(true, c.now);
    c.peek.trigger_enter(c.now);
    c.advance(OPEN_DELAY + Duration::from_millis(100));
    assert_eq!(c.peek.state, State::Idle);
    c.peek.trigger_leave(c.now);
    c.peek.trigger_enter(c.now);
    c.advance(OPEN_DELAY);
    assert_eq!(c.peek.state, State::Peeking);
}

#[test]
fn a_move_off_the_toggle_also_unlocks() {
    let mut c = Clock::new(false);
    c.peek.set_collapsed(true, c.now);
    c.peek.pointer_moved(false, false, c.now);
    c.peek.trigger_enter(c.now);
    c.advance(OPEN_DELAY);
    assert_eq!(c.peek.state, State::Peeking);
}

#[test]
fn moves_keep_it_on_the_drawer_and_close_it_elsewhere() {
    let mut c = Clock::opened(true);
    c.peek.pointer_moved(true, false, c.now);
    c.ms(5000);
    assert_eq!(c.peek.state, State::Peeking);
    c.peek.pointer_moved(false, false, c.now);
    c.advance(CLOSE_GRACE + CLOSE_ANIM);
    assert_eq!(c.peek.state, State::Idle);
}

#[test]
fn a_blur_slides_it_out_at_once() {
    let mut c = Clock::opened(true);
    c.peek.blur(c.now);
    assert_eq!(c.peek.state, State::PeekClosing);
    c.advance(CLOSE_ANIM);
    assert_eq!(c.peek.state, State::Idle);
}

#[test]
fn a_shown_sidebar_never_peeks() {
    let mut c = Clock::new(false);
    c.peek.trigger_enter(c.now);
    c.advance(OPEN_DELAY + Duration::from_millis(100));
    assert_eq!(c.peek.state, State::Idle);
}

#[test]
fn hovering_again_mid_slide_returns_to_peeking() {
    let mut c = Clock::opened(true);
    c.peek.pointer_moved(false, false, c.now);
    c.advance(CLOSE_GRACE);
    assert_eq!(c.peek.state, State::PeekClosing);
    c.peek.trigger_enter(c.now);
    c.advance(OPEN_DELAY);
    assert_eq!(c.peek.state, State::Peeking);
    c.ms(5000);
    assert_eq!(c.peek.state, State::Peeking);
}
