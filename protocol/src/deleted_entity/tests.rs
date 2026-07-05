use super::DeletedEntity;
use expect_test::expect;
use uuid::Uuid;

const ID: Uuid = Uuid::from_bytes([1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16]);
const ID_B: Uuid = Uuid::from_bytes([16, 15, 14, 13, 12, 11, 10, 9, 8, 7, 6, 5, 4, 3, 2, 1]);

#[test]
fn deleted_entity_verifies() {
    let entity = DeletedEntity::new(ID);
    expect!["true"].assert_eq(&entity.verify().to_string());
}

#[test]
fn tampered_deleted_entity_fails_verification() {
    let mut entity = DeletedEntity::new(ID);
    entity.tamper_id(ID_B);
    expect!["false"].assert_eq(&entity.verify().to_string());
}

#[test]
fn different_ids_produce_different_hashes() {
    let a = DeletedEntity::new(ID);
    let b = DeletedEntity::new(ID_B);
    assert_ne!(a.hash(), b.hash());
}

#[test]
fn from_parts_stores_id_and_hash_verbatim() {
    // from_parts trusts the caller and does NO verification at
    // construction — it must store whatever id and hash it is given,
    // even a mismatched pair.
    let hash = blake3::hash(ID_B.as_bytes());
    let e = DeletedEntity::from_parts(ID, hash);
    assert_eq!(e.id(), ID);
    assert_eq!(e.hash(), hash);
}

#[test]
fn from_parts_with_correct_hash_verifies() {
    // A DeletedEntity built via from_parts with the correct BLAKE3(uuid)
    // hash subsequently passes verify() — "no verification at
    // construction, verifiable later".
    let hash = blake3::hash(ID.as_bytes());
    let e = DeletedEntity::from_parts(ID, hash);
    expect!["true"].assert_eq(&e.verify().to_string());
}

#[test]
fn from_parts_with_mismatched_hash_fails_verification() {
    // from_parts does not reject a wrong hash at construction, but
    // verify() must return false for it.
    let wrong_hash = blake3::hash(ID_B.as_bytes());
    let e = DeletedEntity::from_parts(ID, wrong_hash);
    expect!["false"].assert_eq(&e.verify().to_string());
}
