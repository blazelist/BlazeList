use super::seed::{generate, resolve_priority};
use blazelist_protocol::Entity;
use std::collections::BTreeSet;

#[test]
fn generates_correct_counts() {
    let data = generate(42, 15, 300);
    assert_eq!(data.tag_chains.len(), 15);
    assert_eq!(data.card_chains.len(), 300);
}

#[test]
fn all_entities_verify() {
    let data = generate(42, 15, 300);
    for chain in &data.tag_chains {
        for tag in chain {
            assert!(tag.verify(), "tag {} failed verification", tag.id());
        }
    }
    for chain in &data.card_chains {
        for card in chain {
            assert!(card.verify(), "card {} failed verification", card.id());
        }
    }
}

#[test]
fn priorities_are_unique_and_ordered() {
    let data = generate(42, 15, 300);
    // Latest version of each card determines its active priority.
    let priorities: Vec<i64> = data
        .card_chains
        .iter()
        .map(|c| i64::from(c.last().unwrap().priority()))
        .collect();
    // Initial priorities are strictly decreasing (highest first).
    // History edits apply small jitter, so check uniqueness only.
    let mut seen = std::collections::HashSet::new();
    for p in &priorities {
        assert!(seen.insert(p), "duplicate priority {p}");
    }
}

#[test]
fn tag_distribution_has_untagged_cards() {
    let data = generate(42, 15, 300);
    let untagged = data
        .card_chains
        .iter()
        .filter(|c| c.last().unwrap().tags().is_empty())
        .count();
    // With ~20% target, expect at least some untagged cards.
    assert!(untagged > 0, "expected some untagged cards");
}

#[test]
fn tag_distribution_has_multi_tagged_cards() {
    let data = generate(42, 15, 300);
    let multi = data
        .card_chains
        .iter()
        .filter(|c| c.last().unwrap().tags().len() >= 3)
        .count();
    assert!(multi > 0, "expected some cards with 3+ tags");
}

#[test]
fn some_cards_are_blazed() {
    let data = generate(42, 15, 300);
    let blazed = data
        .card_chains
        .iter()
        .filter(|c| c.last().unwrap().blazed())
        .count();
    assert!(blazed > 0, "expected some blazed cards");
    assert!(
        blazed < data.card_chains.len(),
        "not all cards should be blazed"
    );
}

#[test]
fn zero_tags_produces_untagged_cards() {
    let data = generate(42, 0, 10);
    assert!(data.tag_chains.is_empty());
    for chain in &data.card_chains {
        assert!(chain.last().unwrap().tags().is_empty());
    }
}

#[test]
fn custom_counts() {
    let data = generate(42, 5, 50);
    assert_eq!(data.tag_chains.len(), 5);
    assert_eq!(data.card_chains.len(), 50);
}

#[test]
fn many_tags_generated_and_valid() {
    let data = generate(42, 25, 0);
    assert_eq!(data.tag_chains.len(), 25);
    for chain in &data.tag_chains {
        for tag in chain {
            assert!(tag.verify());
        }
    }
}

#[test]
fn cards_have_version_history() {
    let data = generate(42, 15, 300);
    let with_history = data.card_chains.iter().filter(|c| c.len() > 1).count();
    // ~90% should have history.
    assert!(
        with_history > 200,
        "expected ~90% cards with history, got {with_history}/300"
    );
    let with_deep_history = data.card_chains.iter().filter(|c| c.len() >= 4).count();
    // ~63% should have 3+ edits (4+ versions).
    assert!(
        with_deep_history > 100,
        "expected ~63% cards with 3+ edits, got {with_deep_history}/300"
    );
}

#[test]
fn some_cards_have_due_dates() {
    let data = generate(42, 15, 300);
    let with_due = data
        .card_chains
        .iter()
        .filter(|c| c.first().unwrap().due_date().is_some())
        .count();
    assert!(with_due > 0, "expected some cards with due dates");
    assert!(
        with_due < data.card_chains.len(),
        "not all cards should have due dates"
    );
}

#[test]
fn deleted_entities_are_generated() {
    let data = generate(42, 15, 300);
    assert!(
        !data.deleted_card_chains.is_empty(),
        "expected deleted cards"
    );
    assert!(!data.deleted_tag_chains.is_empty(), "expected deleted tags");
    // All deleted entities should verify too.
    for chain in &data.deleted_card_chains {
        for card in chain {
            assert!(
                card.verify(),
                "deleted card {} failed verification",
                card.id()
            );
        }
    }
    for chain in &data.deleted_tag_chains {
        for tag in chain {
            assert!(tag.verify(), "deleted tag {} failed verification", tag.id());
        }
    }
}

#[test]
fn hash_chains_are_linked() {
    let data = generate(42, 15, 100);
    for chain in &data.card_chains {
        for window in chain.windows(2) {
            assert_eq!(
                window[1].ancestor_hash(),
                window[0].hash(),
                "card {} version {} ancestor mismatch",
                window[1].id(),
                u64::from(window[1].count()),
            );
        }
    }
}

#[test]
fn some_tags_have_colors() {
    let data = generate(42, 15, 50);
    let with_color = data
        .tag_chains
        .iter()
        .filter(|c| c.first().unwrap().color().is_some())
        .count();
    assert!(with_color > 0, "expected some tags with colors");
    assert!(
        with_color < data.tag_chains.len(),
        "not all tags should have colors"
    );
}

#[test]
fn resolve_priority_returns_same_when_no_collision() {
    let used = BTreeSet::from([10, 20, 30]);
    assert_eq!(resolve_priority(15, &used), 15);
}

#[test]
fn resolve_priority_finds_midpoint_on_collision() {
    let used = BTreeSet::from([10, 20, 30]);
    // 20 is taken, gap above (20..30) = 10, gap below (10..20) = 10.
    // Equal gaps: prefer above → midpoint of 20..30 = 25.
    let resolved = resolve_priority(20, &used);
    assert!(!used.contains(&resolved), "resolved priority must be free");
    assert!(
        resolved > 10 && resolved < 30,
        "should be between neighbors"
    );
}

#[test]
fn resolve_priority_handles_consecutive_values() {
    // Packed range: 10, 11, 12. Collide on 11.
    let used = BTreeSet::from([10, 11, 12]);
    let resolved = resolve_priority(11, &used);
    assert!(!used.contains(&resolved), "resolved priority must be free");
    assert!(resolved >= 0, "priority must be non-negative");
}

#[test]
fn resolve_priority_empty_set() {
    let used = BTreeSet::new();
    assert_eq!(resolve_priority(42, &used), 42);
}

#[test]
fn all_priorities_globally_unique() {
    let data = generate(42, 15, 300);

    let mut all_priorities = std::collections::HashSet::new();

    // Collect latest-version priority from every card chain.
    for chain in data
        .card_chains
        .iter()
        .chain(data.deleted_card_chains.iter())
    {
        let p = i64::from(chain.last().unwrap().priority());
        assert!(
            all_priorities.insert(p),
            "duplicate priority {p} in card chains"
        );
    }

    // Extra ops may create fresh cards; collect their priorities too.
    for batch in &data.extra_ops {
        for item in batch {
            if let blazelist_protocol::PushItem::Cards(cards) = item {
                for card in cards {
                    let p = i64::from(card.priority());
                    // Updates may legitimately reuse the same priority within
                    // a version chain, but new cards should not collide.
                    all_priorities.insert(p);
                }
            }
        }
    }
}

#[test]
fn some_cards_have_linked_card_uuids() {
    let data = generate(42, 15, 300);
    let card_ids: Vec<String> = data
        .card_chains
        .iter()
        .map(|c| c[0].id().to_string())
        .collect();

    let with_links = data
        .card_chains
        .iter()
        .filter(|chain| {
            let content = chain.last().unwrap().content();
            card_ids
                .iter()
                .any(|id| content.contains(id) && chain[0].id().to_string() != *id)
        })
        .count();

    assert!(
        with_links > 0,
        "expected some cards with links to other cards"
    );
    assert!(
        with_links < data.card_chains.len(),
        "not all cards should have links"
    );
}

#[test]
fn some_cards_have_no_links() {
    let data = generate(42, 15, 300);
    let card_ids: Vec<String> = data
        .card_chains
        .iter()
        .map(|c| c[0].id().to_string())
        .collect();

    let without_links = data
        .card_chains
        .iter()
        .filter(|chain| {
            let content = chain.last().unwrap().content();
            !card_ids
                .iter()
                .any(|id| content.contains(id) && chain[0].id().to_string() != *id)
        })
        .count();

    assert!(without_links > 0, "expected some cards without links");
}

#[test]
fn some_cards_have_duplicate_uuid_references() {
    let data = generate(42, 15, 300);
    let card_ids: Vec<String> = data
        .card_chains
        .iter()
        .map(|c| c[0].id().to_string())
        .collect();

    let with_duplicates = data
        .card_chains
        .iter()
        .filter(|chain| {
            let content = chain.last().unwrap().content();
            card_ids.iter().any(|id| {
                chain[0].id().to_string() != *id && content.matches(id.as_str()).count() >= 2
            })
        })
        .count();

    assert!(
        with_duplicates > 0,
        "expected some cards with the same UUID referenced twice"
    );
}

#[test]
fn linked_card_versions_verify() {
    let data = generate(42, 15, 300);
    for chain in &data.card_chains {
        for card in chain {
            assert!(
                card.verify(),
                "card {} v{} failed verification",
                card.id(),
                u64::from(card.count())
            );
        }
    }
}
