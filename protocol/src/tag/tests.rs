use chrono::{DateTime, Utc};
use expect_test::expect;
use rgb::RGB8;
use uuid::Uuid;

use crate::hash::Entity;

use super::tag::Tag;

const ID: Uuid = Uuid::from_bytes([1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16]);

fn ts(ms: i64) -> DateTime<Utc> {
    DateTime::from_timestamp_millis(ms).unwrap()
}

fn sample_tag() -> Tag {
    Tag::first(ID, "Groceries".into(), None, ts(1000))
}

#[test]
fn first_has_zero_ancestor() {
    let t = sample_tag();
    expect!["0000000000000000000000000000000000000000000000000000000000000000"]
        .assert_eq(&t.ancestor_hash().to_string());
    expect!["1"].assert_eq(&t.count().to_string());
}

#[test]
fn first_verifies() {
    expect!["true"].assert_eq(&sample_tag().verify().to_string());
}

#[test]
fn next_chains_correctly() {
    let t1 = sample_tag();
    let t2 = t1.next("Groceries (renamed)".into(), None, ts(2000));

    assert_eq!(t1.hash(), t2.ancestor_hash());
    expect!["2"].assert_eq(&t2.count().to_string());
    expect!["1970-01-01 00:00:01 UTC"].assert_eq(&t2.created_at().to_string());
    expect!["1970-01-01 00:00:02 UTC"].assert_eq(&t2.modified_at().to_string());
    expect!["true"].assert_eq(&t2.verify().to_string());
}

#[test]
fn different_titles_produce_different_hashes() {
    let t1 = Tag::first(ID, "Groceries".into(), None, ts(1000));
    let t2 = Tag::first(ID, "Work".into(), None, ts(1000));
    assert_ne!(t1.hash(), t2.hash());
}

#[test]
fn tampered_tag_fails_verification() {
    let mut t = sample_tag();
    t.tamper_title("tampered".into());
    expect!["false"].assert_eq(&t.verify().to_string());
}

#[test]
fn hash_is_deterministic() {
    let t1 = Tag::first(ID, "Groceries".into(), None, ts(1000));
    let t2 = Tag::first(ID, "Groceries".into(), None, ts(1000));
    assert_eq!(t1.hash(), t2.hash());
}

#[test]
fn from_parts_with_valid_hash_succeeds() {
    let tag = Tag::first(ID, "Groceries".into(), None, ts(1000));
    let result = Tag::from_parts(
        tag.id(),
        tag.title().into(),
        tag.color(),
        tag.created_at(),
        tag.modified_at(),
        tag.count(),
        tag.ancestor_hash(),
        tag.hash(),
        tag.implies().to_vec(),
    );
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), tag);
}

#[test]
fn from_parts_with_tampered_hash_fails() {
    let tag = Tag::first(ID, "Groceries".into(), None, ts(1000));
    let result = Tag::from_parts(
        tag.id(),
        "TAMPERED".into(),
        tag.color(),
        tag.created_at(),
        tag.modified_at(),
        tag.count(),
        tag.ancestor_hash(),
        tag.hash(),
        tag.implies().to_vec(),
    );
    assert!(result.is_err());
}

#[test]
fn tag_with_color_verifies() {
    let t = Tag::first(
        ID,
        "Groceries".into(),
        Some(RGB8::new(0xc0, 0x78, 0x30)),
        ts(1000),
    );
    assert!(t.verify());
    assert_eq!(t.color(), Some(RGB8::new(0xc0, 0x78, 0x30)));
}

#[test]
fn tag_without_color_has_none() {
    let t = Tag::first(ID, "Groceries".into(), None, ts(1000));
    assert_eq!(t.color(), None);
}

#[test]
fn different_colors_produce_different_hashes() {
    let t1 = Tag::first(ID, "Groceries".into(), None, ts(1000));
    let t2 = Tag::first(
        ID,
        "Groceries".into(),
        Some(RGB8::new(0xff, 0x00, 0x00)),
        ts(1000),
    );
    let t3 = Tag::first(
        ID,
        "Groceries".into(),
        Some(RGB8::new(0x00, 0xff, 0x00)),
        ts(1000),
    );
    assert_ne!(t1.hash(), t2.hash());
    assert_ne!(t2.hash(), t3.hash());
    assert_ne!(t1.hash(), t3.hash());
}

#[test]
fn next_preserves_color() {
    let t1 = Tag::first(
        ID,
        "Groceries".into(),
        Some(RGB8::new(0xc0, 0x78, 0x30)),
        ts(1000),
    );
    let t2 = t1.next(
        "Groceries (renamed)".into(),
        Some(RGB8::new(0xc0, 0x78, 0x30)),
        ts(2000),
    );
    assert_eq!(t2.color(), Some(RGB8::new(0xc0, 0x78, 0x30)));
    assert!(t2.verify());
}

#[test]
fn next_can_change_color() {
    let t1 = Tag::first(ID, "Groceries".into(), None, ts(1000));
    let t2 = t1.next(
        "Groceries".into(),
        Some(RGB8::new(0xff, 0x00, 0x00)),
        ts(2000),
    );
    assert_eq!(t2.color(), Some(RGB8::new(0xff, 0x00, 0x00)));
    assert!(t2.verify());
}

#[test]
fn next_can_remove_color() {
    let t1 = Tag::first(
        ID,
        "Groceries".into(),
        Some(RGB8::new(0xc0, 0x78, 0x30)),
        ts(1000),
    );
    let t2 = t1.next("Groceries".into(), None, ts(2000));
    assert_eq!(t2.color(), None);
    assert!(t2.verify());
}

#[test]
fn from_parts_with_color_succeeds() {
    let tag = Tag::first(
        ID,
        "Groceries".into(),
        Some(RGB8::new(0xc0, 0x78, 0x30)),
        ts(1000),
    );
    let result = Tag::from_parts(
        tag.id(),
        tag.title().into(),
        tag.color(),
        tag.created_at(),
        tag.modified_at(),
        tag.count(),
        tag.ancestor_hash(),
        tag.hash(),
        tag.implies().to_vec(),
    );
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), tag);
}

#[test]
fn first_empty_implies() {
    let t = Tag::first(ID, "Groceries".into(), None, ts(1000));
    assert!(t.implies().is_empty());
    assert!(t.verify());
}

#[test]
fn first_with_implies_sorts_and_dedups() {
    let p1 = Uuid::from_bytes([0xAA; 16]);
    let p2 = Uuid::from_bytes([0xBB; 16]);
    let t = Tag::first_with_implies(ID, "Child".into(), None, vec![p2, p1, p2], ts(1000));
    assert_eq!(t.implies(), &[p1, p2]);
    assert!(t.verify());
}

#[test]
fn next_preserves_implies_by_default() {
    let p = Uuid::from_bytes([0xAA; 16]);
    let t1 = Tag::first_with_implies(ID, "Child".into(), None, vec![p], ts(1000));
    let t2 = t1.next("Child (renamed)".into(), None, ts(2000));
    assert_eq!(t2.implies(), &[p]);
    assert!(t2.verify());
}

#[test]
fn next_with_implies_can_change_list() {
    let p1 = Uuid::from_bytes([0xAA; 16]);
    let p2 = Uuid::from_bytes([0xBB; 16]);
    let t1 = Tag::first_with_implies(ID, "Child".into(), None, vec![p1], ts(1000));
    let t2 = t1.next_with_implies("Child".into(), None, vec![p2], ts(2000));
    assert_eq!(t2.implies(), &[p2]);
    assert!(t2.verify());
    assert_eq!(t2.ancestor_hash(), t1.hash());
}

#[test]
fn first_empty_implies_hashes_same_as_first() {
    // Locks option (b): empty implies is byte-equivalent to pre-feature.
    let t_old = Tag::first(ID, "X".into(), None, ts(1000));
    let t_new = Tag::first_with_implies(ID, "X".into(), None, vec![], ts(1000));
    assert_eq!(t_old.hash(), t_new.hash());
}

#[test]
fn different_implies_produce_different_hashes() {
    let p = Uuid::from_bytes([0xAA; 16]);
    let a = Tag::first(ID, "X".into(), None, ts(1000));
    let b = Tag::first_with_implies(ID, "X".into(), None, vec![p], ts(1000));
    assert_ne!(a.hash(), b.hash());
}
