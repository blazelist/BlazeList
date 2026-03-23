//! Sync helpers shared across BlazeList clients.
//!
//! Both the native CLI and WASM client perform incremental sync by applying
//! a [`ChangeSet`] to their local card/tag collections. This module provides
//! the shared logic for that operation.

use blazelist_protocol::ChangeSet;
use blazelist_protocol::{Card, Entity, Tag, ZERO_HASH};
use indexmap::IndexMap;
use uuid::Uuid;

/// Apply a [`ChangeSet`] to a local card collection.
///
/// This performs the standard incremental sync merge:
/// 1. Index current cards by ID
/// 2. Remove deleted entities
/// 3. Upsert changed/new cards
/// 4. Sort by priority descending
///
/// Returns the merged card list, sorted by priority (highest first).
pub fn apply_card_changeset(current_cards: Vec<Card>, changes: &ChangeSet) -> Vec<Card> {
    let mut cards: IndexMap<Uuid, Card> = current_cards.into_iter().map(|c| (c.id(), c)).collect();

    // Remove deleted entities
    for deleted in &changes.deleted {
        cards.swap_remove(&deleted.id());
    }
    for card in &changes.cards {
        cards.insert(card.id(), card.clone());
    }

    // Sort in-place by priority descending (keys ignored, values compared), then collect
    cards.sort_unstable_by(|_, a, _, b| b.priority().cmp(&a.priority()));
    cards.into_values().collect()
}

/// Apply a [`ChangeSet`] to a local tag collection.
///
/// Performs the same merge pattern as cards:
/// 1. Remove deleted entities
/// 2. Upsert changed/new tags
pub fn apply_tag_changeset(current_tags: Vec<Tag>, changes: &ChangeSet) -> Vec<Tag> {
    let mut tags: IndexMap<Uuid, Tag> = current_tags.into_iter().map(|t| (t.id(), t)).collect();

    // Remove deleted entities
    for deleted in &changes.deleted {
        tags.swap_remove(&deleted.id());
    }

    // Upsert changed tags
    for tag in &changes.tags {
        tags.insert(tag.id(), tag.clone());
    }

    tags.into_values().collect()
}

/// Reconcile an offline queue against the current local card state.
///
/// Returns only the cards that should still be pushed to the server:
///
/// - **Brand-new cards** (`ancestor_hash == ZERO_HASH`) are always kept —
///   they only exist locally because the client inserted them optimistically
///   and have never been confirmed by the server.
/// - **Stale edits** whose local version is *strictly newer* than the queued
///   version are dropped (the server already has a newer state).
/// - Everything else is kept for pushing.
pub fn reconcile_offline_queue(queue: Vec<Card>, local_cards: &[Card]) -> Vec<Card> {
    queue
        .into_iter()
        .filter(|queued| {
            if queued.ancestor_hash() == ZERO_HASH {
                return true;
            }
            !local_cards
                .iter()
                .any(|lc| lc.id() == queued.id() && lc.count() > queued.count())
        })
        .collect()
}

/// Trim trailing whitespace from card content.
///
/// Trims the entire string, then trims trailing whitespace from each line.
/// Used when saving card content from editors in both CLI and WASM.
pub fn trim_content(raw: &str) -> String {
    let trimmed = raw.trim();
    trimmed
        .lines()
        .map(|line| line.trim_end())
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::{fixed_time, fixed_uuid, priority};
    use blazelist_protocol::{DeletedEntity, NonNegativeI64, RootState};

    fn stub_root() -> RootState {
        RootState {
            sequence: NonNegativeI64::try_from(1i64).unwrap(),
            hash: blake3::hash(b"test"),
        }
    }

    fn sample_cards() -> Vec<Card> {
        vec![
            Card::first(
                fixed_uuid(1),
                "First".into(),
                priority(3000),
                vec![],
                false,
                fixed_time(),
                None,
            ),
            Card::first(
                fixed_uuid(2),
                "Second".into(),
                priority(2000),
                vec![],
                false,
                fixed_time(),
                None,
            ),
            Card::first(
                fixed_uuid(3),
                "Third".into(),
                priority(1000),
                vec![],
                false,
                fixed_time(),
                None,
            ),
        ]
    }

    fn sample_tags() -> Vec<Tag> {
        vec![
            Tag::first(fixed_uuid(10), "work".into(), None, fixed_time()),
            Tag::first(fixed_uuid(11), "personal".into(), None, fixed_time()),
        ]
    }

    #[test]
    fn apply_card_changeset_upsert() {
        let cards = sample_cards();
        let updated = Card::first(
            fixed_uuid(2),
            "Updated second".into(),
            priority(2000),
            vec![],
            false,
            fixed_time(),
            None,
        );
        let changes = ChangeSet {
            cards: vec![updated],
            tags: vec![],
            deleted: vec![],
            root: stub_root(),
        };

        let result = apply_card_changeset(cards, &changes);
        assert_eq!(result.len(), 3);
        let card2 = result.iter().find(|c| c.id() == fixed_uuid(2)).unwrap();
        assert_eq!(card2.content(), "Updated second");
    }

    #[test]
    fn apply_card_changeset_delete() {
        let cards = sample_cards();
        let changes = ChangeSet {
            cards: vec![],
            tags: vec![],
            deleted: vec![DeletedEntity::new(fixed_uuid(2))],
            root: stub_root(),
        };

        let result = apply_card_changeset(cards, &changes);
        assert_eq!(result.len(), 2);
        assert!(result.iter().all(|c| c.id() != fixed_uuid(2)));
    }

    #[test]
    fn apply_card_changeset_add_new() {
        let cards = sample_cards();
        let new_card = Card::first(
            fixed_uuid(4),
            "New card".into(),
            priority(4000),
            vec![],
            false,
            fixed_time(),
            None,
        );
        let changes = ChangeSet {
            cards: vec![new_card],
            tags: vec![],
            deleted: vec![],
            root: stub_root(),
        };

        let result = apply_card_changeset(cards, &changes);
        assert_eq!(result.len(), 4);
        // Should be sorted by priority descending, so new card (4000) is first
        assert_eq!(result[0].content(), "New card");
    }

    #[test]
    fn apply_card_changeset_sorted_by_priority() {
        let cards = sample_cards();
        let changes = ChangeSet {
            cards: vec![],
            tags: vec![],
            deleted: vec![],
            root: stub_root(),
        };

        let result = apply_card_changeset(cards, &changes);
        for window in result.windows(2) {
            assert!(window[0].priority() >= window[1].priority());
        }
    }

    #[test]
    fn apply_card_changeset_empty() {
        let changes = ChangeSet {
            cards: vec![],
            tags: vec![],
            deleted: vec![],
            root: stub_root(),
        };

        let result = apply_card_changeset(vec![], &changes);
        assert!(result.is_empty());
    }

    #[test]
    fn apply_tag_changeset_upsert() {
        let tags = sample_tags();
        let updated = Tag::first(fixed_uuid(10), "engineering".into(), None, fixed_time());
        let changes = ChangeSet {
            cards: vec![],
            tags: vec![updated],
            deleted: vec![],
            root: stub_root(),
        };

        let result = apply_tag_changeset(tags, &changes);
        assert_eq!(result.len(), 2);
        let tag10 = result.iter().find(|t| t.id() == fixed_uuid(10)).unwrap();
        assert_eq!(tag10.title(), "engineering");
    }

    #[test]
    fn apply_tag_changeset_delete() {
        let tags = sample_tags();
        let changes = ChangeSet {
            cards: vec![],
            tags: vec![],
            deleted: vec![DeletedEntity::new(fixed_uuid(11))],
            root: stub_root(),
        };

        let result = apply_tag_changeset(tags, &changes);
        assert_eq!(result.len(), 1);
        assert!(result.iter().all(|t| t.id() != fixed_uuid(11)));
    }

    #[test]
    fn trim_content_basic() {
        assert_eq!(trim_content("  hello  "), "hello");
    }

    #[test]
    fn trim_content_trailing_spaces_per_line() {
        assert_eq!(trim_content("line1   \nline2  "), "line1\nline2");
    }

    #[test]
    fn trim_content_preserves_leading_spaces_per_line() {
        assert_eq!(trim_content("  line1\n  line2"), "line1\n  line2");
    }

    #[test]
    fn trim_content_empty() {
        assert_eq!(trim_content(""), "");
        assert_eq!(trim_content("   "), "");
    }

    // --- Tests for changeset merge behavior during reconnection ---
    // When the connection drops and reconnects, an incremental sync merges
    // server changes into the local state via apply_card/tag_changeset.
    // These tests verify that the merge preserves unrelated local data.

    #[test]
    fn apply_card_changeset_preserves_unrelated_cards() {
        let cards = sample_cards();
        // Server only updates card 2 — cards 1 and 3 must survive unchanged
        let updated = Card::first(
            fixed_uuid(2),
            "Server updated".into(),
            priority(2000),
            vec![],
            false,
            fixed_time(),
            None,
        );
        let changes = ChangeSet {
            cards: vec![updated],
            tags: vec![],
            deleted: vec![],
            root: stub_root(),
        };

        let result = apply_card_changeset(cards, &changes);
        assert_eq!(result.len(), 3);
        assert!(result.iter().any(|c| c.content() == "First"));
        assert!(result.iter().any(|c| c.content() == "Third"));
        assert!(result.iter().any(|c| c.content() == "Server updated"));
    }

    #[test]
    fn apply_card_changeset_simultaneous_add_and_delete() {
        let cards = sample_cards();
        let new_card = Card::first(
            fixed_uuid(5),
            "Added by server".into(),
            priority(500),
            vec![],
            false,
            fixed_time(),
            None,
        );
        let changes = ChangeSet {
            cards: vec![new_card],
            tags: vec![],
            deleted: vec![DeletedEntity::new(fixed_uuid(1))],
            root: stub_root(),
        };

        let result = apply_card_changeset(cards, &changes);
        assert_eq!(result.len(), 3);
        assert!(result.iter().all(|c| c.id() != fixed_uuid(1)));
        assert!(result.iter().any(|c| c.content() == "Added by server"));
    }

    #[test]
    fn apply_tag_changeset_add_new() {
        let tags = sample_tags();
        let new_tag = Tag::first(fixed_uuid(12), "urgent".into(), None, fixed_time());
        let changes = ChangeSet {
            cards: vec![],
            tags: vec![new_tag],
            deleted: vec![],
            root: stub_root(),
        };

        let result = apply_tag_changeset(tags, &changes);
        assert_eq!(result.len(), 3);
        assert!(result.iter().any(|t| t.title() == "urgent"));
    }

    #[test]
    fn apply_tag_changeset_preserves_unrelated_tags() {
        let tags = sample_tags();
        // Server updates tag 10 — tag 11 must survive
        let updated = Tag::first(fixed_uuid(10), "engineering".into(), None, fixed_time());
        let changes = ChangeSet {
            cards: vec![],
            tags: vec![updated],
            deleted: vec![],
            root: stub_root(),
        };

        let result = apply_tag_changeset(tags, &changes);
        assert_eq!(result.len(), 2);
        assert!(result.iter().any(|t| t.title() == "personal"));
        assert!(result.iter().any(|t| t.title() == "engineering"));
    }

    #[test]
    fn apply_tag_changeset_simultaneous_add_and_delete() {
        let tags = sample_tags();
        let new_tag = Tag::first(fixed_uuid(13), "new-tag".into(), None, fixed_time());
        let changes = ChangeSet {
            cards: vec![],
            tags: vec![new_tag],
            deleted: vec![DeletedEntity::new(fixed_uuid(10))],
            root: stub_root(),
        };

        let result = apply_tag_changeset(tags, &changes);
        assert_eq!(result.len(), 2);
        assert!(result.iter().all(|t| t.id() != fixed_uuid(10)));
        assert!(result.iter().any(|t| t.title() == "personal"));
        assert!(result.iter().any(|t| t.title() == "new-tag"));
    }

    #[test]
    fn apply_card_changeset_delete_nonexistent_is_no_op() {
        let cards = sample_cards();
        let changes = ChangeSet {
            cards: vec![],
            tags: vec![],
            deleted: vec![DeletedEntity::new(fixed_uuid(99))],
            root: stub_root(),
        };

        let result = apply_card_changeset(cards, &changes);
        assert_eq!(result.len(), 3);
    }

    #[test]
    fn apply_tag_changeset_delete_nonexistent_is_no_op() {
        let tags = sample_tags();
        let changes = ChangeSet {
            cards: vec![],
            tags: vec![],
            deleted: vec![DeletedEntity::new(fixed_uuid(99))],
            root: stub_root(),
        };

        let result = apply_tag_changeset(tags, &changes);
        assert_eq!(result.len(), 2);
    }

    // --- Offline queue reconciliation tests ---

    #[test]
    fn reconcile_keeps_new_card_with_zero_hash_ancestor() {
        // A brand-new card (ancestor == ZERO_HASH) must always be pushed,
        // even if local state already has the same card at the same count.
        let card = Card::first(
            fixed_uuid(1),
            "offline card".into(),
            priority(1000),
            vec![],
            false,
            fixed_time(),
            None,
        );
        let local_cards = vec![card.clone()];
        let queue = vec![card];

        let result = reconcile_offline_queue(queue, &local_cards);
        assert_eq!(result.len(), 1, "new card must not be dropped");
    }

    #[test]
    fn reconcile_keeps_edit_at_same_count() {
        // If the local card has the same count as the queued edit, the
        // queued version might not have been pushed yet — keep it.
        let original = Card::first(
            fixed_uuid(1),
            "v1".into(),
            priority(1000),
            vec![],
            false,
            fixed_time(),
            None,
        );
        let edited = original.next(
            "v2".into(),
            priority(1000),
            vec![],
            false,
            fixed_time(),
            None,
        );
        let local_cards = vec![edited.clone()];
        let queue = vec![edited];

        let result = reconcile_offline_queue(queue, &local_cards);
        assert_eq!(result.len(), 1, "same-count edit must not be dropped");
    }

    #[test]
    fn reconcile_drops_stale_edit() {
        // The server (via incremental sync) advanced the card beyond the
        // queued version — safe to drop.
        let v1 = Card::first(
            fixed_uuid(1),
            "v1".into(),
            priority(1000),
            vec![],
            false,
            fixed_time(),
            None,
        );
        let v2 = v1.next(
            "v2".into(),
            priority(1000),
            vec![],
            false,
            fixed_time(),
            None,
        );
        let v3 = v2.next(
            "v3".into(),
            priority(1000),
            vec![],
            false,
            fixed_time(),
            None,
        );
        // Local state has v3, queue still has v2
        let local_cards = vec![v3];
        let queue = vec![v2];

        let result = reconcile_offline_queue(queue, &local_cards);
        assert!(result.is_empty(), "stale edit must be dropped");
    }

    #[test]
    fn reconcile_keeps_card_not_in_local_state() {
        // Card exists in queue but not in local state (e.g. after a full
        // re-sync wiped local state).
        let card = Card::first(
            fixed_uuid(1),
            "orphan".into(),
            priority(1000),
            vec![],
            false,
            fixed_time(),
            None,
        )
        .next(
            "orphan v2".into(),
            priority(1000),
            vec![],
            false,
            fixed_time(),
            None,
        );
        let local_cards: Vec<Card> = vec![];
        let queue = vec![card];

        let result = reconcile_offline_queue(queue, &local_cards);
        assert_eq!(result.len(), 1, "card not in local state must be kept");
    }

    #[test]
    fn reconcile_empty_queue() {
        let local_cards = sample_cards();
        let result = reconcile_offline_queue(vec![], &local_cards);
        assert!(result.is_empty());
    }

    #[test]
    fn reconcile_mixed_queue() {
        // Mix of new card, stale edit, and current edit.
        let new_card = Card::first(
            fixed_uuid(10),
            "new".into(),
            priority(5000),
            vec![],
            false,
            fixed_time(),
            None,
        );

        let v1 = Card::first(
            fixed_uuid(20),
            "v1".into(),
            priority(4000),
            vec![],
            false,
            fixed_time(),
            None,
        );
        let v2 = v1.next(
            "v2".into(),
            priority(4000),
            vec![],
            false,
            fixed_time(),
            None,
        );
        let v3 = v2.next(
            "v3".into(),
            priority(4000),
            vec![],
            false,
            fixed_time(),
            None,
        );

        let current_edit = Card::first(
            fixed_uuid(30),
            "current".into(),
            priority(3000),
            vec![],
            false,
            fixed_time(),
            None,
        )
        .next(
            "current v2".into(),
            priority(3000),
            vec![],
            false,
            fixed_time(),
            None,
        );

        // Local: new_card at count=1, card 20 at v3, card 30 at count=2
        let local_cards = vec![new_card.clone(), v3, current_edit.clone()];
        // Queue: new_card (keep), v2 of card 20 (stale, drop), current_edit (keep)
        let queue = vec![new_card, v2, current_edit];

        let result = reconcile_offline_queue(queue, &local_cards);
        assert_eq!(result.len(), 2);
        assert!(result.iter().any(|c| c.id() == fixed_uuid(10)));
        assert!(result.iter().any(|c| c.id() == fixed_uuid(30)));
        assert!(!result.iter().any(|c| c.id() == fixed_uuid(20)));
    }
}
