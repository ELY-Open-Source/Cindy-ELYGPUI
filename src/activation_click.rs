use std::time::Instant;

const ACTIVATION_WINDOW_MS: f64 = 120.0;
const RELEASE_GUARD_MS: f64 = 300.0;

/// What the swallow sees: window activation and primary-button presses.
#[derive(Clone, Copy, Debug)]
pub enum Input {
    Blur,
    Focus,
    Down,
    Up,
}

/// Cindy's `createSwallowActivationState`; gpui builds clicks from down and up, so no click input.
#[derive(Default)]
pub struct Swallow {
    was_blurred: bool,
    armed_at: Option<f64>,
    held_down: bool,
    release_guard_until: Option<f64>,
}

impl Swallow {
    /// True when the input belongs to the click that activated the window.
    pub fn handle(&mut self, input: Input, now_ms: f64) -> bool {
        if self
            .armed_at
            .is_some_and(|at| now_ms - at > ACTIVATION_WINDOW_MS)
        {
            self.armed_at = None;
        }
        if !self.held_down && self.release_guard_until.is_some_and(|until| now_ms > until) {
            self.release_guard_until = None;
        }
        match input {
            Input::Blur => {
                *self = Self {
                    was_blurred: true,
                    ..Self::default()
                };
                false
            }
            Input::Focus => {
                if self.was_blurred {
                    self.armed_at = Some(now_ms);
                }
                self.was_blurred = false;
                false
            }
            Input::Down => {
                self.release_guard_until = None;
                self.held_down = self.armed_at.take().is_some();
                self.held_down
            }
            Input::Up => {
                if self.held_down {
                    self.held_down = false;
                    self.release_guard_until = Some(now_ms + RELEASE_GUARD_MS);
                    return true;
                }
                self.release_guard_until.is_some()
            }
        }
    }
}

/// Whether this platform swallows, as Cindy decides per platform.
pub enum Policy {
    /// macOS: gpui marks the activating click; Cindy's `acceptFirstMouse`.
    FirstMouse,
    /// Windows: Cindy's renderer timing heuristic.
    Timing { swallow: Swallow, started: Instant },
    /// Linux, or the setting off.
    Off,
}

impl Policy {
    pub fn new(mac_setting: bool, windows_setting: bool) -> Self {
        match () {
            _ if cfg!(target_os = "macos") && mac_setting => Policy::FirstMouse,
            _ if cfg!(target_os = "windows") && windows_setting => Policy::Timing {
                swallow: Swallow::default(),
                started: Instant::now(),
            },
            _ => Policy::Off,
        }
    }

    pub fn activation(&mut self, active: bool) {
        if let Policy::Timing { swallow, started } = self {
            let now = started.elapsed().as_secs_f64() * 1000.0;
            swallow.handle(if active { Input::Focus } else { Input::Blur }, now);
        }
    }

    /// True to stop a primary-button press or release before the page sees it.
    pub fn swallow(&mut self, input: Input, first_mouse: bool, held: &mut bool) -> bool {
        match self {
            Policy::FirstMouse => match input {
                Input::Down => {
                    *held = first_mouse;
                    first_mouse
                }
                Input::Up => std::mem::take(held),
                _ => false,
            },
            Policy::Timing { swallow, started } => {
                swallow.handle(input, started.elapsed().as_secs_f64() * 1000.0)
            }
            Policy::Off => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Input::*, Swallow};

    fn run(steps: &[(super::Input, f64)]) -> Vec<bool> {
        let mut s = Swallow::default();
        steps
            .iter()
            .map(|(input, at)| s.handle(*input, *at))
            .collect()
    }

    #[test]
    fn matches_cindy_swallow_cases() {
        assert_eq!(
            run(&[(Blur, 0.0), (Focus, 10.0), (Down, 20.0), (Up, 60.0)]),
            [false, false, true, true]
        );
        assert_eq!(
            run(&[(Focus, 0.0), (Down, 10.0)]),
            [false, false],
            "focus without blur"
        );
        assert_eq!(
            run(&[(Blur, 0.0), (Focus, 10.0), (Down, 140.0)]),
            [false, false, false],
            "past 120ms"
        );
        assert_eq!(
            run(&[
                (Blur, 0.0),
                (Focus, 10.0),
                (Down, 20.0),
                (Up, 2000.0),
                (Down, 2100.0)
            ]),
            [false, false, true, true, false],
            "long press swallowed in full, next press passes"
        );
        assert_eq!(
            run(&[
                (Blur, 0.0),
                (Focus, 10.0),
                (Blur, 30.0),
                (Focus, 500.0),
                (Down, 520.0)
            ]),
            [false, false, false, false, true],
            "re-blur re-arms on the second focus"
        );
        assert_eq!(
            run(&[
                (Blur, 0.0),
                (Focus, 10.0),
                (Down, 20.0),
                (Up, 30.0),
                (Up, 200.0),
                (Up, 400.0)
            ]),
            [false, false, true, true, true, false],
            "release guard holds 300ms after release"
        );
    }
}
