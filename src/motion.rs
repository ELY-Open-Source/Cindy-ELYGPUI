//! CSS timing functions gpui lacks, and a value moving along one.

use std::time::{Duration, Instant};

/// CSS `cubic-bezier(x1, y1, x2, y2)` at progress `x`.
fn cubic_bezier(x1: f32, y1: f32, x2: f32, y2: f32, x: f32) -> f32 {
    let at = |a: f32, b: f32, t: f32| {
        3.0 * a * t * (1.0 - t).powi(2) + 3.0 * b * t * t * (1.0 - t) + t.powi(3)
    };
    let (mut low, mut high) = (0.0f32, 1.0f32);
    let mut t = x;
    for _ in 0..32 {
        let found = at(x1, x2, t);
        if (found - x).abs() < 1e-6 {
            break;
        }
        if found < x {
            low = t
        } else {
            high = t
        }
        t = (low + high) / 2.0;
    }
    at(y1, y2, t)
}

/// `prefers-reduced-motion`: Cindy's motion tokens and `motion-reduce:` transitions drop to 0.
#[derive(Clone, Copy, Debug, Default)]
pub struct ReducedMotion(pub bool);

impl gpui::Global for ReducedMotion {}

/// A transition's length, none under reduced motion.
pub fn length(base: std::time::Duration, cx: &gpui::App) -> std::time::Duration {
    if cx.global::<ReducedMotion>().0 {
        std::time::Duration::ZERO
    } else {
        base
    }
}

/// Reads the system setting again; a change reaches Ely and repaints every window.
pub fn refresh_reduced_motion(cx: &mut gpui::App) {
    let reduced = match crate::os_appearance::reduced_motion() {
        Ok(reduced) => reduced,
        Err(error) => {
            log::warn!(target: "theme", "reduced motion unread ({error:#}); keeping {:?}", cx.try_global::<ReducedMotion>());
            return;
        }
    };
    if cx
        .try_global::<ReducedMotion>()
        .is_some_and(|r| r.0 == reduced)
    {
        return;
    }
    log::info!(target: "theme", "reduced motion {reduced}");
    cx.set_global(ReducedMotion(reduced));
    ely_gpui_component::theme::Theme::update(cx, |theme| theme.reduced_motion = reduced);
    cx.refresh_windows();
}

/// Cindy's `--motion-fast`.
pub const FAST: std::time::Duration = std::time::Duration::from_millis(150);

/// Cindy's `--motion-ease-out`, the `ease-out` of Tailwind animations.
pub fn motion_ease_out(x: f32) -> f32 {
    cubic_bezier(0.16, 1.0, 0.3, 1.0, x.clamp(0.0, 1.0))
}

/// `sidebar-peek-in`'s `cubic-bezier(0.22,1,0.36,1)`.
pub fn peek_in(x: f32) -> f32 {
    cubic_bezier(0.22, 1.0, 0.36, 1.0, x.clamp(0.0, 1.0))
}

/// Cindy's `--motion-ease-move`, the sidebar's width easing.
pub fn ease_move(x: f32) -> f32 {
    cubic_bezier(0.4, 0.0, 0.2, 1.0, x.clamp(0.0, 1.0))
}

/// CSS `ease-out`.
pub fn ease_out(x: f32) -> f32 {
    cubic_bezier(0.0, 0.0, 0.58, 1.0, x.clamp(0.0, 1.0))
}

/// A value on its way to another along `--motion-ease-move`.
#[derive(Clone, Copy, Debug)]
pub struct Tween {
    from: f32,
    to: f32,
    start: Instant,
    length: Duration,
    easing: fn(f32) -> f32,
}

impl Tween {
    pub fn still(at: f32, now: Instant) -> Self {
        Tween {
            from: at,
            to: at,
            start: now,
            length: Duration::ZERO,
            easing: ease_move,
        }
    }

    pub fn at(&self, now: Instant) -> (f32, bool) {
        let spent = now.saturating_duration_since(self.start).as_secs_f32();
        let t = if self.length.is_zero() {
            1.0
        } else {
            spent / self.length.as_secs_f32()
        };
        let value = self.from + (self.to - self.from) * (self.easing)(t);
        (value, t < 1.0)
    }

    pub fn toward(&mut self, to: f32, now: Instant, length: Duration) {
        self.toward_along(to, now, length, ease_move);
    }

    /// `toward` along another curve.
    pub fn toward_along(
        &mut self,
        to: f32,
        now: Instant,
        length: Duration,
        easing: fn(f32) -> f32,
    ) {
        if to != self.to || length != self.length {
            *self = Tween {
                from: self.at(now).0,
                to,
                start: now,
                length,
                easing,
            };
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn eases_out_as_css_does() {
        assert_eq!(super::ease_out(0.0), 0.0);
        assert!((super::ease_out(1.0) - 1.0).abs() < 1e-5);
        assert!((super::ease_out(0.5) - 0.684_644).abs() < 1e-4);
        assert!((super::motion_ease_out(0.5) - 0.971_779).abs() < 1e-4);
        assert!((super::ease_move(0.5) - 0.775_561).abs() < 1e-4);
        assert!((super::peek_in(0.5) - 0.961_382).abs() < 1e-4);
    }
}
