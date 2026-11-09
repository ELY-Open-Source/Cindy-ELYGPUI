//! A CSS transition of one property, with its reversing shortening.

use std::time::{Duration, Instant};

use gpui::{App, ElementId, Hsla, Rgba, Window};

use crate::theme::oklch;

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

pub fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

/// Premultiplied, as CSS interpolates colors.
fn premultiplied(a: [f32; 4], b: [f32; 4], t: f32) -> [f32; 4] {
    let alpha = lerp(a[3], b[3], t);
    let channel = |i: usize| match alpha == 0.0 {
        true => 0.0,
        false => lerp(a[i] * a[3], b[i] * b[3], t) / alpha,
    };
    [channel(0), channel(1), channel(2), alpha]
}

/// Between legacy colors: sRGB.
pub fn mix_srgb(a: Hsla, b: Hsla, t: f32) -> Hsla {
    let (a, b): (Rgba, Rgba) = (a.into(), b.into());
    let [r, g, b, a] = premultiplied([a.r, a.g, a.b, a.a], [b.r, b.g, b.b, b.a], t);
    Rgba { r, g, b, a }.into()
}

/// With an `oklch()` token on either side: Oklab.
pub fn mix_oklab(a: Hsla, b: Hsla, t: f32) -> Hsla {
    let lab = |c: Hsla| {
        let rgba: Rgba = c.into();
        let (l, chroma, h) = oklch::from_srgb(rgba);
        let h = h.to_radians();
        [l, chroma * h.cos(), chroma * h.sin(), rgba.a]
    };
    let [l, ok_a, ok_b, alpha] = premultiplied(lab(a), lab(b), t);
    oklch::to_srgb(l, ok_a.hypot(ok_b), ok_b.atan2(ok_a).to_degrees(), alpha).into()
}

/// How a property moves: its length, timing function and interpolation.
pub struct Curve<T> {
    pub length: Duration,
    pub easing: fn(f32) -> f32,
    pub lerp: fn(T, T, f32) -> T,
}

/// This frame's value of the transition under `key`; asks for frames while it runs.
pub fn keyed<T: Copy + PartialEq + 'static>(
    key: ElementId,
    to: T,
    curve: Curve<T>,
    window: &mut Window,
    cx: &mut App,
) -> T {
    let now = cx.background_executor().now();
    let state = window.use_keyed_state(key, cx, |_, _| Transition::still(to, now));
    let (value, running) = state.update(cx, |t, _| {
        t.toward(to, (now, curve.length), curve.easing, curve.lerp);
        t.at(now, curve.easing, curve.lerp)
    });
    if running {
        window.request_animation_frame();
    }
    value
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

    #[test]
    fn colors_mix_premultiplied_in_their_space() {
        use gpui::{Hsla, Rgba};
        let rgba = |r, g, b, a| Hsla::from(Rgba { r, g, b, a });
        let clear = rgba(0.0, 0.0, 0.0, 0.0);
        let tint = rgba(1.0, 1.0, 1.0, 0.06);
        let mid: Rgba = super::mix_srgb(clear, tint, 0.5).into();
        assert!(
            (mid.a - 0.03).abs() < 1e-4 && (mid.r - 1.0).abs() < 1e-3,
            "{mid:?}"
        );
        let (dark, light) = (rgba(0.09, 0.09, 0.09, 1.0), rgba(0.2, 0.2, 0.2, 1.0));
        let ends: [Rgba; 2] = [
            super::mix_oklab(dark, light, 0.0).into(),
            super::mix_oklab(dark, light, 1.0).into(),
        ];
        assert!((ends[0].r - 0.09).abs() < 1e-3 && (ends[1].r - 0.2).abs() < 1e-3);
        let half: Rgba = super::mix_oklab(dark, light, 0.5).into();
        let srgb: Rgba = super::mix_srgb(dark, light, 0.5).into();
        assert!(
            half.r < srgb.r,
            "Oklab's midpoint is darker in sRGB: {half:?} {srgb:?}"
        );
    }
}
