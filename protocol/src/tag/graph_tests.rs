use chrono::{DateTime, Utc};
use uuid::Uuid;

use super::graph::{TagGraph, affected_cards_for_change};
use super::tag::Tag;
use crate::Card;

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
fn detect_cycle_multiple_back_edges_uses_sorted_parents() {
    // A → [B, C], B → A, C → A. A has two distinct back-edges (via B
    // and via C). The load-bearing parent sort (graph.rs:176/197)
    // guarantees A's parents are walked in sorted order (B before C,
    // since B < C), so the DFS reaches the back-edge through B first
    // and reports the deterministic cycle [A, B, A] rather than the
    // C-path cycle.
    let g = graph_of(&[(A, vec![B, C]), (B, vec![A]), (C, vec![A])]);
    let cycle = g.detect_cycle().unwrap();
    assert_eq!(cycle.cycle, vec![A, B, A]);
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
