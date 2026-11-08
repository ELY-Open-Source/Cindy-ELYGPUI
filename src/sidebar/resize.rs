//! `useHorizontalResize.ts` with `useSidebarResize.ts`'s limits, rail and snap.

use crate::cindy_json::js_to_number;

pub const WIDTH_KEY: &str = "sidebar-width";
pub const DEFAULT_WIDTH: f64 = 260.0;
const MIN_WIDTH: f64 = 180.0;
const MAX_WIDTH: f64 = 480.0;
const RAIL_THRESHOLD: f64 = 120.0;
pub const RAIL_WIDTH: f64 = 78.0;
const TWO_COL_SNAP_WIDTH: f64 = 272.0;
const SNAP_TOLERANCE: f64 = 16.0;

/// `readPersistedWidth`: a stored width out of range falls back to the default.
pub fn persisted(stored: Option<&str>) -> f64 {
    stored
        .map(js_to_number)
        .filter(|w| w.is_finite() && (MIN_WIDTH..=MAX_WIDTH).contains(w))
        .unwrap_or(DEFAULT_WIDTH)
}

/// What a step asks of its owner.
#[derive(Debug, Default, PartialEq)]
pub struct Effect {
    /// The rail turned on or off.
    pub rail: Option<bool>,
    /// The width to keep under `sidebar-width`.
    pub persist: Option<f64>,
}

#[derive(Debug)]
pub struct Resize {
    pub width: f64,
    pub dragging: bool,
    start_x: f64,
    start_width: f64,
    rail_active: bool,
}

impl Resize {
    pub fn new(stored: Option<&str>) -> Self {
        let width = persisted(stored);
        Resize {
            width,
            dragging: false,
            start_x: 0.0,
            start_width: width,
            rail_active: false,
        }
    }

    /// `handleDragStart`: a rail drag starts from the rail's width.
    pub fn start(&mut self, x: f64, rail: bool) {
        self.dragging = true;
        self.start_x = x;
        self.rail_active = rail;
        if rail {
            self.width = RAIL_WIDTH;
        }
        self.start_width = self.width;
    }

    /// `handlePointerMove`: below 120px the rail turns on; the card snap holds near 272px.
    pub fn moved(&mut self, x: f64, card_mode: bool) -> Effect {
        if !self.dragging {
            return Effect::default();
        }
        let raw = self.start_width + x - self.start_x;
        let mut effect = Effect::default();
        if raw < RAIL_THRESHOLD && !self.rail_active {
            self.rail_active = true;
            effect.rail = Some(true);
        } else if raw >= RAIL_THRESHOLD && self.rail_active {
            self.rail_active = false;
            effect.rail = Some(false);
        }
        let mut width = MAX_WIDTH.min(RAIL_WIDTH.max(raw));
        if !self.rail_active && card_mode && (width - TWO_COL_SNAP_WIDTH).abs() <= SNAP_TOLERANCE {
            width = TWO_COL_SNAP_WIDTH;
        }
        self.width = width;
        effect
    }

    /// `handlePointerUp`: a rail release restores the stored width; otherwise it is stored.
    pub fn up(&mut self, stored: Option<&str>) -> Effect {
        if !self.dragging {
            return Effect::default();
        }
        self.dragging = false;
        if self.rail_active {
            self.width = persisted(stored);
            return Effect::default();
        }
        self.width = MIN_WIDTH.max(self.width);
        Effect {
            rail: None,
            persist: Some(self.width),
        }
    }

    /// `resetWidth`: double-click leaves the rail and goes back to 260px.
    pub fn reset(&mut self, rail: bool) -> Effect {
        self.rail_active = false;
        self.width = DEFAULT_WIDTH;
        Effect {
            rail: rail.then_some(false),
            persist: Some(DEFAULT_WIDTH),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Effect, Resize, persisted};

    #[test]
    fn reads_a_stored_width_as_javascript_does() {
        assert_eq!(persisted(Some(" 300 ")), 300.0);
        for bad in [
            None,
            Some(""),
            Some("179"),
            Some("481"),
            Some("wide"),
            Some("Infinity"),
        ] {
            assert_eq!(persisted(bad), 260.0, "{bad:?}");
        }
    }

    #[test]
    fn drags_into_and_out_of_the_rail() {
        let mut r = Resize::new(Some("300"));
        r.start(300.0, false);
        assert_eq!(r.moved(400.0, false), Effect::default());
        assert_eq!(r.width, 400.0);
        assert_eq!(r.moved(900.0, false), Effect::default());
        assert_eq!(r.width, 480.0);
        assert_eq!(
            r.moved(110.0, false),
            Effect {
                rail: Some(true),
                persist: None
            }
        );
        assert_eq!(r.width, 110.0);
        assert_eq!(r.moved(0.0, false), Effect::default());
        assert_eq!(r.width, 78.0);
        assert_eq!(r.up(Some("300")), Effect::default());
        assert_eq!(r.width, 300.0, "the rail keeps the stored width");

        r.start(78.0, true);
        assert_eq!(r.width, 78.0);
        assert_eq!(
            r.moved(78.0 + 50.0, false),
            Effect {
                rail: Some(false),
                persist: None
            }
        );
        assert_eq!(r.moved(78.0 + 100.0, false), Effect::default());
        assert_eq!(
            r.up(Some("300")),
            Effect {
                rail: None,
                persist: Some(180.0)
            }
        );
        assert_eq!(r.width, 180.0, "release lifts to the floor");
    }

    #[test]
    fn snaps_in_card_mode_and_resets_on_double_click() {
        let mut r = Resize::new(None);
        r.start(260.0, false);
        r.moved(260.0 + 28.0, true);
        assert_eq!(r.width, 272.0);
        r.moved(260.0 + 29.0, true);
        assert_eq!(r.width, 289.0);
        r.moved(260.0 + 28.0, false);
        assert_eq!(r.width, 288.0);
        assert_eq!(
            r.up(None),
            Effect {
                rail: None,
                persist: Some(288.0)
            }
        );
        assert_eq!(
            r.reset(true),
            Effect {
                rail: Some(false),
                persist: Some(260.0)
            }
        );
        assert_eq!(
            r.reset(false),
            Effect {
                rail: None,
                persist: Some(260.0)
            }
        );
    }
}
