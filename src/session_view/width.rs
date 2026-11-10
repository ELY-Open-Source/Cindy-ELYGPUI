//! `useProportionalWidth(914)`: the message and composer widths from the container's.

/// The view's `maxWidth`.
const CAP: f32 = 914.0;
/// `INPUT_OUTSET`: the composer runs 10px past the messages on each side.
const INPUT_OUTSET: f32 = 20.0;
const MESSAGE_PAD: f32 = 50.0;
const COMPACT_MESSAGE_PAD: f32 = 20.0;
/// `AUTO_COMPACT_THRESHOLD`.
const COMPACT_UNDER: f32 = 700.0;

/// The hook's CSS variables, in CSS pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Widths {
    pub message: f32,
    pub input: f32,
    pub input_pad: f32,
    pub compact: bool,
}

/// `compute`: nothing for a container with no width.
pub fn widths(container: f32) -> Option<Widths> {
    if container <= 0.0 {
        return None;
    }
    let compact = container < COMPACT_UNDER;
    let pad = if compact {
        COMPACT_MESSAGE_PAD
    } else {
        MESSAGE_PAD
    };
    let message = CAP.min((container - pad * 2.0).max(0.0));
    Some(Widths {
        message,
        input: message + INPUT_OUTSET,
        input_pad: (pad - INPUT_OUTSET / 2.0).max(0.0),
        compact,
    })
}

#[cfg(test)]
mod tests {
    use super::{Widths, widths};

    fn at(container: f32) -> (f32, f32, f32, bool) {
        let Widths {
            message,
            input,
            input_pad,
            compact,
        } = widths(container).expect("a width");
        (message, input, input_pad, compact)
    }

    #[test]
    fn the_widths_follow_the_container() {
        assert_eq!(widths(0.0), None);
        assert_eq!(widths(-4.0), None);
        // Compact under 700: 20px beside the messages, 10 beside the composer.
        assert_eq!(at(340.0), (300.0, 320.0, 10.0, true));
        assert_eq!(at(699.5), (659.5, 679.5, 10.0, true));
        assert_eq!(at(30.0), (0.0, 20.0, 10.0, true));
        // From 700 on: 50 and 40.
        assert_eq!(at(700.0), (600.0, 620.0, 40.0, false));
        assert_eq!(at(1014.0), (914.0, 934.0, 40.0, false));
        // Past the cap the column centres.
        assert_eq!(at(1600.0), (914.0, 934.0, 40.0, false));
    }
}
