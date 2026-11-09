//! `conversationSearch.pure.ts`: title matching, the visible text of a message, its preview
//! and snippet, and the merged, sorted results.
#![cfg_attr(
    not(test),
    expect(dead_code, reason = "searchConversations calls it in 8.7c1c")
)]

mod forwarded;
pub mod fts;
pub mod fuzzy;
pub mod history;
pub mod mcp_app;
pub mod merge;
pub mod snippet;
pub mod visible;

/// `normalizeConversationContentPreview`'s and `textPreview`'s default length, in code points.
pub const PREVIEW_CHARS: usize = 180;

#[cfg(test)]
mod tests;
