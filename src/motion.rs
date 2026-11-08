//! CSS timing functions gpui lacks.

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

/// Cindy's `--motion-fast`.
pub const FAST: std::time::Duration = std::time::Duration::from_millis(150);

/// Cindy's `--motion-ease-out`, the `ease-out` of Tailwind animations.
pub fn motion_ease_out(x: f32) -> f32 {
    cubic_bezier(0.16, 1.0, 0.3, 1.0, x.clamp(0.0, 1.0))
}

/// Cindy's `--motion-ease-move`, the sidebar's width easing.
pub fn ease_move(x: f32) -> f32 {
    cubic_bezier(0.4, 0.0, 0.2, 1.0, x.clamp(0.0, 1.0))
}

/// CSS `ease-out`.
pub fn ease_out(x: f32) -> f32 {
    cubic_bezier(0.0, 0.0, 0.58, 1.0, x.clamp(0.0, 1.0))
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
    }
}
