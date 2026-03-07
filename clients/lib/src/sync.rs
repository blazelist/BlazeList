//! Sync helpers shared across BlazeList clients.
//!
//! Both the native CLI and WASM client perform incremental sync by applying
//! a [`ChangeSet`] to their local card/tag collections. This module provides
//! the shared logic for that operation.

use blazelist_protocol::ChangeSet;
use blazelist_protocol::{Card, Entity, Tag};
use std::collections::HashMap;
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
    let mut cards: HashMap<Uuid, Card> = current_cards.into_iter().map(|c| (c.id(), c)).collect();

    // Remove deleted entities
    for deleted in &changes.deleted {
        cards.remove(&deleted.id());
    }

    // Upsert changed cards
    for card in &changes.cards {
        cards.insert(card.id(), card.clone());
    }

    // Convert back to sorted vec
    let mut result: Vec<Card> = cards.into_values().collect();
    result.sort_by_key(|c| std::cmp::Reverse(c.priority()));
    result
}

/// Apply a [`ChangeSet`] to a local tag collection.
///
/// Performs the same merge pattern as cards:
/// 1. Remove deleted entities
/// 2. Upsert changed/new tags
pub fn apply_tag_changeset(current_tags: Vec<Tag>, changes: &ChangeSet) -> Vec<Tag> {
    let mut tags: HashMap<Uuid, Tag> = current_tags.into_iter().map(|t| (t.id(), t)).collect();

    // Remove deleted entities
    for deleted in &changes.deleted {
        tags.remove(&deleted.id());
    }

    // Upsert changed tags
    for tag in &changes.tags {
        tags.insert(tag.id(), tag.clone());
    }

    tags.into_values().collect()
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

    fn dummy_root() -> RootState {
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
            root: dummy_root(),
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
            root: dummy_root(),
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
            root: dummy_root(),
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
            root: dummy_root(),
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
            root: dummy_root(),
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
            root: dummy_root(),
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
            root: dummy_root(),
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
}
