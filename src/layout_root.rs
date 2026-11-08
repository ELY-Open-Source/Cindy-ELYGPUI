//! Cindy's `LayoutRoot.tsx`: the layout tree on screen.

pub mod divider;
pub mod geometry;
#[cfg(test)]
mod screen_tests;
#[cfg(test)]
mod tests;
mod view;

pub use view::LayoutRoot;
