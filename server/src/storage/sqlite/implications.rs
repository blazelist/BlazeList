//! Post-batch tag-implication validator.
//!
//! The server enforces the invariant that every card's tag set is
//! closed under the tag implication relation. A card push (or any
//! card resulting from a batch) that leaves a card missing a
//! transitively-implied tag is rejected. A tag push that would
//! introduce a cycle in the implication graph is rejected. A tag
//! push that would leave any live card in violation is rejected
//! unless the batch also carries the card updates that bring it back
//! into compliance.
//!
//! [`validate_batch_implications`] is the single authoritative check.
//! It runs inside the existing `push_batch` transaction, after all
//! per-item writes have succeeded, and before `recompute_root` —
//! so any violation rolls back the whole batch. Single-item pushes
//! (`push_card_versions`, `push_tag_versions`) funnel through
//! `push_batch` via one-element batches, so they get the same
//! guarantee for free.

use std::collections::BTreeMap;

use blazelist_protocol::{Entity, PushItem, TagGraph};
use rusqlite::Connection;
use uuid::Uuid;

use crate::storage::error::{PushError, PushOpError};

use super::SqliteStorage;

impl SqliteStorage {
    /// Validate the tag implication invariant for a batch.
    ///
    /// Builds an in-memory snapshot of the live tag graph and live card
    /// tag sets, overlays the batch's updates on top, then runs:
    ///
    /// 1. **Dangling-reference check**: every tag's `implies` entry must
    ///    be a live (known, non-deleted) tag id. Violations return
    ///    [`PushError::TagImpliesUnknown`].
    /// 2. **Cycle detection** on the post-batch tag graph. A cycle
    ///    anywhere triggers [`PushError::TagImplicationCycle`].
    /// 3. **Closure check** for every post-batch card. If any card is
    ///    missing a transitively-implied tag, returns the first
    ///    violation as [`PushError::TagImplicationViolation`].
    ///
    /// Called from `push_batch` after per-item writes succeed. Any
    /// error rolls the enclosing transaction back.
    pub(super) fn validate_batch_implications(
        conn: &Connection,
        items: &[PushItem],
    ) -> Result<(), PushOpError> {
        // -- 1. Snapshot live tag implies ----------------------------------
        let mut tag_implies: BTreeMap<Uuid, Vec<Uuid>> = BTreeMap::new();
        {
            let mut stmt = conn.prepare("SELECT id, implies FROM tags")?;
            let rows = stmt.query_map([], |row| {
                let id_bytes: Vec<u8> = row.get(0)?;
                let implies_bytes: Vec<u8> = row.get(1)?;
                Ok((id_bytes, implies_bytes))
            })?;
            for row in rows {
                let (id_bytes, implies_bytes) = row?;
                let id = Uuid::from_bytes(id_bytes.as_slice().try_into().map_err(|_| {
                    PushOpError::Internal("invalid tag id blob in validator".into())
                })?);
                let implies = Self::deserialize_implies(&implies_bytes);
                tag_implies.insert(id, implies);
            }
        }

        // -- 2. Snapshot live card tag lists -------------------------------
        let mut card_tags: BTreeMap<Uuid, Vec<Uuid>> = BTreeMap::new();
        {
            let mut stmt = conn.prepare("SELECT id, tags FROM cards")?;
            let rows = stmt.query_map([], |row| {
                let id_bytes: Vec<u8> = row.get(0)?;
                let tags_bytes: Vec<u8> = row.get(1)?;
                Ok((id_bytes, tags_bytes))
            })?;
            for row in rows {
                let (id_bytes, tags_bytes) = row?;
                let id = Uuid::from_bytes(id_bytes.as_slice().try_into().map_err(|_| {
                    PushOpError::Internal("invalid card id blob in validator".into())
                })?);
                let tags = Self::deserialize_tags(&tags_bytes);
                card_tags.insert(id, tags);
            }
        }

        // -- 3. Overlay the batch on top of the snapshots -------------------
        // For each item we take the **last** version as the post-batch
        // state of that entity. For delete items we remove from the
        // corresponding map. This mirrors how `push_*_inner` mutates
        // the real rows earlier in the same transaction.
        for item in items {
            match item {
                PushItem::Tags(versions) => {
                    if let Some(latest) = versions.last() {
                        tag_implies.insert(latest.id(), latest.implies().to_vec());
                    }
                }
                PushItem::Cards(versions) => {
                    if let Some(latest) = versions.last() {
                        card_tags.insert(latest.id(), latest.tags().to_vec());
                    }
                }
                PushItem::DeleteTag { id } => {
                    tag_implies.remove(id);
                }
                PushItem::DeleteCard { id } => {
                    card_tags.remove(id);
                }
            }
        }

        // -- 4. Dangling-reference check -----------------------------------
        // Every tag's `implies` list must point at live, known tags.
        // "Live" means present as a key in the post-batch snapshot
        // (which already excludes any tag removed by a `DeleteTag` item).
        // This catches typos, stale references, and batches that delete
        // a still-implied target — all of which would otherwise surface
        // later as confusing `TagImplicationViolation` errors on any
        // card holding the implying tag.
        let live_tag_ids: std::collections::BTreeSet<Uuid> = tag_implies.keys().copied().collect();
        for (tag_id, implies) in &tag_implies {
            let mut missing: Vec<Uuid> = implies
                .iter()
                .copied()
                .filter(|parent| !live_tag_ids.contains(parent))
                .collect();
            if !missing.is_empty() {
                missing.sort();
                missing.dedup();
                return Err(PushError::TagImpliesUnknown {
                    tag_id: *tag_id,
                    missing,
                }
                .into());
            }
        }

        // -- 5. Cycle detection on the post-batch tag graph -----------------
        let graph = TagGraph::from_pairs(tag_implies);
        if let Some(cycle) = graph.detect_cycle() {
            return Err(PushError::TagImplicationCycle { cycle: cycle.cycle }.into());
        }

        // -- 6. Closure check for every post-batch card ---------------------
        // Return the first violation so the caller (and wire error) stays
        // small. A user fixing one card and retrying the batch will
        // surface the next one if there is one.
        for (card_id, tags) in &card_tags {
            let missing = graph.missing_for_card(tags);
            if !missing.is_empty() {
                return Err(PushError::TagImplicationViolation {
                    card_id: *card_id,
                    missing,
                }
                .into());
            }
        }

        Ok(())
    }
}
