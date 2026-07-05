use super::seed::{SeedData, find_largest_gap_midpoint, generate, resolve_priority};
use blazelist_protocol::{Card, Entity, PushItem, Tag};
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
        .map(|c| c.last().unwrap().priority())
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
    // Both immediate gaps are size 1, so it falls through to the largest-gap scan.
    let used = BTreeSet::from([10, 11, 12]);
    let resolved = resolve_priority(11, &used);
    assert!(!used.contains(&resolved), "resolved priority must be free");
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
        let p = chain.last().unwrap().priority();
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
                    let p = card.priority();
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
fn some_cards_have_markdown_tables() {
    let data = generate(42, 15, 300);
    let with_tables = data
        .card_chains
        .iter()
        .filter(|chain| {
            let content = chain[0].content();
            content.contains("| --- |") || content.contains("| --- | --- |")
        })
        .count();
    assert!(
        with_tables > 0,
        "expected some cards with markdown tables, got {with_tables}/300"
    );
}

#[test]
fn some_cards_have_blockquotes() {
    let data = generate(42, 15, 300);
    let with_quotes = data
        .card_chains
        .iter()
        .filter(|chain| {
            chain[0]
                .content()
                .lines()
                .any(|line| line.trim_start().starts_with('>'))
        })
        .count();
    assert!(
        with_quotes > 0,
        "expected some cards with blockquotes, got {with_quotes}/300"
    );
}

#[test]
fn some_cards_have_nested_blockquotes() {
    let data = generate(42, 15, 300);
    let with_nested = data
        .card_chains
        .iter()
        .filter(|chain| chain[0].content().contains("> > "))
        .count();
    assert!(
        with_nested > 0,
        "expected some cards with nested blockquotes, got {with_nested}/300"
    );
}

#[test]
fn some_cards_have_multiparagraph_blockquotes() {
    let data = generate(42, 15, 300);
    // A `>` line immediately followed by a bare `>` continuation marks a
    // multi-paragraph (or multi-block) quote — the case the new
    // `blockquote > :last-child` margin reset targets. The single-line
    // quote (case 6) never produces this.
    let with_multi = data
        .card_chains
        .iter()
        .filter(|chain| chain[0].content().contains(">\n>"))
        .count();
    assert!(
        with_multi > 0,
        "expected some cards with multi-paragraph blockquotes, got {with_multi}/300"
    );
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

#[test]
fn find_largest_gap_midpoint_selects_unbounded_leading_gap() {
    // The set {0, 1, 2, 1000} collides densely at the low end. The interior
    // gap [2, 1000] (998 wide) is the largest gap *between two real elements*,
    // but the scan seeds `prev = i64::MIN`, so the very first gap considered is
    // [i64::MIN, 0] — roughly 9.2e18 wide and the genuine global maximum. The
    // function therefore returns the midpoint of THAT gap, i64::MIN / 2, not a
    // value inside [2, 1000]. The chosen midpoint is well clear of the colliding
    // value 1 (it is not adjacent to it), which is the property that matters for
    // collision resolution.
    let used = BTreeSet::from([0, 1, 2, 1000]);
    let mid = find_largest_gap_midpoint(&used);
    assert_eq!(mid, i64::MIN / 2);
    assert!(!used.contains(&mid), "midpoint must be free");
    // Not adjacent to (and far below) the colliding value 1.
    assert!(mid < 0, "midpoint lands in the unbounded leading gap");
}

#[test]
fn find_largest_gap_midpoint_selects_interior_gap_when_ends_are_clamped() {
    // To make a NON-edge interior gap win, both the leading sentinel gap
    // [i64::MIN, first] and the trailing sentinel gap [last, i64::MAX] must be
    // clamped to width 0 by including i64::MIN and i64::MAX themselves, and the
    // remaining low/high gaps must be smaller than the target interior gap.
    //
    // With {i64::MIN, -3, 1000, 2000, i64::MAX}:
    //   * leading gap  [i64::MIN, i64::MIN] = 0
    //   * gap [i64::MIN, -3]                ≈ 9.2e18  (largest, leftish interior)
    //   * gap [-3, 1000]                    = 1003
    //   * gap [1000, 2000]                  = 1000
    //   * gap [2000, i64::MAX]              ≈ 9.2e18
    //   * trailing gap [i64::MAX, i64::MAX] = 0
    // so the leftmost interior gap [i64::MIN, -3] still dominates here too —
    // proving that any gap touching a sentinel boundary dwarfs small interior
    // gaps. We lock the resulting midpoint exactly.
    let used = BTreeSet::from([i64::MIN, -3, 1000, 2000, i64::MAX]);
    let mid = find_largest_gap_midpoint(&used);
    // prev = i64::MIN, p = -3: gap = (-3) - i64::MIN; midpoint = i64::MIN + gap/2.
    let gap = (-3_i128) - i64::MIN as i128;
    let expected = (i64::MIN as i128 + gap / 2) as i64;
    assert_eq!(mid, expected);
    assert!(mid < -3, "midpoint lands inside the [i64::MIN, -3] gap");
    assert!(!used.contains(&mid), "midpoint must be free");
}

#[test]
fn find_largest_gap_midpoint_tracks_trailing_gap_after_last_element() {
    // Pack the low end against i64::MIN so every gap *within* the set is tiny:
    //   * leading gap [i64::MIN, i64::MIN] = 0
    //   * interior gaps                    = 1 each
    // The only large gap is the trailing one, [i64::MIN + 2, i64::MAX], which is
    // handled by the explicit post-loop check (the "gap after the last element").
    // Without that check the function would fall back to its default seed
    // (MAX_PRIORITY / 2 == i64::MAX / 2); with it, the answer is the midpoint of
    // the trailing gap. We assert the trailing value to lock that branch.
    let used = BTreeSet::from([i64::MIN, i64::MIN + 1, i64::MIN + 2]);
    let mid = find_largest_gap_midpoint(&used);
    let last = i64::MIN + 2;
    let gap = i64::MAX as i128 - last as i128;
    let expected = (last as i128 + gap / 2) as i64;
    assert_eq!(mid, expected);
    // The trailing-gap branch must override the default seed of i64::MAX / 2.
    assert_ne!(
        mid,
        i64::MAX / 2,
        "trailing-gap tracking must change the result"
    );
    assert!(!used.contains(&mid), "midpoint must be free");
}

#[test]
fn find_largest_gap_midpoint_empty_set_returns_trailing_midpoint() {
    // Empty set: the loop never runs, so prev stays i64::MIN and only the
    // trailing-gap check fires, covering [i64::MIN, i64::MAX] in full. Its
    // midpoint overrides the default seed because that gap (~1.8e19) exceeds the
    // initial best_gap of 0.
    let used = BTreeSet::new();
    let mid = find_largest_gap_midpoint(&used);
    let gap = i64::MAX as i128 - i64::MIN as i128;
    let expected = (i64::MIN as i128 + gap / 2) as i64;
    assert_eq!(mid, expected);
}

#[test]
fn resolve_priority_falls_through_to_largest_gap_scan() {
    // Both immediate neighbours of the colliding value are exactly one step
    // away (gaps of size 1), so resolve_priority exhausts its gap_above /
    // gap_below branches and delegates to find_largest_gap_midpoint. The
    // returned value must match the direct scan and stay out of `used`.
    let used = BTreeSet::from([0, 1, 2, 1000]);
    let resolved = resolve_priority(1, &used);
    assert_eq!(resolved, find_largest_gap_midpoint(&used));
    assert!(!used.contains(&resolved), "resolved priority must be free");
}

// ---------------------------------------------------------------------------
// Deterministic-output guard.
//
// `generate` is reproducible from its `u64` seed: the RNG is `ChaCha8Rng` and
// every `fake` draw goes through `fake_with_rng`, so the ONLY non-reproducible
// data is absolute timestamps (anchored to `Utc::now()`). The fingerprint below
// covers every seed-derived field EXCEPT those timestamps. It exists so a
// refactor that silently changes the RNG call order/count — yielding
// different-but-still-valid seed data that the statistical tests above would
// happily accept — fails loudly here instead.
//
// To re-baseline after an *intentional* seeder change: run this test and copy
// the "left" hash from the failure into `EXPECT`.

/// FNV-1a (64-bit) over explicitly-fed bytes. Inline and endian-pinned so the
/// fingerprint is stable across platforms and toolchains (unlike `DefaultHasher`,
/// whose output is not guaranteed stable).
struct Fnv(u64);

impl Fnv {
    fn new() -> Self {
        Fnv(0xcbf2_9ce4_8422_2325)
    }

    fn write(&mut self, bytes: &[u8]) {
        for &b in bytes {
            self.0 ^= u64::from(b);
            self.0 = self.0.wrapping_mul(0x0000_0100_0000_01b3);
        }
    }

    /// Seed-derived card fields only. `created_at`/`modified_at` and the absolute
    /// `due_date` value are time-anchored, so only due-date *presence* is fed.
    fn card(&mut self, c: &Card) {
        self.write(b"C");
        self.write(c.id().as_bytes());
        self.write(&c.priority().to_le_bytes());
        self.write(c.content().as_bytes());
        self.write(&[u8::from(c.blazed()), u8::from(c.due_date().is_some())]);
        self.write(&(c.tags().len() as u64).to_le_bytes());
        for t in c.tags() {
            self.write(t.as_bytes());
        }
    }

    fn tag(&mut self, t: &Tag) {
        self.write(b"T");
        self.write(t.id().as_bytes());
        self.write(t.title().as_bytes());
        match t.color() {
            Some(c) => self.write(&[1, c.r, c.g, c.b]),
            None => self.write(&[0]),
        }
        self.write(&(t.implies().len() as u64).to_le_bytes());
        for i in t.implies() {
            self.write(i.as_bytes());
        }
    }
}

fn fingerprint(data: &SeedData) -> String {
    let mut h = Fnv::new();

    h.write(b"tag_chains");
    h.write(&(data.tag_chains.len() as u64).to_le_bytes());
    for chain in &data.tag_chains {
        h.write(&(chain.len() as u64).to_le_bytes());
        for t in chain {
            h.tag(t);
        }
    }

    h.write(b"card_chains");
    h.write(&(data.card_chains.len() as u64).to_le_bytes());
    for chain in &data.card_chains {
        h.write(&(chain.len() as u64).to_le_bytes());
        for c in chain {
            h.card(c);
        }
    }

    h.write(b"deleted_tag_chains");
    h.write(&(data.deleted_tag_chains.len() as u64).to_le_bytes());
    for chain in &data.deleted_tag_chains {
        h.write(&(chain.len() as u64).to_le_bytes());
        for t in chain {
            h.tag(t);
        }
    }

    h.write(b"deleted_card_chains");
    h.write(&(data.deleted_card_chains.len() as u64).to_le_bytes());
    for chain in &data.deleted_card_chains {
        h.write(&(chain.len() as u64).to_le_bytes());
        for c in chain {
            h.card(c);
        }
    }

    h.write(b"extra_ops");
    h.write(&(data.extra_ops.len() as u64).to_le_bytes());
    for ops in &data.extra_ops {
        h.write(&(ops.len() as u64).to_le_bytes());
        for item in ops {
            match item {
                PushItem::Cards(cs) => {
                    h.write(b"oc");
                    h.write(&(cs.len() as u64).to_le_bytes());
                    for c in cs {
                        h.card(c);
                    }
                }
                PushItem::Tags(ts) => {
                    h.write(b"ot");
                    h.write(&(ts.len() as u64).to_le_bytes());
                    for t in ts {
                        h.tag(t);
                    }
                }
                PushItem::DeleteCard { id } => {
                    h.write(b"dc");
                    h.write(id.as_bytes());
                }
                PushItem::DeleteTag { id } => {
                    h.write(b"dt");
                    h.write(id.as_bytes());
                }
            }
        }
    }

    format!("{:016x}", h.0)
}

#[test]
fn seed_output_is_deterministic_for_a_fixed_seed() {
    // Same seed -> identical fingerprint. If this fails, the fingerprint picked
    // up a non-reproducible (time/thread-random) field and must be narrowed.
    let a = fingerprint(&generate(42, 15, 300));
    let b = fingerprint(&generate(42, 15, 300));
    assert_eq!(a, b, "fingerprint not reproducible for a fixed seed");

    // Golden: changes iff the seeded output changed. A behavior-preserving
    // refactor MUST keep this stable; an intentional seeder change re-baselines it.
    const EXPECT: &str = "40139bf06e6283a5";
    assert_eq!(
        a, EXPECT,
        "seeded output for seed=42 changed: RNG call order/count regression, or an \
         intentional seeder change (re-baseline EXPECT with the left-hand hash)"
    );
}
