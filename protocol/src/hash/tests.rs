use super::*;
use expect_test::expect;
use rgb::RGB8;
use uuid::Uuid;

#[test]
fn zero_hash_is_all_zeros() {
    expect!["0000000000000000000000000000000000000000000000000000000000000000"]
        .assert_eq(&ZERO_HASH.to_string());
}

#[test]
fn canonical_card_hash_is_deterministic() {
    let id = Uuid::from_bytes([1; 16]);
    let a = canonical_card_hash(
        &id,
        "hello",
        42,
        &[],
        false,
        1000,
        2000,
        1,
        &ZERO_HASH,
        None,
    );
    let b = canonical_card_hash(
        &id,
        "hello",
        42,
        &[],
        false,
        1000,
        2000,
        1,
        &ZERO_HASH,
        None,
    );
    assert_eq!(a, b);
}

#[test]
fn canonical_card_hash_different_content_different_hash() {
    let id = Uuid::from_bytes([1; 16]);
    let a = canonical_card_hash(
        &id,
        "hello",
        42,
        &[],
        false,
        1000,
        2000,
        1,
        &ZERO_HASH,
        None,
    );
    let b = canonical_card_hash(
        &id,
        "world",
        42,
        &[],
        false,
        1000,
        2000,
        1,
        &ZERO_HASH,
        None,
    );
    assert_ne!(a, b);
}

#[test]
fn canonical_card_hash_different_tags_different_hash() {
    let id = Uuid::from_bytes([1; 16]);
    let tag = Uuid::from_bytes([2; 16]);
    let a = canonical_card_hash(&id, "c", 0, &[], false, 0, 0, 1, &ZERO_HASH, None);
    let b = canonical_card_hash(&id, "c", 0, &[tag], false, 0, 0, 1, &ZERO_HASH, None);
    assert_ne!(a, b);
}

#[test]
fn canonical_card_hash_different_priority_different_hash() {
    let id = Uuid::from_bytes([1; 16]);
    let a = canonical_card_hash(&id, "c", 1, &[], false, 0, 0, 1, &ZERO_HASH, None);
    let b = canonical_card_hash(&id, "c", 2, &[], false, 0, 0, 1, &ZERO_HASH, None);
    assert_ne!(a, b);
}

#[test]
fn canonical_card_hash_different_blazed_different_hash() {
    let id = Uuid::from_bytes([1; 16]);
    let a = canonical_card_hash(&id, "c", 0, &[], false, 0, 0, 1, &ZERO_HASH, None);
    let b = canonical_card_hash(&id, "c", 0, &[], true, 0, 0, 1, &ZERO_HASH, None);
    assert_ne!(a, b);
}

#[test]
fn canonical_card_hash_snapshot() {
    // Pin down a specific hash value to detect accidental changes to the
    // canonical format.
    let id = Uuid::from_bytes([1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16]);
    let tag = Uuid::from_bytes([
        0xa1, 0xb2, 0xc3, 0xd4, 0xe5, 0xf6, 0x47, 0x08, 0x89, 0x0a, 0x0b, 0x0c, 0x0d, 0x0e, 0x0f,
        0x10,
    ]);
    let hash = canonical_card_hash(
        &id,
        "- Tofu\n- Lentils",
        5000,
        &[tag],
        false,
        1000,
        2000,
        1,
        &ZERO_HASH,
        None,
    );
    expect!["3aa7f4cdbc1d562f3a27adba152e6e43dd8612bcdf5e8f8ac91daf343a9d3848"]
        .assert_eq(&hash.to_string());
}

#[test]
fn canonical_card_hash_different_due_date_different_hash() {
    let id = Uuid::from_bytes([1; 16]);
    let a = canonical_card_hash(&id, "c", 0, &[], false, 0, 0, 1, &ZERO_HASH, None);
    let b = canonical_card_hash(
        &id,
        "c",
        0,
        &[],
        false,
        0,
        0,
        1,
        &ZERO_HASH,
        Some(1_000_000),
    );
    let c = canonical_card_hash(
        &id,
        "c",
        0,
        &[],
        false,
        0,
        0,
        1,
        &ZERO_HASH,
        Some(2_000_000),
    );
    assert_ne!(a, b);
    assert_ne!(b, c);
    assert_ne!(a, c);
}

#[test]
fn canonical_tag_hash_is_deterministic() {
    let id = Uuid::from_bytes([2; 16]);
    let a = canonical_tag_hash(&id, "Groceries", 1000, 2000, 1, &ZERO_HASH, None);
    let b = canonical_tag_hash(&id, "Groceries", 1000, 2000, 1, &ZERO_HASH, None);
    assert_eq!(a, b);
}

#[test]
fn canonical_tag_hash_different_title_different_hash() {
    let id = Uuid::from_bytes([2; 16]);
    let a = canonical_tag_hash(&id, "Groceries", 0, 0, 1, &ZERO_HASH, None);
    let b = canonical_tag_hash(&id, "Work", 0, 0, 1, &ZERO_HASH, None);
    assert_ne!(a, b);
}

#[test]
fn canonical_tag_hash_snapshot() {
    let id = Uuid::from_bytes([
        0xa1, 0xb2, 0xc3, 0xd4, 0xe5, 0xf6, 0x47, 0x08, 0x89, 0x0a, 0x0b, 0x0c, 0x0d, 0x0e, 0x0f,
        0x10,
    ]);
    let hash = canonical_tag_hash(&id, "Groceries", 1000, 2000, 1, &ZERO_HASH, None);
    expect!["d2aeffc782308defcbd56988109ab8db28254a77f0f63de027442a4d777cc8d5"]
        .assert_eq(&hash.to_string());
}

#[test]
fn canonical_tag_hash_different_color_different_hash() {
    let id = Uuid::from_bytes([2; 16]);
    let a = canonical_tag_hash(&id, "Groceries", 0, 0, 1, &ZERO_HASH, None);
    let b = canonical_tag_hash(
        &id,
        "Groceries",
        0,
        0,
        1,
        &ZERO_HASH,
        Some(&RGB8::new(0xff, 0x00, 0x00)),
    );
    let c = canonical_tag_hash(
        &id,
        "Groceries",
        0,
        0,
        1,
        &ZERO_HASH,
        Some(&RGB8::new(0x00, 0xff, 0x00)),
    );
    assert_ne!(a, b);
    assert_ne!(b, c);
    assert_ne!(a, c);
}

#[test]
fn canonical_card_hash_matches_manual_computation() {
    let id = Uuid::from_bytes([1; 16]);
    let content = "hello";
    let priority: i64 = 42;
    let tags: &[Uuid] = &[];
    let blazed = false;
    let created_at_ms: i64 = 1000;
    let modified_at_ms: i64 = 2000;
    let count: i64 = 1;
    let ancestor_hash = &ZERO_HASH;

    let mut hasher = blake3::Hasher::new();
    hasher.update(id.as_bytes());
    hasher.update(&(content.len() as u64).to_be_bytes());
    hasher.update(content.as_bytes());
    hasher.update(&priority.to_be_bytes());
    hasher.update(&(tags.len() as u64).to_be_bytes());
    hasher.update(&[blazed as u8]);
    hasher.update(&created_at_ms.to_be_bytes());
    hasher.update(&modified_at_ms.to_be_bytes());
    hasher.update(&[0u8]); // no due_date
    hasher.update(&count.to_be_bytes());
    hasher.update(ancestor_hash.as_bytes());
    let expected = hasher.finalize();

    let actual = canonical_card_hash(
        &id,
        content,
        priority,
        tags,
        blazed,
        created_at_ms,
        modified_at_ms,
        count,
        ancestor_hash,
        None,
    );
    assert_eq!(actual, expected);
}
