use super::card::Card;
use crate::NonNegativeI64;
use crate::hash::Entity;
use chrono::{DateTime, Utc};
use expect_test::expect;
use uuid::Uuid;

const ID: Uuid = Uuid::from_bytes([1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16]);
const ID_B: Uuid = Uuid::from_bytes([16, 15, 14, 13, 12, 11, 10, 9, 8, 7, 6, 5, 4, 3, 2, 1]);
const TAG_ID: Uuid = Uuid::from_bytes([
    0xa1, 0xb2, 0xc3, 0xd4, 0xe5, 0xf6, 0x47, 0x08, 0x89, 0x0a, 0x0b, 0x0c, 0x0d, 0x0e, 0x0f, 0x10,
]);

fn ts(ms: i64) -> DateTime<Utc> {
    DateTime::from_timestamp_millis(ms).unwrap()
}

fn p(v: i64) -> NonNegativeI64 {
    NonNegativeI64::try_from(v).unwrap()
}

fn sample_card() -> Card {
    Card::first(
        ID,
        "Some markdown content".into(),
        p(100),
        vec![],
        false,
        ts(1000),
        None,
    )
}

#[test]
fn first_has_zero_ancestor() {
    let c = sample_card();
    expect!["0000000000000000000000000000000000000000000000000000000000000000"]
        .assert_eq(&c.ancestor_hash().to_string());
    expect!["1"].assert_eq(&c.count().to_string());
}

#[test]
fn first_verifies() {
    expect!["true"].assert_eq(&sample_card().verify().to_string());
}

#[test]
fn next_chains_correctly() {
    let c1 = sample_card();
    let c2 = c1.next(
        "Updated content".into(),
        p(100),
        vec![],
        false,
        ts(2000),
        None,
    );

    expect!["fc58274bbf070dbc559b6ca207d7e6267d4296dc108f3c9effc00df21eee18b2"]
        .assert_eq(&c1.hash().to_string());
    expect!["fc58274bbf070dbc559b6ca207d7e6267d4296dc108f3c9effc00df21eee18b2"]
        .assert_eq(&c2.ancestor_hash().to_string());
    expect!["2"].assert_eq(&c2.count().to_string());
    expect!["1970-01-01 00:00:01 UTC"].assert_eq(&c2.created_at().to_string());
    expect!["1970-01-01 00:00:02 UTC"].assert_eq(&c2.modified_at().to_string());
    expect!["true"].assert_eq(&c2.verify().to_string());
}

#[test]
fn different_content_produces_different_hash() {
    let c1 = Card::first(
        ID,
        "Content A".into(),
        p(100),
        vec![],
        false,
        ts(1000),
        None,
    );
    let c2 = Card::first(
        ID,
        "Content B".into(),
        p(100),
        vec![],
        false,
        ts(1000),
        None,
    );
    assert_ne!(c1.hash(), c2.hash());
}

#[test]
fn different_ids_produce_different_hashes() {
    let c1 = Card::first(ID, "C".into(), p(100), vec![], false, ts(1000), None);
    let c2 = Card::first(ID_B, "C".into(), p(100), vec![], false, ts(1000), None);
    assert_ne!(c1.hash(), c2.hash());
}

#[test]
fn tampered_card_fails_verification() {
    let c = sample_card();
    // Reconstruct with tampered content but same hash — verification should fail
    let result = Card::from_parts(
        c.id(),
        "tampered".into(),
        c.priority(),
        c.tags().to_vec(),
        c.blazed(),
        c.created_at(),
        c.modified_at(),
        c.count(),
        c.ancestor_hash(),
        c.hash(),
        c.due_date(),
    );
    assert!(result.is_err());
}

#[test]
fn hash_is_deterministic() {
    let c1 = Card::first(ID, "C".into(), p(100), vec![], false, ts(1000), None);
    let c2 = Card::first(ID, "C".into(), p(100), vec![], false, ts(1000), None);
    expect!["68e2d3eb485be81518b8f22169fd34c0e863dcef45247b66bec0f3172fd9821b"]
        .assert_eq(&c1.hash().to_string());
    expect!["68e2d3eb485be81518b8f22169fd34c0e863dcef45247b66bec0f3172fd9821b"]
        .assert_eq(&c2.hash().to_string());
}

#[test]
fn tags_affect_hash() {
    let c1 = Card::first(ID, "C".into(), p(100), vec![], false, ts(1000), None);
    let c2 = Card::first(ID, "C".into(), p(100), vec![TAG_ID], false, ts(1000), None);
    assert_ne!(c1.hash(), c2.hash());
}

#[test]
fn count_affects_hash() {
    let c1 = Card::first(ID, "C".into(), p(1), vec![], false, ts(0), None);
    let c2 = Card::first(ID, "C".into(), p(1), vec![], false, ts(0), None);
    // c1 and c2 are identical first versions, so same hash
    // The original test mutated a private field; instead verify that
    // chaining (which increments count) produces a different hash.
    let c3 = c2.next("C".into(), p(1), vec![], false, ts(1), None);
    assert_ne!(c1.hash(), c3.hash());
}

#[test]
fn from_parts_with_valid_hash_succeeds() {
    let card = Card::first(ID, "Content".into(), p(100), vec![], false, ts(1000), None);
    let result = Card::from_parts(
        card.id(),
        card.content().into(),
        card.priority(),
        card.tags().to_vec(),
        card.blazed(),
        card.created_at(),
        card.modified_at(),
        card.count(),
        card.ancestor_hash(),
        card.hash(),
        card.due_date(),
    );
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), card);
}

#[test]
fn from_parts_with_tampered_hash_fails() {
    let card = Card::first(ID, "Content".into(), p(100), vec![], false, ts(1000), None);
    let result = Card::from_parts(
        card.id(),
        "TAMPERED".into(),
        card.priority(),
        card.tags().to_vec(),
        card.blazed(),
        card.created_at(),
        card.modified_at(),
        card.count(),
        card.ancestor_hash(),
        card.hash(),
        card.due_date(),
    );
    assert!(result.is_err());
}

#[test]
fn due_date_affects_hash() {
    let c1 = Card::first(ID, "C".into(), p(100), vec![], false, ts(1000), None);
    let c2 = Card::first(
        ID,
        "C".into(),
        p(100),
        vec![],
        false,
        ts(1000),
        Some(ts(5000)),
    );
    assert_ne!(c1.hash(), c2.hash());
    assert_eq!(c1.due_date(), None);
    assert_eq!(c2.due_date(), Some(ts(5000)));
}
