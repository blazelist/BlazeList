//! Priority-only debounce pipeline.
//!
//! Card moves (priority changes), and the sibling rebalance versions they
//! sometimes trigger, are coalesced into one push per burst so the server
//! records a single history entry. Every other edit pushes immediately.
//!
//! Each burst stores the pre-burst snapshot of the moved card plus, on a
//! rebalance, the pre-burst snapshot of each shifted sibling. New moves
//! overwrite the target priorities; the flushed chain has exactly one
//! version per card. Only one burst is in flight at a time — starting a
//! move on a different card flushes the previous burst.
//!
//! The burst is in-memory only. On `pagehide` the latest versions are
//! dropped into the OPFS-backed offline queue; the OPFS write and
//! network push are async, so a torn-down tab may lose a burst.
//! Priorities are recoverable in one keypress.

use blazelist_protocol::{Card, Entity, Utc};
use std::collections::HashMap;
use uuid::Uuid;

/// In-flight burst of priority moves for one card.
#[derive(Clone, Debug)]
pub struct PriorityBurst {
    pub card_id: Uuid,
    /// Pre-burst snapshot; the flush builds exactly one new version on
    /// top of this regardless of how many moves the burst saw.
    pub base: Card,
    pub priority: i64,
    /// Pre-burst snapshots of siblings shifted by a rebalance. Once a
    /// sibling is recorded its base is never re-written.
    pub sibling_bases: HashMap<Uuid, Card>,
    pub sibling_priorities: HashMap<Uuid, i64>,
}

impl PriorityBurst {
    pub fn new(base: Card, priority: i64) -> Self {
        Self {
            card_id: base.id(),
            base,
            priority,
            sibling_bases: HashMap::new(),
            sibling_priorities: HashMap::new(),
        }
    }

    /// Merge a new move into the burst. Sibling targets are overwritten;
    /// only the first base snapshot per sibling is kept.
    pub fn apply_move(&mut self, new_priority: i64, siblings_with_prev: &[(Card, i64)]) {
        self.priority = new_priority;
        for (prev_sibling, new_priority) in siblings_with_prev {
            let id = prev_sibling.id();
            self.sibling_bases
                .entry(id)
                .or_insert_with(|| prev_sibling.clone());
            self.sibling_priorities.insert(id, *new_priority);
        }
    }

    /// Materialise the burst into `(primary, siblings)`. Each card is one
    /// new version derived from its pre-burst base.
    pub fn build_versions(&self) -> (Card, Vec<Card>) {
        let now = Utc::now();
        let primary = self.base.next(
            self.base.content().to_string(),
            self.priority,
            self.base.tags().to_vec(),
            self.base.blazed(),
            now,
            self.base.due_date(),
        );
        let mut siblings = Vec::with_capacity(self.sibling_bases.len());
        for (id, base) in &self.sibling_bases {
            // Defensive: `apply_move` always inserts both entries together,
            // but fall back rather than panic if that ever drifts.
            let target = self
                .sibling_priorities
                .get(id)
                .copied()
                .unwrap_or_else(|| base.priority());
            siblings.push(base.next(
                base.content().to_string(),
                target,
                base.tags().to_vec(),
                base.blazed(),
                now,
                base.due_date(),
            ));
        }
        (primary, siblings)
    }
}

// WASM-side orchestration: timers (`web_sys`) + Leptos signals (`AppState`).
// The pure `PriorityBurst` above is host-testable.
#[cfg(target_arch = "wasm32")]
mod wasm_runtime {
    use super::PriorityBurst;
    use crate::state::store::AppState;
    use crate::state::sync::push_card_or_queue;
    use blazelist_protocol::{Card, Entity, PushItem};
    use leptos::prelude::*;
    use uuid::Uuid;
    use wasm_bindgen::prelude::*;

    #[wasm_bindgen]
    extern "C" {
        #[wasm_bindgen(js_name = "setTimeout")]
        fn set_timeout_js(handler: &js_sys::Function, timeout: i32) -> i32;
        #[wasm_bindgen(js_name = "clearTimeout")]
        fn clear_timeout_js(handle: i32);
    }

    fn cancel_priority_timer(state: &AppState) {
        let handle = state.priority_burst_timer_handle;
        let old = handle.get_untracked();
        if old != 0 {
            clear_timeout_js(old);
            handle.set(0);
        }
        state.priority_burst_countdown_ms.set(0);
    }

    /// Record a move on `prev_card`. If a burst on a different card is in
    /// flight, flush it first so server-side history entries stay 1:1
    /// with bursts. Updates the optimistic local view immediately.
    pub fn enqueue_move(
        state: &AppState,
        prev_card: &Card,
        new_priority: i64,
        siblings_with_prev: &[(Card, i64)],
    ) {
        let card_id = prev_card.id();

        let existing_card_id = state
            .pending_priority
            .with_untracked(|burst| burst.as_ref().map(|b| b.card_id));
        if let Some(existing_id) = existing_card_id
            && existing_id != card_id
        {
            flush(state);
        }

        state.pending_priority.update(|burst| match burst {
            Some(b) if b.card_id == card_id => {
                b.apply_move(new_priority, siblings_with_prev);
            }
            _ => {
                let mut fresh = PriorityBurst::new(prev_card.clone(), new_priority);
                fresh.apply_move(new_priority, siblings_with_prev);
                *burst = Some(fresh);
            }
        });

        if let Some(b) = state.pending_priority.get_untracked() {
            let (primary, siblings) = b.build_versions();
            state.upsert_card(primary);
            for sc in siblings {
                state.upsert_card(sc);
            }
        }

        arm_timer(*state);
    }

    fn arm_timer(state: AppState) {
        cancel_priority_timer(&state);

        if !state.priority_debounce_enabled.get_untracked() {
            flush(&state);
            return;
        }

        let delay_ms = state.priority_debounce_delay_ms.get_untracked();
        let cb = Closure::once(move || flush(&state));
        let func = cb.into_js_value();
        state.priority_burst_countdown_ms.set(delay_ms);
        let handle = set_timeout_js(func.unchecked_ref(), delay_ms as i32);
        state.priority_burst_timer_handle.set(handle);
    }

    /// Drop the in-flight burst for `id` without pushing — used by the
    /// delete path so the burst doesn't resurrect the card. Bursts on
    /// other cards are left alone.
    pub fn discard_for(state: &AppState, id: Uuid) {
        let matches = state
            .pending_priority
            .with_untracked(|b| b.as_ref().is_some_and(|b| b.card_id == id));
        if matches {
            state.pending_priority.set(None);
            cancel_priority_timer(state);
        }
    }

    /// Signal-only half of [`flush`]: take the burst and materialise it.
    /// Does NOT cancel the timer — callers must do that themselves
    /// ([`flush`] / [`flush_blocking`] wrap it). Split out for
    /// host-testability of `build_versions`.
    #[must_use]
    pub fn flush_sync_take(state: &AppState) -> Option<(Card, Vec<Card>)> {
        let burst = state.pending_priority.get_untracked();
        state.pending_priority.set(None);
        burst.map(|b| b.build_versions())
    }

    pub fn flush(state: &AppState) {
        cancel_priority_timer(state);
        if let Some((primary, siblings)) = flush_sync_take(state) {
            let state = *state;
            leptos::task::spawn_local(async move {
                push_burst(&state, primary, siblings).await;
            });
        }
    }

    /// Flush and await. Call this before pushing a non-priority field
    /// change (content, tags, blaze, due date) on the same card the
    /// burst targets: otherwise the burst's push can lose the ancestor-
    /// mismatch race and silently overwrite the caller's edit with the
    /// burst's pre-snapshot fields. Await from inside the same
    /// `spawn_local` as the subsequent push to sequence them strictly.
    pub async fn flush_now(state: &AppState) {
        cancel_priority_timer(state);
        if let Some((primary, siblings)) = flush_sync_take(state) {
            push_burst(state, primary, siblings).await;
        }
    }

    /// Unload-time flush (`pagehide` / `visibilitychange` → hidden).
    /// Synchronously stashes the burst in the in-memory offline queue,
    /// then kicks off the OPFS persist + push as `spawn_local` — either
    /// may be cut short by the unload (see module docs).
    pub fn flush_blocking(state: &AppState) {
        cancel_priority_timer(state);
        let Some((primary, siblings)) = flush_sync_take(state) else {
            return;
        };

        let state = *state;
        state.offline_queue.update(|q| {
            let id = primary.id();
            q.retain(|c| c.id() != id);
            q.push(primary.clone());
            for sc in &siblings {
                let id = sc.id();
                q.retain(|c| c.id() != id);
                q.push(sc.clone());
            }
        });

        leptos::task::spawn_local(async move {
            crate::storage::save_offline_queue(&state.offline_queue.get_untracked()).await;
            push_burst(&state, primary, siblings).await;
        });
    }

    /// Push the burst as a single batch when siblings are present, or as
    /// a one-card push otherwise. Falls through to the offline queue on
    /// failure via [`push_card_or_queue`].
    async fn push_burst(state: &AppState, primary: Card, siblings: Vec<Card>) {
        if siblings.is_empty() {
            push_card_or_queue(state, primary).await;
            return;
        }
        if let Some(client) = crate::state::store::get_client() {
            let mut items: Vec<PushItem> = Vec::with_capacity(siblings.len() + 1);
            items.push(PushItem::Cards(vec![primary.clone()]));
            for sc in &siblings {
                items.push(PushItem::Cards(vec![sc.clone()]));
            }
            match blazelist_client_lib::client::Client::push_batch(&*client, items).await {
                Ok(_) => {
                    let primary_id = primary.id();
                    let sibling_ids: Vec<Uuid> = siblings.iter().map(|c| c.id()).collect();
                    state.offline_queue.update(|q| {
                        q.retain(|c| c.id() != primary_id && !sibling_ids.contains(&c.id()));
                    });
                    crate::storage::save_offline_queue(&state.offline_queue.get_untracked()).await;
                    return;
                }
                Err(e) => {
                    tracing::warn!(%e, "Priority-burst batch push failed, queuing per card");
                }
            }
        }
        push_card_or_queue(state, primary).await;
        for sc in siblings {
            push_card_or_queue(state, sc).await;
        }
    }
}

#[cfg(target_arch = "wasm32")]
pub use wasm_runtime::{discard_for, enqueue_move, flush, flush_blocking, flush_now};

#[cfg(test)]
#[cfg(not(target_arch = "wasm32"))]
mod tests {
    use super::*;

    fn make_card(id: Uuid, priority: i64, content: &str) -> Card {
        Card::first(
            id,
            content.to_string(),
            priority,
            Vec::new(),
            false,
            Utc::now(),
            None,
        )
    }

    #[test]
    fn coalesce_multiple_moves_on_same_card_yields_one_version_with_latest_priority() {
        let id = Uuid::new_v4();
        let base = make_card(id, 100, "original");
        let mut burst = PriorityBurst::new(base.clone(), 100);
        burst.apply_move(200, &[]);
        burst.apply_move(300, &[]);
        burst.apply_move(400, &[]);

        let (primary, siblings) = burst.build_versions();
        assert!(siblings.is_empty(), "no rebalance means no siblings");
        assert_eq!(primary.id(), id);
        assert_eq!(primary.priority(), 400);
        assert_eq!(primary.content(), "original");
        assert_eq!(
            i64::from(primary.count()),
            i64::from(base.count()) + 1,
            "exactly one new version per burst — never a chain of N"
        );
    }

    #[test]
    fn distinct_card_bursts_stay_independent() {
        // The cross-card flush is enforced by `enqueue_move`; this just
        // pins the single-card invariant on `PriorityBurst` itself.
        let id_a = Uuid::new_v4();
        let id_b = Uuid::new_v4();
        let base_a = make_card(id_a, 100, "A");
        let mut burst = PriorityBurst::new(base_a, 200);
        let (primary, siblings) = burst.build_versions();
        assert_eq!(primary.id(), id_a);
        assert!(siblings.is_empty());
        assert_eq!(burst.card_id, id_a);
        assert_ne!(burst.card_id, id_b);
        burst.apply_move(300, &[]);
        let (primary, _) = burst.build_versions();
        assert_eq!(primary.priority(), 300);
    }

    #[test]
    fn rebalance_siblings_appear_once_each_with_latest_priority() {
        let primary_id = Uuid::new_v4();
        let sib_a_id = Uuid::new_v4();
        let sib_b_id = Uuid::new_v4();

        let base_primary = make_card(primary_id, 100, "primary");
        let sib_a_base = make_card(sib_a_id, 200, "A");
        let sib_b_base = make_card(sib_b_id, 300, "B");

        let mut burst = PriorityBurst::new(base_primary, 150);
        burst.apply_move(150, &[(sib_a_base.clone(), 210), (sib_b_base.clone(), 310)]);
        burst.apply_move(160, &[(sib_a_base.clone(), 220)]);

        let (primary, siblings) = burst.build_versions();
        assert_eq!(primary.priority(), 160);
        assert_eq!(siblings.len(), 2, "exactly one version per sibling");

        let mut got_ids: Vec<Uuid> = siblings.iter().map(|c| c.id()).collect();
        got_ids.sort();
        let mut expected_ids = vec![sib_a_id, sib_b_id];
        expected_ids.sort();
        assert_eq!(got_ids, expected_ids);

        let sib_a = siblings.iter().find(|c| c.id() == sib_a_id).unwrap();
        let sib_b = siblings.iter().find(|c| c.id() == sib_b_id).unwrap();
        assert_eq!(sib_a.priority(), 220, "sib_a uses the LATEST shift target");
        assert_eq!(
            sib_b.priority(),
            310,
            "sib_b keeps its original shift target"
        );
        assert_eq!(
            i64::from(sib_a.count()),
            i64::from(sib_a_base.count()) + 1,
            "siblings must be exactly one version beyond their base"
        );
    }

    #[test]
    fn build_versions_on_empty_burst_returns_primary_only() {
        let id = Uuid::new_v4();
        let base = make_card(id, 100, "x");
        let mut burst = PriorityBurst::new(base.clone(), 100);
        burst.apply_move(150, &[]);
        let (primary, siblings) = burst.build_versions();
        assert!(siblings.is_empty());
        assert_eq!(primary.priority(), 150);
    }

    #[test]
    fn taken_burst_can_be_rebuilt_from_same_base() {
        // Two sequential bursts on the same base must each produce a
        // version exactly one step beyond `base` — pinning the "take,
        // build, drop" contract used by `flush_sync_take`.
        let id = Uuid::new_v4();
        let base = make_card(id, 100, "x");
        let mut burst1 = PriorityBurst::new(base.clone(), 100);
        burst1.apply_move(150, &[]);
        let (primary1, _) = burst1.build_versions();
        drop(burst1);
        let mut burst2 = PriorityBurst::new(base.clone(), 100);
        burst2.apply_move(200, &[]);
        let (primary2, _) = burst2.build_versions();
        assert_eq!(primary2.priority(), 200);
        assert_ne!(primary1.priority(), primary2.priority());
        assert_eq!(i64::from(primary1.count()), i64::from(base.count()) + 1);
        assert_eq!(i64::from(primary2.count()), i64::from(base.count()) + 1);
    }

    #[test]
    fn apply_move_preserves_first_sibling_base_across_calls() {
        // Regression: a sibling shifted by two moves must anchor on the
        // FIRST snapshot; re-recording on every move would break the
        // version-chain ancestor.
        let primary_id = Uuid::new_v4();
        let sib_id = Uuid::new_v4();

        let primary_base = make_card(primary_id, 100, "primary");
        let sib_v0 = make_card(sib_id, 200, "sib");
        let sib_v1 = sib_v0.next(
            sib_v0.content().to_string(),
            210,
            sib_v0.tags().to_vec(),
            sib_v0.blazed(),
            Utc::now(),
            sib_v0.due_date(),
        );

        let mut burst = PriorityBurst::new(primary_base, 150);
        burst.apply_move(150, &[(sib_v0.clone(), 210)]);
        burst.apply_move(160, &[(sib_v1.clone(), 220)]);

        let recorded_base = burst.sibling_bases.get(&sib_id).unwrap();
        assert_eq!(recorded_base.priority(), sib_v0.priority());
    }
}
