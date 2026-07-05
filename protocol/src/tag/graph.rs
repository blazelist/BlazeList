//! Tag implication graph.
//!
//! A [`TagGraph`] stores the direct-parent (`implies`) relation for a
//! set of tags and exposes the two operations the rest of the system
//! needs:
//!
//! - [`TagGraph::closure_of`] — the transitive closure of a set of
//!   tag IDs under the implies relation, which is the set of tags that
//!   any card holding those tags is required to also hold.
//! - [`TagGraph::detect_cycle`] — iterative DFS that returns the first
//!   cycle it finds, used by the server to reject cyclic tag graphs
//!   during push validation.
//!
//! The graph is used both server-side (to enforce the implication
//! invariant on every `PushBatch`) and client-side (to preview which
//! cards would need updating when a tag's implies list changes, and
//! to auto-cascade transitive tags in the card editor).
//!
//! ## No server-side cascade
//!
//! The server **rejects** a card push whose tag set is not closed under
//! the implication graph rather than auto-inserting the missing tags.
//! Clients are expected to call [`TagGraph::missing_for_card`] before
//! pushing and submit a version that already includes the full closure.
//! The rationale: each card version is authored by the client and its
//! BLAKE3 hash is the proof of authorship — if the server fabricated a
//! version to add missing tags, the hash chain would be forged. Callers
//! that want "change implies + bring all affected cards into
//! compliance" atomically should use a single `PushBatch` with every
//! card update the new graph requires (see
//! [`affected_cards_for_change`]).
//!
//! The graph uses `BTreeMap` / `BTreeSet` throughout so that closures
//! and the cycle-detection DFS return deterministic orderings.

use std::collections::{BTreeMap, BTreeSet};

use uuid::Uuid;

use crate::Card;
use crate::hash::Entity;
use crate::tag::tag::Tag;

/// In-memory view of the tag implication graph.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TagGraph {
    /// Direct parents per tag. Missing keys are treated as "no implies".
    edges: BTreeMap<Uuid, Vec<Uuid>>,
}

/// A cycle discovered by [`TagGraph::detect_cycle`]. The `cycle` vector
/// lists the tag IDs along the cycle starting from the first node that
/// closes the loop, and ends with the same node so the round-trip is
/// explicit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TagImplicationCycle {
    pub cycle: Vec<Uuid>,
}

impl TagGraph {
    /// Build a graph from a slice of [`Tag`]s. Only each tag's
    /// `implies()` list is consulted — other tag fields are ignored.
    pub fn from_tags(tags: &[Tag]) -> Self {
        let mut edges = BTreeMap::new();
        for tag in tags {
            edges.insert(tag.id(), tag.implies().to_vec());
        }
        Self { edges }
    }

    /// Build a graph directly from `(id, implies)` pairs. Useful for
    /// the server-side snapshot that reads out of SQLite row-by-row.
    pub fn from_pairs<I: IntoIterator<Item = (Uuid, Vec<Uuid>)>>(pairs: I) -> Self {
        Self {
            edges: pairs.into_iter().collect(),
        }
    }

    /// Add or replace a tag's implies list.
    pub fn upsert(&mut self, id: Uuid, implies: Vec<Uuid>) {
        self.edges.insert(id, implies);
    }

    /// Remove a tag from the graph entirely.
    pub fn remove(&mut self, id: &Uuid) {
        self.edges.remove(id);
    }

    /// Direct implies (parent) list for a tag, or empty if the tag is
    /// not in the graph.
    pub fn implies_of(&self, id: &Uuid) -> &[Uuid] {
        self.edges.get(id).map(|v| v.as_slice()).unwrap_or(&[])
    }

    /// Direct implies (parent) list for a tag, sorted. Used by
    /// [`TagGraph::detect_cycle`] to walk children in a stable order so
    /// the reported cycle is deterministic. The sort is load-bearing.
    fn sorted_implies(&self, id: &Uuid) -> Vec<Uuid> {
        let mut p = self.implies_of(id).to_vec();
        p.sort();
        p
    }

    /// Whether the graph knows about a tag at all. Used by callers that
    /// want to distinguish "tag has no implies" from "tag does not
    /// exist" — e.g. orphan-reference detection.
    pub fn contains(&self, id: &Uuid) -> bool {
        self.edges.contains_key(id)
    }

    /// Number of tags in the graph.
    pub fn len(&self) -> usize {
        self.edges.len()
    }

    /// Whether the graph contains no tags.
    pub fn is_empty(&self) -> bool {
        self.edges.is_empty()
    }

    /// Transitive closure of `roots` under the implies relation.
    ///
    /// The returned set includes the roots themselves plus every tag
    /// reachable by following `implies` edges. Missing edges are
    /// treated as empty lists (so referencing a tag that is not in the
    /// graph is silently tolerated at this layer — the caller decides
    /// whether that is a violation).
    pub fn closure_of(&self, roots: &[Uuid]) -> BTreeSet<Uuid> {
        let mut visited: BTreeSet<Uuid> = BTreeSet::new();
        let mut stack: Vec<Uuid> = roots.to_vec();
        while let Some(id) = stack.pop() {
            if !visited.insert(id) {
                continue;
            }
            for parent in self.implies_of(&id) {
                if !visited.contains(parent) {
                    stack.push(*parent);
                }
            }
        }
        visited
    }

    /// For a card with `current_tags`, returns the sorted list of tag
    /// IDs that need to be added to satisfy the implication invariant.
    /// Returns an empty `Vec` if the current set is already closed.
    pub fn missing_for_card(&self, current_tags: &[Uuid]) -> Vec<Uuid> {
        let closure = self.closure_of(current_tags);
        let have: BTreeSet<Uuid> = current_tags.iter().copied().collect();
        closure.difference(&have).copied().collect()
    }

    /// Iterative DFS cycle detection. Returns the first cycle found, or
    /// `None` if the graph is acyclic. The returned cycle starts and
    /// ends with the same node id for clarity.
    pub fn detect_cycle(&self) -> Option<TagImplicationCycle> {
        #[derive(Clone, Copy, PartialEq, Eq)]
        enum Color {
            White,
            Gray,
            Black,
        }

        let mut color: BTreeMap<Uuid, Color> = self
            .edges
            .keys()
            .copied()
            .map(|id| (id, Color::White))
            .collect();

        // Stack entries: (current_node, parent_iterator_index).
        // We walk children in sorted order so the result is stable.
        let mut stack: Vec<(Uuid, usize, Vec<Uuid>)> = Vec::new();
        // path: gray stack so we can reconstruct a cycle when one is found.
        let mut path: Vec<Uuid> = Vec::new();

        for &start in self.edges.keys() {
            if color.get(&start).copied() != Some(Color::White) {
                continue;
            }
            color.insert(start, Color::Gray);
            path.push(start);
            let parents = self.sorted_implies(&start);
            stack.push((start, 0, parents));

            while let Some((node, idx, parents)) = stack.last_mut() {
                if *idx >= parents.len() {
                    color.insert(*node, Color::Black);
                    path.pop();
                    stack.pop();
                    continue;
                }
                let next = parents[*idx];
                *idx += 1;

                match color.get(&next).copied().unwrap_or(Color::White) {
                    Color::White => {
                        color.insert(next, Color::Gray);
                        path.push(next);
                        let next_parents = self.sorted_implies(&next);
                        stack.push((next, 0, next_parents));
                    }
                    Color::Gray => {
                        // Found a back-edge: `next` is already on the
                        // current DFS path. Slice out the cycle.
                        let start_idx = path.iter().position(|id| *id == next).unwrap_or(0);
                        let mut cycle: Vec<Uuid> = path[start_idx..].to_vec();
                        cycle.push(next);
                        return Some(TagImplicationCycle { cycle });
                    }
                    Color::Black => {
                        // Already fully explored — no cycle through here.
                    }
                }
            }
        }
        None
    }
}

/// Compute the list of cards that would need new versions to remain
/// compliant with the invariant under `next`. Returns, for each affected
/// card, the sorted list of tag IDs that must be added to the card's tag
/// set.
///
/// A card that is already closed under `next` is skipped. Callers that
/// want diff semantics against a previous graph can compute
/// `prev.missing_for_card` and `next.missing_for_card` themselves — the
/// closure under `next` is all this function needs.
pub fn affected_cards_for_change(next: &TagGraph, cards: &[Card]) -> Vec<(Uuid, Vec<Uuid>)> {
    let mut affected: Vec<(Uuid, Vec<Uuid>)> = Vec::new();
    for card in cards {
        let missing = next.missing_for_card(card.tags());
        if !missing.is_empty() {
            affected.push((card.id(), missing));
        }
    }
    affected
}
