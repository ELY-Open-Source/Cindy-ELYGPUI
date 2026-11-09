//! Conversation search as Cindy's main process runs it: `searchConversations` over
//! `conversationSearch.pure.ts` and `chatHistorySearch`'s FTS pass.
#![cfg_attr(
    not(test),
    expect(dead_code, reason = "the results list reads the rest in 8.7c2b")
)]

pub mod conversations;
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
