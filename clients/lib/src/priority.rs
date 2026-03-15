//! Priority placement and rebalancing for card ordering.
//!
//! Consolidates duplicated priority computation logic from both the CLI and
//! WASM clients and adds rebalancing for exhausted gaps.

use blazelist_protocol::{Card, Entity, Utc};
use rand::RngExt;
use uuid::Uuid;

/// Jitter is ±(gap / JITTER_DIVISOR) around the midpoint.
///
/// A divisor of 16 gives ±6.25% jitter — enough to avoid collisions when two
/// clients independently target the same gap, without drifting far from center.
const JITTER_DIVISOR: i128 = 16;

/// Compute a priority between `upper` and `lower` using midpoint + jitter.
///
/// Midpoint (floored) between the two bounds, plus a small random jitter
/// to avoid collisions when two clients independently place cards into the
/// same gap.
pub fn compute_priority(upper: i64, lower: i64) -> i64 {
    let upper_val = upper as i128;
    let lower_val = lower as i128;
    // Overflow-safe midpoint using i128 arithmetic.
    let gap = upper_val - lower_val;
    let midpoint = lower_val + gap / 2;

    if gap <= 2 {
        // No room for jitter — just return the midpoint.
        return midpoint as i64;
    }

    let jitter_range = (gap / JITTER_DIVISOR).max(1);
    let mut rng = rand::rng();
    let jitter_val = rng.random_range(0..jitter_range * 2 + 1) - jitter_range;
    let result = (midpoint + jitter_val).clamp(lower_val + 1, upper_val - 1);

    result as i64
}

/// Returns a priority value as a percentage of the full `i64` range.
///
/// Maps `i64::MIN` to `0.0` and `i64::MAX` to `100.0`.
pub fn priority_percentage(priority: i64) -> f64 {
    ((priority as f64 - i64::MIN as f64) / (i64::MAX as f64 - i64::MIN as f64)) * 100.0
}

/// Trait for types that expose a card's identity and priority.
///
/// This allows the placement functions to work with both `&[Card]` and
/// `&[&Card]` without duplicating the logic.
trait CardRef {
    fn card_id(&self) -> Uuid;
    fn card_priority(&self) -> i64;
}

impl CardRef for Card {
    fn card_id(&self) -> Uuid {
        self.id()
    }
    fn card_priority(&self) -> i64 {
        self.priority()
    }
}

impl CardRef for &Card {
    fn card_id(&self) -> Uuid {
        (*self).id()
    }
    fn card_priority(&self) -> i64 {
        (*self).priority()
    }
}

/// Where to insert a card in the sorted list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InsertPosition {
    /// Insert at the top (highest priority).
    Top,
    /// Insert at the bottom (lowest priority).
    Bottom,
    /// Insert at this 0-based index in the descending-priority list.
    At(usize),
}

/// Result of priority placement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Placement {
    /// Gap exists — use this priority directly.
    Simple(i64),
    /// Gap exhausted — the new card gets `priority`, and `shifted` cards
    /// need their priorities updated too.
    Rebalanced {
        priority: i64,
        shifted: Vec<(Uuid, i64)>,
    },
}

/// Maximum gap between the virtual bound and a real card at a list edge.
///
/// When inserting at the top or bottom of a list, one bound is a virtual
/// extreme (i64::MAX or i64::MIN). This constant caps that gap so the new
/// card lands ~half this value (~32k) from its neighbor instead of halfway
/// across the full i64 range. Sequential edge inserts use ~32k each (linear),
/// giving ~2^48 inserts before reaching the center — vs ~63 with pure
/// midpoint halving.
const MAX_EDGE_GAP: i128 = 65_536;

/// Cap virtual bounds so edge inserts stay close to the nearest real card.
///
/// Only caps when exactly one bound is a virtual extreme (i64::MAX or i64::MIN)
/// and the other is a real card priority. When both are extreme (empty list),
/// the full range is preserved so the first card lands near zero.
fn cap_edge_bounds(upper: i64, lower: i64) -> (i64, i64) {
    let u = upper as i128;
    let l = lower as i128;

    if u - l <= MAX_EDGE_GAP {
        return (upper, lower);
    }

    let capped_upper = if upper == i64::MAX && lower != i64::MIN {
        (l + MAX_EDGE_GAP).min(i64::MAX as i128) as i64
    } else {
        upper
    };

    let capped_lower = if lower == i64::MIN && upper != i64::MAX {
        (u - MAX_EDGE_GAP).max(i64::MIN as i128) as i64
    } else {
        lower
    };

    (capped_upper, capped_lower)
}

/// Compute priority for a card at the given insert position.
///
/// `cards` must be sorted by priority descending.
pub fn place_card(cards: &[Card], position: InsertPosition) -> Placement {
    let (upper, lower, insert_idx) = bounds_for_position(cards, &position);
    let (upper, lower) = cap_edge_bounds(upper, lower);
    try_place(cards, upper, lower, insert_idx)
}

/// Compute priority for moving a card to a new position.
///
/// `cards` must be sorted by priority descending.
/// `card_id` is the card being moved.
pub fn move_card(cards: &[Card], card_id: Uuid, target: InsertPosition) -> Placement {
    let others: Vec<&Card> = cards.iter().filter(|c| c.id() != card_id).collect();
    let (upper, lower, insert_idx) = bounds_for_position(&others, &target);
    let (upper, lower) = cap_edge_bounds(upper, lower);
    try_place(&others, upper, lower, insert_idx)
}

/// Determine (upper, lower, insert_idx) for an insert position.
fn bounds_for_position<C: CardRef>(
    cards: &[C],
    position: &InsertPosition,
) -> (i64, i64, usize) {
    match position {
        InsertPosition::Top => {
            let lower = cards
                .first()
                .map(|c| c.card_priority())
                .unwrap_or(i64::MIN);
            (i64::MAX, lower, 0)
        }
        InsertPosition::Bottom => {
            let upper = cards
                .last()
                .map(|c| c.card_priority())
                .unwrap_or(i64::MAX);
            (upper, i64::MIN, cards.len())
        }
        InsertPosition::At(idx) => {
            let idx = (*idx).min(cards.len());
            let upper = if idx == 0 {
                i64::MAX
            } else {
                cards[idx - 1].card_priority()
            };
            let lower = if idx < cards.len() {
                cards[idx].card_priority()
            } else {
                i64::MIN
            };
            (upper, lower, idx)
        }
    }
}

/// Try to place at the given bounds. If the gap is sufficient, return Simple.
/// Otherwise, rebalance the packed range.
fn try_place<C: CardRef>(
    cards: &[C],
    upper: i64,
    lower: i64,
    insert_idx: usize,
) -> Placement {
    let upper_val = upper as i128;
    let lower_val = lower as i128;

    if upper_val - lower_val > 1 {
        Placement::Simple(compute_priority(upper, lower))
    } else {
        rebalance(cards, insert_idx)
    }
}

/// Rebalance a packed range around `insert_idx`.
///
/// Expands outward from the insertion point to find contiguous cards with
/// gap <= 1 between them. Then redistributes all cards in that range evenly
/// across the available space, and assigns the new card a priority within
/// the redistributed range.
fn rebalance<C: CardRef>(cards: &[C], insert_idx: usize) -> Placement {
    if cards.is_empty() {
        return Placement::Simple(compute_priority(i64::MAX, i64::MIN));
    }

    // Find the packed range boundaries.
    let mut left = if insert_idx > 0 { insert_idx - 1 } else { 0 };
    while left > 0 {
        let gap = cards[left - 1].card_priority() as i128 - cards[left].card_priority() as i128;
        if gap > 1 {
            break;
        }
        left -= 1;
    }

    let mut right = if insert_idx < cards.len() {
        insert_idx
    } else {
        cards.len() - 1
    };
    while right + 1 < cards.len() {
        let gap = cards[right].card_priority() as i128 - cards[right + 1].card_priority() as i128;
        if gap > 1 {
            break;
        }
        right += 1;
    }

    // Determine the available space.
    let range_upper: i128 = if left == 0 {
        i64::MAX as i128
    } else {
        cards[left - 1].card_priority() as i128
    };
    let range_lower: i128 = if right >= cards.len() - 1 {
        i64::MIN as i128
    } else {
        cards[right + 1].card_priority() as i128
    };

    // Total slots = existing cards in range + 1 new card.
    let range_count = right - left + 1;
    let total_slots = range_count + 1;

    // Distribute evenly.
    let space = range_upper - range_lower;
    let step = (space / (total_slots as i128 + 1)).max(1);

    // Assign priorities: slot 0 is highest (closest to range_upper).
    let new_slot = insert_idx - left;

    let mut shifted = Vec::with_capacity(range_count);
    let mut new_priority = None;
    let mut slot = 0;

    for card in &cards[left..=right] {
        if slot == new_slot && new_priority.is_none() {
            let p = range_upper - step * (slot as i128 + 1);
            new_priority = Some(p.clamp(i64::MIN as i128, i64::MAX as i128) as i64);
            slot += 1;
        }

        let p = range_upper - step * (slot as i128 + 1);
        let new_p = p.clamp(i64::MIN as i128, i64::MAX as i128) as i64;
        if new_p != card.card_priority() {
            shifted.push((card.card_id(), new_p));
        }
        slot += 1;
    }

    if new_priority.is_none() {
        let p = range_upper - step * (slot as i128 + 1);
        new_priority = Some(p.clamp(i64::MIN as i128, i64::MAX as i128) as i64);
    }

    let priority = new_priority.unwrap();

    if shifted.is_empty() {
        Placement::Simple(priority)
    } else {
        Placement::Rebalanced { priority, shifted }
    }
}

/// Compute a valid priority for a card whose desired priority collides.
///
/// Finds where `desired_priority` falls in the sorted card list and
/// calls [`place_card`] at that position to produce a non-colliding priority.
/// `cards` must be sorted by priority descending and must NOT include the
/// card being placed.
pub fn resolve_collision(cards: &[Card], desired_priority: i64) -> Placement {
    let idx = cards
        .iter()
        .position(|c| c.priority() <= desired_priority)
        .unwrap_or(cards.len());
    place_card(cards, InsertPosition::At(idx))
}

/// Build new versions of cards whose priorities must shift after a rebalance.
///
/// Given the `shifted` list from a [`Placement::Rebalanced`] result and the
/// full card list, produces updated card versions with their new priorities.
pub fn build_shifted_versions(shifted: &[(Uuid, i64)], all_cards: &[Card]) -> Vec<Card> {
    let now = Utc::now();
    shifted
        .iter()
        .filter_map(|(id, new_p)| {
            all_cards.iter().find(|c| c.id() == *id).map(|c| {
                c.next(
                    c.content().to_string(),
                    *new_p,
                    c.tags().to_vec(),
                    c.blazed(),
                    now,
                    c.due_date(),
                )
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use blazelist_protocol::Card;
    use chrono::Utc;

    fn make_card(priority: i64) -> Card {
        Card::first(
            Uuid::new_v4(),
            String::new(),
            priority,
            vec![],
            false,
            Utc::now(),
            None,
        )
    }

    #[test]
    fn place_card_top_empty_list() {
        let cards: Vec<Card> = vec![];
        let result = place_card(&cards, InsertPosition::Top);
        match result {
            Placement::Simple(p) => {
                assert!(p > i64::MIN, "priority should be above MIN: {p}");
            }
            Placement::Rebalanced { .. } => panic!("expected Simple for empty list"),
        }
    }

    #[test]
    fn place_card_top_with_existing() {
        let cards = vec![make_card(1000), make_card(500)];
        let result = place_card(&cards, InsertPosition::Top);
        match result {
            Placement::Simple(p) => {
                assert!(p > cards[0].priority(), "should be above highest card");
            }
            Placement::Rebalanced { .. } => panic!("expected Simple"),
        }
    }

    #[test]
    fn place_card_bottom_with_existing() {
        let cards = vec![make_card(1000), make_card(500)];
        let result = place_card(&cards, InsertPosition::Bottom);
        match result {
            Placement::Simple(p) => {
                assert!(p < cards[1].priority(), "should be below lowest card");
            }
            Placement::Rebalanced { .. } => panic!("expected Simple"),
        }
    }

    #[test]
    fn place_card_at_middle() {
        let cards = vec![make_card(1000), make_card(500)];
        let result = place_card(&cards, InsertPosition::At(1));
        match result {
            Placement::Simple(p) => {
                assert!(
                    p > 500 && p < 1000,
                    "should be between 500 and 1000: {p}"
                );
            }
            Placement::Rebalanced { .. } => panic!("expected Simple"),
        }
    }

    #[test]
    fn place_card_triggers_rebalance_on_gap_exhaustion() {
        let cards = vec![make_card(101), make_card(100)];
        let result = place_card(&cards, InsertPosition::At(1));
        match result {
            Placement::Rebalanced { .. } => {}
            Placement::Simple(_) => panic!("expected Rebalanced for exhausted gap"),
        }
    }

    #[test]
    fn move_card_simple() {
        let cards = vec![make_card(3000), make_card(2000), make_card(1000)];
        let card_id = cards[2].id();
        let result = move_card(&cards, card_id, InsertPosition::Top);
        match result {
            Placement::Simple(p) => {
                assert!(p > cards[0].priority(), "should be above highest");
            }
            Placement::Rebalanced { .. } => panic!("expected Simple"),
        }
    }

    #[test]
    fn move_card_rebalance() {
        let cards = vec![make_card(102), make_card(101), make_card(100)];
        let card_id = cards[0].id();
        let result = move_card(&cards, card_id, InsertPosition::At(1));
        // Either Simple or Rebalanced is fine, just verify it doesn't panic.
        match result {
            Placement::Rebalanced { .. } | Placement::Simple(_) => {}
        }
    }

    #[test]
    fn top_insert_capped_near_neighbor() {
        let cards = vec![make_card(0)];
        for _ in 0..100 {
            let result = place_card(&cards, InsertPosition::Top);
            match result {
                Placement::Simple(p) => {
                    assert!(p > 0, "should be above card: {p}");
                    assert!(
                        p <= MAX_EDGE_GAP as i64,
                        "should be within cap range: {p}"
                    );
                }
                Placement::Rebalanced { .. } => panic!("expected Simple"),
            }
        }
    }

    #[test]
    fn bottom_insert_capped_near_neighbor() {
        let cards = vec![make_card(0)];
        for _ in 0..100 {
            let result = place_card(&cards, InsertPosition::Bottom);
            match result {
                Placement::Simple(p) => {
                    assert!(p < 0, "should be below card: {p}");
                    assert!(
                        p >= -(MAX_EDGE_GAP) as i64,
                        "should be within cap range: {p}"
                    );
                }
                Placement::Rebalanced { .. } => panic!("expected Simple"),
            }
        }
    }

    #[test]
    fn empty_list_not_capped() {
        let cards: Vec<Card> = vec![];
        for _ in 0..100 {
            let result = place_card(&cards, InsertPosition::Top);
            match result {
                Placement::Simple(p) => {
                    // First card should land near zero (midpoint of full range),
                    // not pinned near i64::MIN.
                    assert!(
                        p.abs() < i64::MAX / 4,
                        "first card should be near center, got {p}"
                    );
                }
                Placement::Rebalanced { .. } => panic!("expected Simple"),
            }
        }
    }

    #[test]
    fn between_card_insert_not_capped() {
        // Two cards far apart — midpoint behavior should be preserved.
        let cards = vec![make_card(1_000_000), make_card(-1_000_000)];
        for _ in 0..100 {
            let result = place_card(&cards, InsertPosition::At(1));
            match result {
                Placement::Simple(p) => {
                    // With pure midpoint + jitter, should be near 0 (±gap/16 = ±125k).
                    assert!(p > -200_000 && p < 200_000, "should be near midpoint: {p}");
                }
                Placement::Rebalanced { .. } => panic!("expected Simple"),
            }
        }
    }

    #[test]
    fn cap_near_i64_max_no_overflow() {
        let cards = vec![make_card(i64::MAX - 100)];
        let result = place_card(&cards, InsertPosition::Top);
        match result {
            Placement::Simple(p) => {
                assert!(p > i64::MAX - 100, "should be above card: {p}");
                assert!(p < i64::MAX, "should be below MAX: {p}");
            }
            Placement::Rebalanced { .. } => panic!("expected Simple"),
        }
    }

    // -- compute_priority unit tests --

    #[test]
    fn compute_priority_midpoint_between_extremes() {
        let result = compute_priority(i64::MAX, i64::MIN);
        assert!(result > i64::MIN);
        assert!(result < i64::MAX);
    }

    #[test]
    fn compute_priority_always_between_bounds() {
        for _ in 0..100 {
            let result = compute_priority(1000, 100);
            assert!(result > 100, "expected > 100, got {result}");
            assert!(result < 1000, "expected < 1000, got {result}");
        }
    }

    #[test]
    fn compute_priority_narrow_gap() {
        let result = compute_priority(101, 99);
        assert_eq!(result, 100);
    }

    #[test]
    fn compute_priority_gap_of_one() {
        let result = compute_priority(101, 100);
        assert_eq!(result, 100);
    }

    #[test]
    fn compute_priority_same_values() {
        let result = compute_priority(50, 50);
        assert_eq!(result, 50);
    }

    #[test]
    fn compute_priority_at_min_lower() {
        let result = compute_priority(100, i64::MIN);
        assert!(result > i64::MIN);
        assert!(result < 100);
    }

    #[test]
    fn compute_priority_negative_bounds() {
        for _ in 0..100 {
            let result = compute_priority(-100, -1000);
            assert!(result > -1000, "expected > -1000, got {result}");
            assert!(result < -100, "expected < -100, got {result}");
        }
    }

    #[test]
    fn compute_priority_across_zero() {
        for _ in 0..100 {
            let result = compute_priority(1000, -1000);
            assert!(result > -1000, "expected > -1000, got {result}");
            assert!(result < 1000, "expected < 1000, got {result}");
        }
    }

    // -- priority_percentage unit tests --

    #[test]
    fn priority_percentage_extremes() {
        assert_eq!(priority_percentage(i64::MIN), 0.0);
        assert_eq!(priority_percentage(i64::MAX), 100.0);
    }

    #[test]
    fn priority_percentage_midrange() {
        let pct = priority_percentage(0);
        assert!(pct > 49.9 && pct < 50.1, "expected ~50%, got {pct}");
    }

    // -- rebalance & resolve_collision edge case tests --

    /// Build the full priority list after a placement: applies shifted priorities
    /// to the original cards and inserts the new card's priority at the correct
    /// position.  Returns the combined list sorted descending.
    fn apply_placement(original_cards: &[Card], placement: &Placement) -> Vec<i64> {
        let (new_p, shifted) = match placement {
            Placement::Simple(p) => (*p, &[][..]),
            Placement::Rebalanced { priority, shifted } => (*priority, shifted.as_slice()),
        };
        let mut priorities: Vec<i64> = original_cards
            .iter()
            .map(|c| {
                shifted
                    .iter()
                    .find(|(id, _)| *id == c.id())
                    .map(|(_, p)| *p)
                    .unwrap_or_else(|| c.priority())
            })
            .collect();
        priorities.push(new_p);
        priorities.sort_unstable_by(|a, b| b.cmp(a));
        priorities
    }

    /// Assert that a placement produces a valid result:
    /// - all priorities strictly within (i64::MIN, i64::MAX)
    /// - no duplicates (including against unchanged original cards)
    /// - sorted descending with no ties
    fn assert_valid_placement(label: &str, original_cards: &[Card], placement: &Placement) {
        let combined = apply_placement(original_cards, placement);
        for &p in &combined {
            assert!(p > i64::MIN, "{label}: priority {p} must be > i64::MIN");
            assert!(p < i64::MAX, "{label}: priority {p} must be < i64::MAX");
        }
        for w in combined.windows(2) {
            assert!(
                w[0] > w[1],
                "{label}: priorities must be strictly descending, got {} then {}",
                w[0],
                w[1]
            );
        }
    }

    // -- rebalance: boundary tests --

    #[test]
    fn rebalance_near_i64_max_insert_top() {
        let cards = vec![
            make_card(i64::MAX - 1),
            make_card(i64::MAX - 2),
            make_card(i64::MAX - 3),
        ];
        let result = place_card(&cards, InsertPosition::Top);
        assert!(matches!(result, Placement::Rebalanced { .. }));
        assert_valid_placement("MAX top", &cards, &result);
    }

    #[test]
    fn rebalance_near_i64_max_insert_middle() {
        let cards = vec![
            make_card(i64::MAX - 1),
            make_card(i64::MAX - 2),
            make_card(i64::MAX - 3),
        ];
        let result = place_card(&cards, InsertPosition::At(1));
        assert!(matches!(result, Placement::Rebalanced { .. }));
        assert_valid_placement("MAX mid", &cards, &result);
    }

    #[test]
    fn rebalance_near_i64_max_insert_bottom() {
        // Bottom is far from the packed range — edge capping gives room for Simple.
        let cards = vec![
            make_card(i64::MAX - 1),
            make_card(i64::MAX - 2),
            make_card(i64::MAX - 3),
        ];
        let result = place_card(&cards, InsertPosition::Bottom);
        assert_valid_placement("MAX bot", &cards, &result);
    }

    #[test]
    fn rebalance_near_i64_min_insert_top() {
        // Top is far from the packed range — edge capping gives room for Simple.
        let cards = vec![
            make_card(i64::MIN + 3),
            make_card(i64::MIN + 2),
            make_card(i64::MIN + 1),
        ];
        let result = place_card(&cards, InsertPosition::Top);
        assert_valid_placement("MIN top", &cards, &result);
    }

    #[test]
    fn rebalance_near_i64_min_insert_middle() {
        let cards = vec![
            make_card(i64::MIN + 3),
            make_card(i64::MIN + 2),
            make_card(i64::MIN + 1),
        ];
        let result = place_card(&cards, InsertPosition::At(1));
        assert!(matches!(result, Placement::Rebalanced { .. }));
        assert_valid_placement("MIN mid", &cards, &result);
    }

    #[test]
    fn rebalance_near_i64_min_insert_bottom() {
        let cards = vec![
            make_card(i64::MIN + 3),
            make_card(i64::MIN + 2),
            make_card(i64::MIN + 1),
        ];
        let result = place_card(&cards, InsertPosition::Bottom);
        assert!(matches!(result, Placement::Rebalanced { .. }));
        assert_valid_placement("MIN bot", &cards, &result);
    }

    // -- rebalance: packed middle with non-shifted neighbors --

    #[test]
    fn rebalance_packed_middle_no_neighbor_collision() {
        let cards = vec![
            make_card(1000),
            make_card(103),
            make_card(102),
            make_card(101),
            make_card(100),
            make_card(-1000),
        ];
        let result = place_card(&cards, InsertPosition::At(2));
        assert!(matches!(result, Placement::Rebalanced { .. }));
        assert_valid_placement("packed mid", &cards, &result);

        // Shifted + new priorities must stay within the neighbor bounds.
        let (new_p, shifted) = match &result {
            Placement::Rebalanced { priority, shifted } => (*priority, shifted),
            _ => unreachable!(),
        };
        assert!(new_p > -1000 && new_p < 1000, "new priority {new_p} outside neighbor bounds");
        for &(_, p) in shifted {
            assert!(p > -1000 && p < 1000, "shifted priority {p} outside neighbor bounds");
        }
    }

    // -- rebalance: long packed chains --

    #[test]
    fn rebalance_long_chain_10_cards() {
        // 10 consecutive priorities, insert in the middle.
        let cards: Vec<Card> = (0..10).rev().map(|i| make_card(i)).collect();
        let result = place_card(&cards, InsertPosition::At(5));
        assert!(matches!(result, Placement::Rebalanced { .. }));
        assert_valid_placement("chain 10", &cards, &result);
    }

    #[test]
    fn rebalance_long_chain_20_cards() {
        // 20 consecutive priorities, insert at every position.
        // Top/Bottom get edge capping → Simple; interior positions → Rebalanced.
        let cards: Vec<Card> = (0..20).rev().map(|i| make_card(i)).collect();
        for idx in 0..=20 {
            let pos = if idx == 0 {
                InsertPosition::Top
            } else if idx == 20 {
                InsertPosition::Bottom
            } else {
                InsertPosition::At(idx)
            };
            let result = place_card(&cards, pos);
            if idx > 0 && idx < 20 {
                assert!(
                    matches!(result, Placement::Rebalanced { .. }),
                    "interior index {idx} should rebalance, got {result:?}"
                );
            }
            assert_valid_placement(&format!("chain 20 at {idx}"), &cards, &result);
        }
    }

    #[test]
    fn rebalance_long_chain_at_boundary() {
        // 10 consecutive cards packed right at i64::MAX.
        let cards: Vec<Card> = (0..10)
            .map(|i| make_card(i64::MAX - 1 - i))
            .collect();
        let result = place_card(&cards, InsertPosition::Top);
        assert!(matches!(result, Placement::Rebalanced { .. }));
        assert_valid_placement("chain at MAX", &cards, &result);

        // Verify multiple cards actually shifted (not just one).
        if let Placement::Rebalanced { shifted, .. } = &result {
            assert!(
                shifted.len() >= 2,
                "expected multiple cards shifted, got {}",
                shifted.len()
            );
        }
    }

    // -- rebalance: two adjacent cards with gap of exactly 1 --

    #[test]
    fn rebalance_gap_of_one() {
        let cards = vec![make_card(1), make_card(0)];
        let result = place_card(&cards, InsertPosition::At(1));
        assert!(matches!(result, Placement::Rebalanced { .. }));
        assert_valid_placement("gap 1", &cards, &result);
    }

    // -- rebalance: packed range spanning zero --

    #[test]
    fn rebalance_spanning_zero() {
        let cards = vec![make_card(2), make_card(1), make_card(0), make_card(-1), make_card(-2)];
        let result = place_card(&cards, InsertPosition::At(2));
        assert!(matches!(result, Placement::Rebalanced { .. }));
        assert_valid_placement("span zero", &cards, &result);
    }

    // -- rebalance: sequential rebalances (rebalance, then insert again) --

    #[test]
    fn sequential_rebalances() {
        let mut cards = vec![make_card(2), make_card(1), make_card(0)];

        // First rebalance: insert between 2 and 1.
        let r1 = place_card(&cards, InsertPosition::At(1));
        assert!(matches!(r1, Placement::Rebalanced { .. }));
        assert_valid_placement("seq r1", &cards, &r1);

        // Apply the first rebalance to get the new card list.
        let combined = apply_placement(&cards, &r1);
        cards = combined.iter().map(|&p| make_card(p)).collect();

        // Second insert into the rebalanced layout — should succeed without panic.
        let r2 = place_card(&cards, InsertPosition::At(2));
        assert_valid_placement("seq r2", &cards, &r2);
    }

    // -- resolve_collision tests --

    #[test]
    fn resolve_collision_finds_gap() {
        let cards = vec![make_card(300), make_card(200), make_card(100)];
        let result = resolve_collision(&cards, 200);
        assert_valid_placement("collision gap", &cards, &result);
        let new_p = match &result {
            Placement::Simple(p) => *p,
            Placement::Rebalanced { priority, .. } => *priority,
        };
        assert_ne!(new_p, 200, "should not collide with existing priority");
    }

    #[test]
    fn resolve_collision_at_top_of_list() {
        let cards = vec![make_card(300), make_card(200), make_card(100)];
        let result = resolve_collision(&cards, 300);
        assert_valid_placement("collision top", &cards, &result);
    }

    #[test]
    fn resolve_collision_at_bottom_of_list() {
        let cards = vec![make_card(300), make_card(200), make_card(100)];
        let result = resolve_collision(&cards, 100);
        assert_valid_placement("collision bot", &cards, &result);
    }

    #[test]
    fn resolve_collision_empty_list() {
        let cards: Vec<Card> = vec![];
        let result = resolve_collision(&cards, 42);
        assert_valid_placement("collision empty", &cards, &result);
    }

    #[test]
    fn resolve_collision_triggers_rebalance() {
        let cards = vec![make_card(3), make_card(2), make_card(1)];
        let result = resolve_collision(&cards, 2);
        assert!(
            matches!(result, Placement::Rebalanced { .. }),
            "expected Rebalanced for exhausted gaps, got {result:?}"
        );
        assert_valid_placement("collision rebalance", &cards, &result);
    }

    #[test]
    fn resolve_collision_at_i64_max() {
        let cards = vec![make_card(i64::MAX - 1)];
        let result = resolve_collision(&cards, i64::MAX - 1);
        assert_valid_placement("collision MAX", &cards, &result);
    }

    #[test]
    fn resolve_collision_at_i64_min() {
        let cards = vec![make_card(i64::MIN + 1)];
        let result = resolve_collision(&cards, i64::MIN + 1);
        assert_valid_placement("collision MIN", &cards, &result);
    }

    #[test]
    fn resolve_collision_packed_boundary() {
        // Collide in the middle of a packed range at i64::MAX boundary.
        let cards = vec![
            make_card(i64::MAX - 1),
            make_card(i64::MAX - 2),
            make_card(i64::MAX - 3),
        ];
        let result = resolve_collision(&cards, i64::MAX - 2);
        assert_valid_placement("collision packed MAX", &cards, &result);
    }

    #[test]
    fn resolve_collision_packed_boundary_min() {
        // Collide in the middle of a packed range at i64::MIN boundary.
        let cards = vec![
            make_card(i64::MIN + 3),
            make_card(i64::MIN + 2),
            make_card(i64::MIN + 1),
        ];
        let result = resolve_collision(&cards, i64::MIN + 2);
        assert_valid_placement("collision packed MIN", &cards, &result);
    }

    #[test]
    fn resolve_collision_long_packed_chain() {
        // 10 consecutive cards, collide in the middle.
        let cards: Vec<Card> = (0..10).rev().map(|i| make_card(i)).collect();
        let result = resolve_collision(&cards, 5);
        assert!(matches!(result, Placement::Rebalanced { .. }));
        assert_valid_placement("collision chain", &cards, &result);
    }
}
