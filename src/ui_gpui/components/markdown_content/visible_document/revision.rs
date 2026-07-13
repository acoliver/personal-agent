//! Message-revision freshness token.
//!
//! A [`MessageRevision`] captures all the dimensions that can change a
//! message's visible text: the message id, the displayed content, the
//! streaming revision, and the emoji-filter flag. A selection is never reused
//! after any of these change (REQ-151-007).

use std::fmt;

/// A compact fingerprint that uniquely identifies the visible content of a
/// single message at a point in time.
///
/// Two revisions are equal only when *every* component matches. This is the
/// mechanism that prevents stale selections from being reused after a
/// conversation, message edit, streaming update, or emoji-filter toggle.
///
/// @plan PLAN-20260713-ISSUE151 Phase 1
/// @requirement REQ-151-007
#[derive(Clone)]
pub struct MessageRevision {
    message_id: String,
    content_hash: u64,
    streaming_revision: u64,
    emoji_filtered: bool,
}

impl MessageRevision {
    /// Create a new revision for the given message.
    ///
    /// `content` is the raw displayed content (typically the markdown source)
    /// used to detect content changes. `streaming_revision` increments during
    /// streaming so each delta invalidates the previous revision.
    #[must_use]
    pub fn new(
        message_id: &str,
        content: &str,
        streaming_revision: u64,
        emoji_filtered: bool,
    ) -> Self {
        Self {
            message_id: message_id.to_string(),
            content_hash: fnv1a_64(content.as_bytes()),
            streaming_revision,
            emoji_filtered,
        }
    }

    /// Return `true` when this revision exactly matches the supplied current
    /// parameters.
    ///
    /// Used to reject stale selections: a selection built against revision *R*
    /// is only valid while `R.is_current(...)` remains true.
    #[must_use]
    pub fn is_current(
        &self,
        message_id: &str,
        content: &str,
        streaming_revision: u64,
        emoji_filtered: bool,
    ) -> bool {
        self.message_id == message_id
            && self.content_hash == fnv1a_64(content.as_bytes())
            && self.streaming_revision == streaming_revision
            && self.emoji_filtered == emoji_filtered
    }

    /// Stable hash of the content for equality comparison.
    #[must_use]
    pub fn hash(&self) -> u64 {
        self.content_hash
    }
}

impl PartialEq for MessageRevision {
    fn eq(&self, other: &Self) -> bool {
        self.message_id == other.message_id
            && self.content_hash == other.content_hash
            && self.streaming_revision == other.streaming_revision
            && self.emoji_filtered == other.emoji_filtered
    }
}

impl Eq for MessageRevision {}

impl fmt::Debug for MessageRevision {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MessageRevision")
            .field("message_id", &self.message_id)
            .field("content_hash", &self.content_hash)
            .field("streaming_revision", &self.streaming_revision)
            .field("emoji_filtered", &self.emoji_filtered)
            .finish()
    }
}

/// FNV-1a 64-bit hash — a small, dependency-free, well-distributed hash.
///
/// No cryptographic strength is needed here; this is purely for change
/// detection.
fn fnv1a_64(data: &[u8]) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for &byte in data {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0100_0000_01b3);
    }
    hash
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fnv1a_is_deterministic() {
        assert_eq!(fnv1a_64(b"hello"), fnv1a_64(b"hello"));
        assert_ne!(fnv1a_64(b"hello"), fnv1a_64(b"world"));
    }
}
