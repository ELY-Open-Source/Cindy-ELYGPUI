//! A CSS transition of one property, with its reversing shortening.

use std::time::{Duration, Instant};

#[derive(Clone, Copy)]
pub struct Transition<T> {
    from: T,
    to: T,
    /// The reversing-adjusted start value.
    back: T,
    factor: f32,
    start: Instant,
    length: Duration,
}

impl<T: Copy + PartialEq> Transition<T> {
    pub fn still(at: T, now: Instant) -> Self {
        Self {
            from: at,
            to: at,
            back: at,
            factor: 1.0,
            start: now,
            length: Duration::ZERO,
        }
    }

    /// The eased progress at `now`, and whether it still runs.
    fn progress(&self, now: Instant, easing: fn(f32) -> f32) -> (f32, bool) {
        if self.length.is_zero() {
            return (1.0, false);
        }
        let t = now.saturating_duration_since(self.start).as_secs_f32() / self.length.as_secs_f32();
        (easing(t.min(1.0)), t < 1.0)
    }

    pub fn at(&self, now: Instant, easing: fn(f32) -> f32, lerp: fn(T, T, f32) -> T) -> (T, bool) {
        let (p, running) = self.progress(now, easing);
        (lerp(self.from, self.to, p), running)
    }

    /// Starts toward `to` from the current value, shortened when it reverses a running one.
    pub fn toward(
        &mut self,
        to: T,
        (now, length): (Instant, Duration),
        easing: fn(f32) -> f32,
        lerp: fn(T, T, f32) -> T,
    ) {
        // `transition: none` stops a running one.
        if length.is_zero() {
            *self = Self::still(to, now);
            return;
        }
        if to == self.to {
            return;
        }
        let (current, running) = self.at(now, easing, lerp);
        let (p, _) = self.progress(now, easing);
        let (back, factor) = match running && to == self.back {
            true => {
                let factor = (p * self.factor + 1.0 - self.factor).abs().clamp(0.0, 1.0);
                (self.to, factor)
            }
            false => (current, 1.0),
        };
        *self = Self {
            from: current,
            to,
            back,
            factor,
            start: now,
            length: length.mul_f32(factor),
        };
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use super::Transition;

    fn lerp(a: f32, b: f32, t: f32) -> f32 {
        a + (b - a) * t
    }

    fn linear(t: f32) -> f32 {
        t
    }

    #[test]
    fn a_reversal_takes_the_time_it_has_run() {
        let now = Instant::now();
        let ms = |n| now + Duration::from_millis(n);
        let mut t = Transition::still(0.0, now);
        t.toward(1.0, (now, Duration::from_millis(100)), linear, lerp);
        let (v, running) = t.at(ms(40), linear, lerp);
        assert!((v - 0.4).abs() < 1e-5 && running);
        t.toward(0.0, (ms(40), Duration::from_millis(100)), linear, lerp);
        assert!((t.at(ms(60), linear, lerp).0 - 0.2).abs() < 1e-5);
        assert_eq!(t.at(ms(80), linear, lerp), (0.0, false), "40ms, not 100");
        t.toward(1.0, (ms(80), Duration::from_millis(100)), linear, lerp);
        assert_eq!(
            t.at(ms(180), linear, lerp),
            (1.0, false),
            "a finished one is not reversed"
        );
        t.toward(0.0, (ms(180), Duration::from_millis(100)), linear, lerp);
        t.toward(0.0, (ms(200), Duration::ZERO), linear, lerp);
        assert_eq!(
            t.at(ms(200), linear, lerp),
            (0.0, false),
            "no duration ends it"
        );
    }
}
