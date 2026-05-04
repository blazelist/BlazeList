//! Tag implication graph utilities for clients.
//!
//! Re-exports the authoritative [`TagGraph`] and helpers from
//! [`blazelist_protocol::tag::graph`] so WASM and other clients can
//! import them through `blazelist_client_lib::tag_graph` without
//! reaching across crate boundaries. The types themselves are
//! defined in the protocol crate so the server can reuse them for
//! push validation without depending on this client library.
//!
//! The client-side use cases are:
//!
//! - Computing the affected-card set when the user edits a tag's
//!   `implies` list in the tag detail view (so the UI can show
//!   "this will update N cards" before submitting the batch).
//! - Cascading transitively-implied tags into the card editor's
//!   `selected_tags` when the user picks a parent-implying tag.
//! - Blocking a chip-remove that would leave a card missing a
//!   still-implied tag.

pub use blazelist_protocol::{TagGraph, TagImplicationCycle, affected_cards_for_change};

#[cfg(test)]
mod tests {
    use super::*;
    use blazelist_protocol::{Card, Tag};
    use chrono::{DateTime, Utc};
    use uuid::Uuid;

    const A: Uuid = Uuid::from_bytes([0xAA; 16]);
    const B: Uuid = Uuid::from_bytes([0xBB; 16]);
    const C: Uuid = Uuid::from_bytes([0xCC; 16]);

    fn ts(ms: i64) -> DateTime<Utc> {
        DateTime::from_timestamp_millis(ms).unwrap()
    }

    fn tag_with(id: Uuid, implies: Vec<Uuid>) -> Tag {
        Tag::first_with_implies(id, "t".into(), None, implies, ts(0))
    }

    #[test]
    fn affected_cards_for_change_skips_unchanged_cards() {
        let next = TagGraph::from_tags(&[tag_with(A, vec![B]), tag_with(B, vec![])]);
        // Card already has both A and B — nothing to change.
        let card = Card::first(
            Uuid::from_bytes([0xEE; 16]),
            "c".into(),
            1,
            vec![A, B],
            false,
            ts(0),
            None,
        );
        let affected = affected_cards_for_change(&next, &[card]);
        assert!(affected.is_empty());
    }

    #[test]
    fn affected_cards_for_change_only_includes_referencing_cards() {
        let next = TagGraph::from_tags(&[tag_with(A, vec![B]), tag_with(B, vec![])]);

        // Card 1 references A without B → needs B added.
        let needs = Card::first(
            Uuid::from_bytes([0xEE; 16]),
            "needs".into(),
            1,
            vec![A],
            false,
            ts(0),
            None,
        );
        // Card 2 has no relevant tags → untouched.
        let untouched = Card::first(
            Uuid::from_bytes([0xFF; 16]),
            "untouched".into(),
            2,
            vec![C],
            false,
            ts(0),
            None,
        );

        let affected = affected_cards_for_change(&next, &[needs, untouched]);
        assert_eq!(affected.len(), 1);
        assert_eq!(affected[0].1, vec![B]);
    }

    #[test]
    fn closure_of_reuses_protocol_helper() {
        let graph = TagGraph::from_tags(&[
            tag_with(A, vec![B]),
            tag_with(B, vec![C]),
            tag_with(C, vec![]),
        ]);
        let closure = graph.closure_of(&[A]);
        assert_eq!(closure.into_iter().collect::<Vec<_>>(), vec![A, B, C]);
    }
}
