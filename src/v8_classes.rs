//! Character classes as V8 holds them, in the ranges `scripts/sync-golden.mjs` writes.

/// Whether sorted, inclusive `ranges` hold `ch`.
pub fn within(ranges: &[[u32; 2]], ch: char) -> bool {
    let cp = u32::from(ch);
    let at = ranges.partition_point(|[_, end]| *end < cp);
    ranges.get(at).is_some_and(|[start, _]| *start <= cp)
}
