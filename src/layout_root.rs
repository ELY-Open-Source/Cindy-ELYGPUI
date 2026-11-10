//! Cindy's `LayoutRoot.tsx`: the layout tree on screen.

pub mod divider;
pub mod geometry;
#[cfg(test)]
mod panel_tests;
#[cfg(test)]
pub(crate) mod screen_tests;
#[cfg(test)]
mod tests;
pub(super) mod view;

pub use view::{LayoutRoot, SidebarSeen};
