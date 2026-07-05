//! Pure drag-and-drop logic — no `web_sys`, no Leptos signals, no DOM.
//!
//! `DropEdge` and `drop_position` live here so they can be exercised by
//! the host `cargo test` target. The wasm-only drag controller in
//! `components/drag_drop.rs` re-uses both.

// Host builds never run the wasm controller; gate dead-code warnings so
// the module stays warning-clean under both targets.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]

use blazelist_client_lib::priority::InsertPosition;
use blazelist_protocol::{Card, Entity};
use uuid::Uuid;

/// Which edge of the hovered card the drag source would land on when dropped.
///
/// The hit-test pipeline canonicalises every inter-card gap to
/// `Above(next-card)`: the lower half of card N and the upper half of
/// card N+1 resolve to the same value, so a single indicator lights up
/// per gap. `Below` is emitted only for the **last** card's lower half,
/// where there is no next sibling to anchor `Above` against.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DropEdge {
    Above,
    Below,
}

/// Which drag activation mode is in effect.
///
/// Stored on disk as a string (`"anywhere"` / `"handle"`); see
/// `state::settings::is_valid_drag_and_drop_mode`. The wasm view layer
/// wires different DOM event sources depending on the mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DragMode {
    /// Mouse / pen drag activates on `pointerdown` anywhere on a card
    /// after a small movement threshold. Best for desktop.
    Anywhere,
    /// Drag activates only when the gesture starts on the card's
    /// leading number, which doubles as the drag grip. Best for touch
    /// so the rest of the card keeps native scroll and existing swipes.
    Handle,
}

impl DragMode {
    /// Parse a setting string into a mode, falling back to the default
    /// (`Anywhere`) for unknown values.
    pub fn from_str_or_default(s: &str) -> Self {
        match s {
            "handle" => DragMode::Handle,
            _ => DragMode::Anywhere,
        }
    }
}

/// Compute the `InsertPosition` for dropping `source_id` on `edge` of
/// `target_id` in the descending-priority `filtered` list.
///
/// `move_card` operates on the same list with the source removed, so
/// the target's index in that reduced list is one less when source sat
/// above it. The returned position is in the reduced-list coordinate
/// system, which is what `move_card` and `apply_move_placement` expect.
///
/// Returns `None` for:
/// - source or target not in `filtered` (deleted by sync mid-drag)
/// - source == target (defensive; UI shouldn't surface this)
/// - the gap source already occupies (dropping `Above` the card
///   immediately below source, or `Below` the card immediately above)
pub fn drop_position(
    filtered: &[Card],
    source_id: Uuid,
    target_id: Uuid,
    edge: DropEdge,
) -> Option<InsertPosition> {
    if source_id == target_id {
        return None;
    }
    let src_idx = filtered.iter().position(|c| c.id() == source_id)?;
    let tgt_idx = filtered.iter().position(|c| c.id() == target_id)?;
    let base = if tgt_idx > src_idx {
        tgt_idx - 1
    } else {
        tgt_idx
    };
    let pos = match edge {
        DropEdge::Above => base,
        DropEdge::Below => base + 1,
    };
    if pos == src_idx {
        return None;
    }
    Some(InsertPosition::At(pos))
}

#[cfg(test)]
mod tests {
    use super::*;
    use blazelist_protocol::Utc;

    fn card(priority: i64) -> Card {
        Card::first(
            Uuid::new_v4(),
            String::new(),
            priority,
            Vec::new(),
            false,
            Utc::now(),
            None,
        )
    }

    fn list4() -> (Card, Card, Card, Card) {
        (card(40), card(30), card(20), card(10))
    }

    #[test]
    fn drop_above_first_from_middle() {
        let (a, b, c, d) = list4();
        let list = [a.clone(), b, c.clone(), d];
        let pos = drop_position(&list, c.id(), a.id(), DropEdge::Above).unwrap();
        assert_eq!(pos, InsertPosition::At(0));
    }

    #[test]
    fn drop_below_last_from_middle() {
        let (a, b, c, d) = list4();
        let list = [a, b.clone(), c, d.clone()];
        let pos = drop_position(&list, b.id(), d.id(), DropEdge::Below).unwrap();
        assert_eq!(pos, InsertPosition::At(3));
    }

    #[test]
    fn drop_above_neighbour_below_is_noop() {
        let (a, b, c, d) = list4();
        let list = [a, b.clone(), c.clone(), d];
        assert_eq!(drop_position(&list, b.id(), c.id(), DropEdge::Above), None,);
    }

    #[test]
    fn drop_below_neighbour_above_is_noop() {
        let (a, b, c, d) = list4();
        let list = [a, b.clone(), c.clone(), d];
        assert_eq!(drop_position(&list, c.id(), b.id(), DropEdge::Below), None,);
    }

    #[test]
    fn drop_above_distant_card_below_source() {
        let (a, b, c, d) = list4();
        let e = card(5);
        let list = [a.clone(), b, c, d.clone(), e];
        let pos = drop_position(&list, a.id(), d.id(), DropEdge::Above).unwrap();
        assert_eq!(pos, InsertPosition::At(2));
    }

    #[test]
    fn drop_below_distant_card_above_source() {
        let (a, b, c, d) = list4();
        let list = [a, b.clone(), c, d.clone()];
        let pos = drop_position(&list, d.id(), b.id(), DropEdge::Below).unwrap();
        assert_eq!(pos, InsertPosition::At(2));
    }

    #[test]
    fn full_forward_traversal() {
        let (a, b, c, d) = list4();
        let list = [a.clone(), b, c, d.clone()];
        let pos = drop_position(&list, a.id(), d.id(), DropEdge::Below).unwrap();
        assert_eq!(pos, InsertPosition::At(3));
    }

    #[test]
    fn full_reverse_traversal() {
        let (a, b, c, d) = list4();
        let list = [a.clone(), b, c, d.clone()];
        let pos = drop_position(&list, d.id(), a.id(), DropEdge::Above).unwrap();
        assert_eq!(pos, InsertPosition::At(0));
    }

    #[test]
    fn missing_source_returns_none() {
        let (a, b, c, d) = list4();
        let foreign = card(99);
        let list = [a.clone(), b, c, d];
        assert_eq!(
            drop_position(&list, foreign.id(), a.id(), DropEdge::Above),
            None
        );
    }

    #[test]
    fn missing_target_returns_none() {
        let (a, b, c, d) = list4();
        let foreign = card(99);
        let list = [a.clone(), b, c, d];
        assert_eq!(
            drop_position(&list, a.id(), foreign.id(), DropEdge::Below),
            None
        );
    }

    #[test]
    fn source_equals_target_returns_none() {
        let (a, _b, _c, _d) = list4();
        let list = [a.clone()];
        assert_eq!(drop_position(&list, a.id(), a.id(), DropEdge::Above), None);
        assert_eq!(drop_position(&list, a.id(), a.id(), DropEdge::Below), None);
    }

    #[test]
    fn single_card_list_is_noop() {
        let only = card(10);
        let list = [only.clone()];
        assert_eq!(
            drop_position(&list, only.id(), only.id(), DropEdge::Above),
            None
        );
    }

    #[test]
    fn mode_parses_known_values() {
        assert_eq!(
            DragMode::from_str_or_default("anywhere"),
            DragMode::Anywhere
        );
        assert_eq!(DragMode::from_str_or_default("handle"), DragMode::Handle);
    }

    #[test]
    fn mode_falls_back_on_unknown() {
        assert_eq!(DragMode::from_str_or_default(""), DragMode::Anywhere);
        assert_eq!(DragMode::from_str_or_default("HANDLE"), DragMode::Anywhere);
        assert_eq!(DragMode::from_str_or_default("desktop"), DragMode::Anywhere);
    }
}
