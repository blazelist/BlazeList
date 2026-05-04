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
            let parents = {
                let mut p = self.implies_of(&start).to_vec();
                p.sort();
                p
            };
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
                        let next_parents = {
                            let mut p = self.implies_of(&next).to_vec();
                            p.sort();
                            p
                        };
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

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{DateTime, Utc};
    use uuid::Uuid;

    const A: Uuid = Uuid::from_bytes([0xAA; 16]);
    const B: Uuid = Uuid::from_bytes([0xBB; 16]);
    const C: Uuid = Uuid::from_bytes([0xCC; 16]);
    const D: Uuid = Uuid::from_bytes([0xDD; 16]);
    const E: Uuid = Uuid::from_bytes([0xEE; 16]);

    fn ts(ms: i64) -> DateTime<Utc> {
        DateTime::from_timestamp_millis(ms).unwrap()
    }

    fn tag(id: Uuid, implies: Vec<Uuid>) -> Tag {
        Tag::first_with_implies(id, format!("{id:x}"), None, implies, ts(0))
    }

    fn graph_of(pairs: &[(Uuid, Vec<Uuid>)]) -> TagGraph {
        TagGraph::from_pairs(pairs.iter().cloned())
    }

    #[test]
    fn closure_of_empty_returns_roots() {
        let g = graph_of(&[(A, vec![]), (B, vec![])]);
        let closure = g.closure_of(&[A]);
        assert_eq!(closure.into_iter().collect::<Vec<_>>(), vec![A]);
    }

    #[test]
    fn closure_of_chain_a_implies_b_implies_c() {
        let g = graph_of(&[(A, vec![B]), (B, vec![C]), (C, vec![])]);
        let closure = g.closure_of(&[A]);
        let sorted: Vec<Uuid> = closure.into_iter().collect();
        assert_eq!(sorted, vec![A, B, C]);
    }

    #[test]
    fn closure_of_diamond_no_duplicates() {
        // A → B, A → C, B → D, C → D. Expect {A,B,C,D}.
        let g = graph_of(&[(A, vec![B, C]), (B, vec![D]), (C, vec![D]), (D, vec![])]);
        let closure: Vec<Uuid> = g.closure_of(&[A]).into_iter().collect();
        assert_eq!(closure, vec![A, B, C, D]);
    }

    #[test]
    fn closure_of_multiple_roots_unions() {
        let g = graph_of(&[(A, vec![B]), (B, vec![]), (C, vec![D]), (D, vec![])]);
        let closure: Vec<Uuid> = g.closure_of(&[A, C]).into_iter().collect();
        assert_eq!(closure, vec![A, B, C, D]);
    }

    #[test]
    fn missing_for_card_returns_sorted_unique() {
        let g = graph_of(&[(A, vec![B, C]), (B, vec![]), (C, vec![])]);
        let missing = g.missing_for_card(&[A]);
        assert_eq!(missing, vec![B, C]);
    }

    #[test]
    fn missing_for_card_empty_when_already_closed() {
        let g = graph_of(&[(A, vec![B]), (B, vec![])]);
        assert!(g.missing_for_card(&[A, B]).is_empty());
    }

    #[test]
    fn detect_cycle_self_loop() {
        let g = graph_of(&[(A, vec![A])]);
        let cycle = g.detect_cycle().unwrap();
        assert_eq!(cycle.cycle, vec![A, A]);
    }

    #[test]
    fn detect_cycle_two_node() {
        let g = graph_of(&[(A, vec![B]), (B, vec![A])]);
        let cycle = g.detect_cycle().unwrap();
        assert_eq!(cycle.cycle.first(), cycle.cycle.last());
        assert!(cycle.cycle.contains(&A));
        assert!(cycle.cycle.contains(&B));
    }

    #[test]
    fn detect_cycle_long_chain_then_back_edge() {
        // A → B → C → D → B creates a cycle (B,C,D,B).
        let g = graph_of(&[(A, vec![B]), (B, vec![C]), (C, vec![D]), (D, vec![B])]);
        let cycle = g.detect_cycle().unwrap();
        assert_eq!(cycle.cycle.first(), cycle.cycle.last());
        assert!(cycle.cycle.contains(&B));
    }

    #[test]
    fn detect_cycle_none_on_dag() {
        let g = graph_of(&[(A, vec![B, C]), (B, vec![D]), (C, vec![D]), (D, vec![])]);
        assert!(g.detect_cycle().is_none());
    }

    #[test]
    fn upsert_replaces_entry() {
        let mut g = graph_of(&[(A, vec![B])]);
        g.upsert(A, vec![C]);
        assert_eq!(g.implies_of(&A), &[C]);
    }

    #[test]
    fn remove_drops_tag() {
        let mut g = graph_of(&[(A, vec![B]), (B, vec![])]);
        g.remove(&A);
        assert!(!g.contains(&A));
        // Closure still terminates cleanly for an absent tag.
        let closure: Vec<Uuid> = g.closure_of(&[A]).into_iter().collect();
        assert_eq!(closure, vec![A]);
    }

    #[test]
    fn from_tags_builds_graph() {
        let t_a = tag(A, vec![B]);
        let t_b = tag(B, vec![]);
        let g = TagGraph::from_tags(&[t_a, t_b]);
        assert_eq!(g.implies_of(&A), &[B]);
    }

    #[test]
    fn affected_cards_for_change_skips_compliant_cards() {
        // Card already has {A, B}; new graph demands only that A implies B.
        let next = graph_of(&[(A, vec![B]), (B, vec![])]);
        let card = Card::first(E, "c".into(), 1, vec![A, B], false, ts(0), None);
        let affected = affected_cards_for_change(&next, &[card]);
        assert!(affected.is_empty());
    }

    #[test]
    fn affected_cards_for_change_reports_additions() {
        let next = graph_of(&[(A, vec![B]), (B, vec![])]);
        let card = Card::first(E, "c".into(), 1, vec![A], false, ts(0), None);
        let affected = affected_cards_for_change(&next, &[card]);
        assert_eq!(affected.len(), 1);
        assert_eq!(affected[0].1, vec![B]);
    }

    #[test]
    fn affected_cards_for_change_ignores_untouched_cards() {
        let next = graph_of(&[(A, vec![B]), (B, vec![])]);
        // This card does not reference A at all — no change required.
        let card = Card::first(E, "c".into(), 1, vec![C], false, ts(0), None);
        let affected = affected_cards_for_change(&next, &[card]);
        assert!(affected.is_empty());
    }
}
