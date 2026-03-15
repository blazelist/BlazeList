//! Priority placement and rebalancing for card ordering.
//!
//! Consolidates duplicated priority computation logic from both the CLI and
//! WASM clients and adds rebalancing for exhausted gaps.

use blazelist_protocol::{Card, Entity, Utc, compute_priority};
use uuid::Uuid;

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

/// Compute priority for a card at the given insert position.
///
/// `cards` must be sorted by priority descending.
pub fn place_card(cards: &[Card], position: InsertPosition) -> Placement {
    let (upper, lower, insert_idx) = bounds_for_position(cards, &position);
    try_place(cards, upper, lower, insert_idx)
}

/// Compute priority for moving a card to a new position.
///
/// `cards` must be sorted by priority descending.
/// `card_id` is the card being moved.
pub fn move_card(cards: &[Card], card_id: Uuid, target: InsertPosition) -> Placement {
    let others: Vec<&Card> = cards.iter().filter(|c| c.id() != card_id).collect();
    let (upper, lower, insert_idx) = bounds_for_position(&others, &target);
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
}
