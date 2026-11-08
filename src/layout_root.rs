//! Cindy's `LayoutRoot.tsx`: the layout tree on screen.

#![cfg_attr(not(test), expect(dead_code, reason = "the view lands in 6.3b"))]

pub mod divider;
pub mod geometry;
#[cfg(test)]
mod tests;
