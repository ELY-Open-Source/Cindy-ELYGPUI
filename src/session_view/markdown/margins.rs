//! CSS margin collapsing for a column of blocks, in CSS pixels.

/// A block's vertical margins; a `hollow` block has no height, so its two margins meet.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Margins {
    pub top: f32,
    pub bottom: f32,
    pub hollow: bool,
}

/// A container's children once their margins collapsed.
#[derive(Debug, PartialEq)]
pub struct Stack {
    /// The space before each child that has a height.
    pub gaps: Vec<Option<f32>>,
    /// The margins that leave an open container through its edges.
    pub top: f32,
    pub bottom: f32,
    /// The space a closed container keeps after its last child.
    pub tail: f32,
    /// Whether no child has a height.
    pub empty: bool,
}

/// Adjoining margins collapse to the largest; an `open` container has no padding or border.
pub fn collapse(children: &[Margins], open: bool) -> Stack {
    let mut gaps = Vec::with_capacity(children.len());
    let (mut carry, mut top, mut empty) = (0.0_f32, 0.0, true);
    for child in children {
        if child.hollow {
            carry = carry.max(child.top).max(child.bottom);
            gaps.push(None);
            continue;
        }
        let gap = carry.max(child.top);
        if empty && open {
            top = gap;
            gaps.push(Some(0.0));
        } else {
            gaps.push(Some(gap));
        }
        (carry, empty) = (child.bottom, false);
    }
    if empty && open {
        top = carry;
    }
    let (bottom, tail) = if open { (carry, 0.0) } else { (0.0, carry) };
    Stack {
        gaps,
        top,
        bottom,
        tail,
        empty,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn block(top: f32, bottom: f32) -> Margins {
        Margins {
            top,
            bottom,
            hollow: false,
        }
    }

    #[test]
    fn siblings_keep_the_larger_margin() {
        let stack = collapse(&[block(12.0, 12.0), block(8.0, 8.0), block(8.0, 0.0)], true);
        assert_eq!(stack.gaps, [Some(0.0), Some(12.0), Some(8.0)]);
        assert_eq!((stack.top, stack.bottom, stack.tail), (12.0, 0.0, 0.0));
    }

    #[test]
    fn a_closed_container_keeps_its_childrens_margins_inside() {
        let stack = collapse(&[block(8.0, 8.0)], false);
        assert_eq!(stack.gaps, [Some(8.0)]);
        assert_eq!((stack.top, stack.bottom, stack.tail), (0.0, 0.0, 8.0));
    }

    #[test]
    fn a_hollow_block_joins_the_margins_around_it() {
        let hollow = Margins {
            top: 16.0,
            bottom: 16.0,
            hollow: true,
        };
        let stack = collapse(&[block(0.0, 8.0), hollow, block(8.0, 0.0)], true);
        assert_eq!(stack.gaps, [Some(0.0), None, Some(16.0)]);
        let alone = collapse(&[hollow], true);
        assert!(alone.empty);
        assert_eq!((alone.top, alone.bottom), (16.0, 16.0));
    }
}
