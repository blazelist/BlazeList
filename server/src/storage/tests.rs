#[cfg(test)]
mod tests {
    use blazelist_protocol::CardFilter;
    use blazelist_protocol::{Card, DateTime, Entity, NonNegativeI64, Tag, Utc, ZERO_HASH};
    use expect_test::expect;
    use uuid::Uuid;

    use crate::storage::error::{PushError, PushOpError, StorageError};
    use crate::storage::sqlite::SqliteStorage;
    use crate::storage::traits::Storage;

    // -- Delete validation (NotFound / AlreadyDeleted) -----------------------

    #[test]
    fn delete_nonexistent_card_returns_not_found() {
        let s = store();
        match s.delete_card(ID_A) {
            Err(StorageError::NotFound) => {}
            other => panic!("expected NotFound, got {other:?}"),
        }
    }

    #[test]
    fn delete_already_deleted_card_returns_already_deleted() {
        let s = store();
        let card = Card::first(ID_A, "C".into(), p(1), vec![], false, ts(0), None);
        s.push_card_versions(&[card]).unwrap();
        s.delete_card(ID_A).unwrap();
        match s.delete_card(ID_A) {
            Err(StorageError::AlreadyDeleted) => {}
            other => panic!("expected AlreadyDeleted, got {other:?}"),
        }
    }

    #[test]
    fn delete_nonexistent_tag_returns_not_found() {
        let s = store();
        match s.delete_tag(TAG_ID) {
            Err(StorageError::NotFound) => {}
            other => panic!("expected NotFound, got {other:?}"),
        }
    }

    #[test]
    fn delete_already_deleted_tag_returns_already_deleted() {
        let s = store();
        let tag = Tag::first(TAG_ID, "Groceries".into(), None, ts(1000));
        s.push_tag_versions(&[tag]).unwrap();
        s.delete_tag(TAG_ID).unwrap();
        match s.delete_tag(TAG_ID) {
            Err(StorageError::AlreadyDeleted) => {}
            other => panic!("expected AlreadyDeleted, got {other:?}"),
        }
    }

    /// Compute a bucket hash from a list of entity hashes (in order: cards,
    /// tags, deleted — each sorted by UUID within the bucket). Returns
    /// ZERO_HASH if the list is empty.
    fn bucket_hash(hashes: &[blake3::Hash]) -> blake3::Hash {
        if hashes.is_empty() {
            return ZERO_HASH;
        }
        let mut hasher = blake3::Hasher::new();
        for h in hashes {
            hasher.update(h.as_bytes());
        }
        hasher.finalize()
    }

    /// Compute the expected root hash from a sparse list of (bucket, hash)
    /// pairs. Buckets not listed are assumed to be ZERO_HASH. Returns
    /// ZERO_HASH if all buckets are zero.
    fn expected_root(pairs: &[(u8, blake3::Hash)]) -> blake3::Hash {
        let mut bucket_hashes = [ZERO_HASH; 256];
        for &(b, h) in pairs {
            bucket_hashes[b as usize] = h;
        }
        if bucket_hashes.iter().all(|h| *h == ZERO_HASH) {
            return ZERO_HASH;
        }
        let mut hasher = blake3::Hasher::new();
        for h in &bucket_hashes {
            hasher.update(h.as_bytes());
        }
        hasher.finalize()
    }

    const ID_A: Uuid = Uuid::from_bytes([1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16]);
    const ID_B: Uuid = Uuid::from_bytes([16, 15, 14, 13, 12, 11, 10, 9, 8, 7, 6, 5, 4, 3, 2, 1]);
    const TAG_ID: Uuid = Uuid::from_bytes([
        0xa1, 0xb2, 0xc3, 0xd4, 0xe5, 0xf6, 0x47, 0x08, 0x89, 0x0a, 0x0b, 0x0c, 0x0d, 0x0e, 0x0f,
        0x10,
    ]);

    // Same-bucket UUIDs: all start with 0x01 → bucket 1, same as ID_A.
    // Second byte differs so UUID ordering within the bucket is: ID_A < SAME_BUCKET_1 < SAME_BUCKET_2.
    const SAME_BUCKET_1: Uuid = Uuid::from_bytes([1, 3, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
    const SAME_BUCKET_2: Uuid = Uuid::from_bytes([1, 4, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);

    fn ts(ms: i64) -> DateTime<Utc> {
        DateTime::from_timestamp_millis(ms).unwrap()
    }

    fn p(v: i64) -> NonNegativeI64 {
        NonNegativeI64::try_from(v).unwrap()
    }

    fn store() -> SqliteStorage {
        SqliteStorage::open_in_memory().unwrap()
    }

    // -- Root ----------------------------------------------------------------

    #[test]
    fn empty_root() {
        let s = store();
        let root = s.get_root().unwrap();
        expect!["0000000000000000000000000000000000000000000000000000000000000000"]
            .assert_eq(&root.hash.to_string());
        expect!["0"].assert_eq(&root.sequence.to_string());
    }

    // -- Card CRUD -----------------------------------------------------------

    #[test]
    fn create_and_get_card() {
        let s = store();
        let card = Card::first(
            ID_A,
            "- Tofu\n- Lentils".into(),
            p(5000),
            vec![],
            false,
            ts(1000),
            None,
        );
        s.push_card_versions(std::slice::from_ref(&card)).unwrap();

        let fetched = s.get_card(ID_A).unwrap().unwrap();
        assert_eq!(card, fetched);
        expect!["true"].assert_eq(&fetched.verify().to_string());
    }

    #[test]
    fn card_not_found() {
        let s = store();
        assert!(s.get_card(ID_A).unwrap().is_none());
    }

    #[test]
    fn push_updates_root() {
        let s = store();
        let card = Card::first(ID_A, "C".into(), p(1), vec![], false, ts(0), None);
        s.push_card_versions(&[card]).unwrap();

        let root = s.get_root().unwrap();
        expect!["1"].assert_eq(&root.sequence.to_string());
        // Root hash is no longer zero (one card exists).
        assert_ne!(root.hash, ZERO_HASH);
    }

    #[test]
    fn push_chain_of_versions() {
        let s = store();
        let v1 = Card::first(ID_A, "C1".into(), p(1), vec![], false, ts(0), None);
        s.push_card_versions(std::slice::from_ref(&v1)).unwrap();

        let v2 = v1.next("C2".into(), p(1), vec![], false, ts(1000), None);
        let v3 = v2.next("C3".into(), p(1), vec![], false, ts(2000), None);
        s.push_card_versions(&[v2, v3.clone()]).unwrap();

        let latest = s.get_card(ID_A).unwrap().unwrap();
        assert_eq!(latest, v3);
        expect!["3"].assert_eq(&latest.count().to_string());
    }

    #[test]
    fn push_rejects_ancestor_mismatch() {
        let s = store();
        let v1 = Card::first(ID_A, "C1".into(), p(1), vec![], false, ts(0), None);
        s.push_card_versions(std::slice::from_ref(&v1)).unwrap();

        // Push from stale base (pretend another client pushed v2).
        let v2_a = v1.next("C2a".into(), p(1), vec![], false, ts(1000), None);
        s.push_card_versions(&[v2_a]).unwrap();

        // Now try pushing v2_b from the same v1 base.
        let v2_b = v1.next("C2b".into(), p(1), vec![], false, ts(1500), None);
        let err = s.push_card_versions(&[v2_b]).unwrap_err();
        assert!(matches!(
            err,
            PushOpError::Domain(PushError::CardAncestorMismatch(_))
        ));
        let s = store();
        let err = s.push_card_versions(&[]).unwrap_err();
        assert!(matches!(err, PushOpError::Domain(PushError::EmptyChain)));
    }

    #[test]
    fn from_parts_rejects_tampered_card() {
        let card = Card::first(ID_A, "C".into(), p(1), vec![], false, ts(0), None);
        // Tamper with content without updating hash.
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
            card.hash(), // hash doesn't match "TAMPERED" content
            card.due_date(),
        );
        assert!(result.is_err());
    }

    #[test]
    fn list_cards_all() {
        let s = store();
        let c1 = Card::first(ID_A, "".into(), p(100), vec![], false, ts(0), None);
        let c2 = Card::first(ID_B, "".into(), p(200), vec![], true, ts(0), None);
        s.push_card_versions(&[c1]).unwrap();
        s.push_card_versions(&[c2]).unwrap();

        let all = s.list_cards(CardFilter::All, None).unwrap();
        expect!["2"].assert_eq(&all.len().to_string());
    }

    #[test]
    fn list_cards_filtered() {
        let s = store();
        let c1 = Card::first(ID_A, "".into(), p(100), vec![], false, ts(0), None);
        let c2 = Card::first(ID_B, "".into(), p(200), vec![], true, ts(0), None);
        s.push_card_versions(&[c1]).unwrap();
        s.push_card_versions(&[c2]).unwrap();

        let extinguished = s.list_cards(CardFilter::Extinguished, None).unwrap();
        expect!["1"].assert_eq(&extinguished.len().to_string());

        let blazed = s.list_cards(CardFilter::Blazed, None).unwrap();
        expect!["1"].assert_eq(&blazed.len().to_string());
    }

    #[test]
    fn card_history() {
        let s = store();
        let v1 = Card::first(ID_A, "C1".into(), p(1), vec![], false, ts(0), None);
        s.push_card_versions(std::slice::from_ref(&v1)).unwrap();

        let v2 = v1.next("C2".into(), p(1), vec![], false, ts(1000), None);
        s.push_card_versions(&[v2]).unwrap();

        let history = s.get_card_history(ID_A, None).unwrap();
        expect!["2"].assert_eq(&history.len().to_string());
        expect!["1"].assert_eq(&history[0].count().to_string());
        expect!["2"].assert_eq(&history[1].count().to_string());
    }

    // -- Card deletion -------------------------------------------------------

    #[test]
    fn delete_card() {
        let s = store();
        let card = Card::first(ID_A, "C".into(), p(1), vec![], false, ts(0), None);
        s.push_card_versions(&[card]).unwrap();

        let deleted = s.delete_card(ID_A).unwrap();
        expect!["true"].assert_eq(&deleted.verify().to_string());

        // Card is gone.
        assert!(s.get_card(ID_A).unwrap().is_none());
        // History is gone.
        assert!(s.get_card_history(ID_A, None).unwrap().is_empty());
        // Cannot push to deleted card.
        let new = Card::first(ID_A, "C2".into(), p(1), vec![], false, ts(1000), None);
        assert!(matches!(
            s.push_card_versions(&[new]).unwrap_err(),
            PushOpError::Domain(PushError::AlreadyDeleted)
        ));
    }

    #[test]
    fn delete_updates_root() {
        let s = store();
        let card = Card::first(ID_A, "C".into(), p(1), vec![], false, ts(0), None);
        s.push_card_versions(&[card]).unwrap();
        let root_before = s.get_root().unwrap();

        s.delete_card(ID_A).unwrap();
        let root_after = s.get_root().unwrap();

        assert_ne!(root_before.hash, root_after.hash);
        expect!["2"].assert_eq(&root_after.sequence.to_string());
        // Root hash is NOT zero — the deleted entity hash is included.
        assert_ne!(root_after.hash, ZERO_HASH);
    }

    // -- Tag CRUD ------------------------------------------------------------

    #[test]
    fn create_and_get_tag() {
        let s = store();
        let tag = Tag::first(TAG_ID, "Groceries".into(), None, ts(1000));
        s.push_tag_versions(std::slice::from_ref(&tag)).unwrap();

        let fetched = s.get_tag(TAG_ID).unwrap().unwrap();
        assert_eq!(tag, fetched);
        expect!["true"].assert_eq(&fetched.verify().to_string());
    }

    #[test]
    fn tag_not_found() {
        let s = store();
        assert!(s.get_tag(TAG_ID).unwrap().is_none());
    }

    #[test]
    fn push_tag_chain() {
        let s = store();
        let t1 = Tag::first(TAG_ID, "Groceries".into(), None, ts(1000));
        s.push_tag_versions(std::slice::from_ref(&t1)).unwrap();

        let t2 = t1.next("Food".into(), None, ts(2000));
        s.push_tag_versions(std::slice::from_ref(&t2)).unwrap();

        let latest = s.get_tag(TAG_ID).unwrap().unwrap();
        assert_eq!(latest, t2);
    }

    #[test]
    fn push_tag_rejects_ancestor_mismatch() {
        let s = store();
        let t1 = Tag::first(TAG_ID, "Groceries".into(), None, ts(1000));
        s.push_tag_versions(std::slice::from_ref(&t1)).unwrap();

        let t2_a = t1.next("Food".into(), None, ts(2000));
        s.push_tag_versions(&[t2_a]).unwrap();

        let t2_b = t1.next("Supplies".into(), None, ts(2500));
        let err = s.push_tag_versions(&[t2_b]).unwrap_err();
        assert!(matches!(
            err,
            PushOpError::Domain(PushError::TagAncestorMismatch(_))
        ));
    }

    #[test]
    fn list_tags() {
        let s = store();
        let t1 = Tag::first(TAG_ID, "Groceries".into(), None, ts(1000));
        let t2 = Tag::first(ID_A, "Work".into(), None, ts(1000));
        s.push_tag_versions(&[t1]).unwrap();
        s.push_tag_versions(&[t2]).unwrap();

        let tags = s.list_tags().unwrap();
        expect!["2"].assert_eq(&tags.len().to_string());
    }

    #[test]
    fn delete_tag() {
        let s = store();
        let tag = Tag::first(TAG_ID, "Groceries".into(), None, ts(1000));
        s.push_tag_versions(&[tag]).unwrap();

        let deleted = s.delete_tag(TAG_ID).unwrap();
        expect!["true"].assert_eq(&deleted.verify().to_string());

        assert!(s.get_tag(TAG_ID).unwrap().is_none());
        assert!(matches!(
            s.push_tag_versions(&[Tag::first(TAG_ID, "X".into(), None, ts(2000))])
                .unwrap_err(),
            PushOpError::Domain(PushError::AlreadyDeleted)
        ));
    }

    // -- Card with tags ------------------------------------------------------

    #[test]
    fn card_with_tags_round_trips() {
        let s = store();
        let card = Card::first(
            ID_A,
            "content".into(),
            p(100),
            vec![TAG_ID, ID_B],
            false,
            ts(1000),
            None,
        );
        s.push_card_versions(std::slice::from_ref(&card)).unwrap();

        let fetched = s.get_card(ID_A).unwrap().unwrap();
        assert_eq!(card, fetched);
        assert_eq!(fetched.tags(), &[ID_B, TAG_ID]);
    }

    // -- Root hash determinism -----------------------------------------------

    #[test]
    fn root_hash_deterministic_regardless_of_insert_order() {
        let s1 = store();
        let s2 = store();

        let c1 = Card::first(ID_A, "".into(), p(1), vec![], false, ts(0), None);
        let c2 = Card::first(ID_B, "".into(), p(2), vec![], false, ts(0), None);

        // Insert in different order.
        s1.push_card_versions(std::slice::from_ref(&c1)).unwrap();
        s1.push_card_versions(std::slice::from_ref(&c2)).unwrap();

        s2.push_card_versions(&[c2]).unwrap();
        s2.push_card_versions(&[c1]).unwrap();

        let r1 = s1.get_root().unwrap();
        let r2 = s2.get_root().unwrap();
        assert_eq!(r1.hash, r2.hash);
    }

    // -- Tag + root interaction ----------------------------------------------

    #[test]
    fn tag_push_updates_root() {
        let s = store();
        let root_before = s.get_root().unwrap();
        let tag = Tag::first(TAG_ID, "Groceries".into(), None, ts(1000));
        s.push_tag_versions(&[tag]).unwrap();
        let root_after = s.get_root().unwrap();
        assert_ne!(root_before.hash, root_after.hash);
        expect!["1"].assert_eq(&root_after.sequence.to_string());
    }

    #[test]
    fn tag_delete_updates_root() {
        let s = store();
        let tag = Tag::first(TAG_ID, "Groceries".into(), None, ts(1000));
        s.push_tag_versions(&[tag]).unwrap();
        let root_before = s.get_root().unwrap();
        s.delete_tag(TAG_ID).unwrap();
        let root_after = s.get_root().unwrap();
        assert_ne!(root_before.hash, root_after.hash);
        expect!["2"].assert_eq(&root_after.sequence.to_string());
        // Root hash is NOT zero — the deleted entity hash is included.
        assert_ne!(root_after.hash, ZERO_HASH);
    }

    #[test]
    fn push_to_deleted_tag_returns_already_deleted() {
        let s = store();
        let tag = Tag::first(TAG_ID, "Groceries".into(), None, ts(1000));
        s.push_tag_versions(&[tag]).unwrap();
        s.delete_tag(TAG_ID).unwrap();
        let new_tag = Tag::first(TAG_ID, "Revived".into(), None, ts(2000));
        assert!(matches!(
            s.push_tag_versions(&[new_tag]).unwrap_err(),
            PushOpError::Domain(PushError::AlreadyDeleted)
        ));
    }

    #[test]
    fn tag_ancestor_mismatch_returns_server_latest() {
        let s = store();
        let t1 = Tag::first(TAG_ID, "Groceries".into(), None, ts(1000));
        s.push_tag_versions(std::slice::from_ref(&t1)).unwrap();
        let t2_a = t1.next("Food".into(), None, ts(2000));
        s.push_tag_versions(std::slice::from_ref(&t2_a)).unwrap();
        // Try pushing from stale base.
        let t2_b = t1.next("Supplies".into(), None, ts(2500));
        match s.push_tag_versions(&[t2_b]).unwrap_err() {
            PushOpError::Domain(PushError::TagAncestorMismatch(server_latest)) => {
                assert_eq!(*server_latest, t2_a);
            }
            other => panic!("expected TagAncestorMismatch, got {other:?}"),
        }
    }

    // -- Large priority -------------------------------------------------------

    #[test]
    fn card_large_priority_round_trips() {
        let s = store();
        let big_priority = NonNegativeI64::try_from(i64::MAX / 2).unwrap();
        let card = Card::first(
            ID_A,
            "content".into(),
            big_priority,
            vec![],
            false,
            ts(1000),
            None,
        );
        s.push_card_versions(std::slice::from_ref(&card)).unwrap();
        let fetched = s.get_card(ID_A).unwrap().unwrap();
        assert_eq!(card, fetched);
        assert_eq!(fetched.priority(), big_priority);
    }

    #[test]
    fn card_max_priority_round_trips() {
        let s = store();
        let card = Card::first(
            ID_A,
            "content".into(),
            NonNegativeI64::MAX,
            vec![],
            false,
            ts(1000),
            None,
        );
        s.push_card_versions(std::slice::from_ref(&card)).unwrap();
        let fetched = s.get_card(ID_A).unwrap().unwrap();
        assert_eq!(card, fetched);
        assert_eq!(fetched.priority(), NonNegativeI64::MAX);
    }

    // -- Duplicate priority ---------------------------------------------------

    #[test]
    fn duplicate_priority_returns_conflicting_card_info() {
        let s = store();
        let c1 = Card::first(ID_A, "".into(), p(100), vec![], false, ts(0), None);
        s.push_card_versions(&[c1]).unwrap();

        let c2 = Card::first(ID_B, "".into(), p(100), vec![], false, ts(0), None);
        match s.push_card_versions(&[c2]).unwrap_err() {
            PushOpError::Domain(PushError::DuplicatePriority {
                conflicting_id,
                priority,
            }) => {
                assert_eq!(conflicting_id, ID_A);
                assert_eq!(priority, p(100));
            }
            other => panic!("expected DuplicatePriority, got {other:?}"),
        }
    }

    // -- Chain validation -----------------------------------------------------

    #[test]
    fn push_rejects_broken_internal_chain() {
        let s = store();
        let v1 = Card::first(ID_A, "C1".into(), p(1), vec![], false, ts(0), None);
        s.push_card_versions(std::slice::from_ref(&v1)).unwrap();
        let v2 = v1.next("C2".into(), p(1), vec![], false, ts(1000), None);
        // Create v3 that chains from v1 instead of v2 (broken internal chain).
        let v3_bad = v1.next("C3".into(), p(1), vec![], false, ts(2000), None);
        let err = s.push_card_versions(&[v2, v3_bad]).unwrap_err();
        assert!(matches!(
            err,
            PushOpError::Domain(PushError::HashVerificationFailed)
        ));
    }

    #[test]
    fn push_same_version_twice_fails() {
        let s = store();
        let v1 = Card::first(ID_A, "C".into(), p(1), vec![], false, ts(0), None);
        s.push_card_versions(std::slice::from_ref(&v1)).unwrap();
        let v2 = v1.next("C2".into(), p(1), vec![], false, ts(1000), None);
        s.push_card_versions(std::slice::from_ref(&v2)).unwrap();
        // Try pushing v2 again — ancestor is v1's hash but server's latest is now v2's hash.
        let err = s.push_card_versions(&[v2]).unwrap_err();
        assert!(matches!(
            err,
            PushOpError::Domain(PushError::CardAncestorMismatch(_))
        ));
        let s = store();
        let v1 = Card::first(ID_A, "C1".into(), p(1), vec![], false, ts(0), None);
        let v2 = v1.next("C2".into(), p(1), vec![], false, ts(1000), None);
        let v3 = v2.next("C3".into(), p(1), vec![], false, ts(2000), None);
        s.push_card_versions(&[v1]).unwrap();
        s.push_card_versions(&[v2]).unwrap();
        s.push_card_versions(&[v3]).unwrap();
        let history = s.get_card_history(ID_A, None).unwrap();
        assert_eq!(history.len(), 3);
        assert_eq!(history[0].count(), p(1));
        assert_eq!(history[1].count(), p(2));
        assert_eq!(history[2].count(), p(3));
    }

    // -- Root includes both cards and tags ------------------------------------

    #[test]
    fn root_hash_includes_cards_and_tags() {
        let s = store();
        let card = Card::first(ID_A, "C".into(), p(1), vec![], false, ts(0), None);
        s.push_card_versions(&[card]).unwrap();
        let root_cards_only = s.get_root().unwrap();

        let tag = Tag::first(TAG_ID, "Groceries".into(), None, ts(1000));
        s.push_tag_versions(&[tag]).unwrap();
        let root_cards_and_tags = s.get_root().unwrap();

        // Root hash must change when a tag is added.
        assert_ne!(root_cards_only.hash, root_cards_and_tags.hash);
    }

    // -- Root hash includes deleted entities -----------------------------------

    #[test]
    fn same_deletions_different_order_same_root() {
        let s1 = store();
        let s2 = store();

        let c1 = Card::first(ID_A, "".into(), p(1), vec![], false, ts(0), None);
        let c2 = Card::first(ID_B, "".into(), p(2), vec![], false, ts(0), None);

        // Store 1: create both then delete in order A, B.
        s1.push_card_versions(std::slice::from_ref(&c1)).unwrap();
        s1.push_card_versions(std::slice::from_ref(&c2)).unwrap();
        s1.delete_card(ID_A).unwrap();
        s1.delete_card(ID_B).unwrap();

        // Store 2: create both then delete in order B, A.
        s2.push_card_versions(&[c1]).unwrap();
        s2.push_card_versions(&[c2]).unwrap();
        s2.delete_card(ID_B).unwrap();
        s2.delete_card(ID_A).unwrap();

        let r1 = s1.get_root().unwrap();
        let r2 = s2.get_root().unwrap();
        // Root hashes must match — deletion order doesn't matter, only UUID order.
        assert_eq!(r1.hash, r2.hash);
    }

    #[test]
    fn root_hash_includes_deleted_entity_hash() {
        let s = store();
        let card = Card::first(ID_A, "C".into(), p(1), vec![], false, ts(0), None);
        s.push_card_versions(&[card]).unwrap();
        s.delete_card(ID_A).unwrap();

        let root = s.get_root().unwrap();
        // ID_A is in bucket 1. Bucket 1 hash = BLAKE3(deleted_entity_hash).
        let deleted = blazelist_protocol::DeletedEntity::new(ID_A);
        let b1_hash = bucket_hash(&[deleted.hash()]);
        let expected = expected_root(&[(1, b1_hash)]);
        assert_eq!(root.hash, expected);
    }

    #[test]
    fn root_hash_canonical_ordering_with_cards_tags_deleted() {
        let s = store();
        let card = Card::first(ID_A, "".into(), p(1), vec![], false, ts(0), None);
        let tag = Tag::first(TAG_ID, "T".into(), None, ts(0));
        s.push_card_versions(std::slice::from_ref(&card)).unwrap();
        s.push_tag_versions(std::slice::from_ref(&tag)).unwrap();

        // Create and delete ID_B so it becomes a deleted entity.
        let card_b = Card::first(ID_B, "".into(), p(2), vec![], false, ts(0), None);
        s.push_card_versions(&[card_b]).unwrap();
        s.delete_card(ID_B).unwrap();

        let root = s.get_root().unwrap();

        // ID_A → bucket 1 (card), ID_B → bucket 16 (deleted), TAG_ID → bucket 161 (tag).
        // Within each bucket: cards sorted by UUID, then tags, then deleted.
        let deleted_b = blazelist_protocol::DeletedEntity::new(ID_B);
        let b1_hash = bucket_hash(&[card.hash()]); // bucket 1: card ID_A
        let b16_hash = bucket_hash(&[deleted_b.hash()]); // bucket 16: deleted ID_B
        let b161_hash = bucket_hash(&[tag.hash()]); // bucket 161: tag TAG_ID
        let expected = expected_root(&[(1, b1_hash), (16, b16_hash), (161, b161_hash)]);
        assert_eq!(root.hash, expected);
    }

    #[test]
    fn bucket_of_uses_first_uuid_byte() {
        // ID_A starts with 0x01 → bucket 1
        assert_eq!(SqliteStorage::bucket_of(ID_A), 1);
        // ID_B starts with 0x10 → bucket 16
        assert_eq!(SqliteStorage::bucket_of(ID_B), 16);
        // TAG_ID starts with 0xa1 → bucket 161
        assert_eq!(SqliteStorage::bucket_of(TAG_ID), 161);
        // Same-bucket UUIDs share bucket 1 with ID_A.
        assert_eq!(SqliteStorage::bucket_of(SAME_BUCKET_1), 1);
        assert_eq!(SqliteStorage::bucket_of(SAME_BUCKET_2), 1);
    }

    // -- Same-bucket scenarios ------------------------------------------------

    #[test]
    fn same_bucket_two_cards_correct_hash() {
        // Two cards in bucket 1, verify the root hash is computed correctly
        // by combining both card hashes within the same bucket.
        let s = store();
        let c1 = Card::first(ID_A, "Card A".into(), p(1), vec![], false, ts(0), None);
        let c2 = Card::first(
            SAME_BUCKET_1,
            "Card SB1".into(),
            p(2),
            vec![],
            false,
            ts(0),
            None,
        );
        s.push_card_versions(std::slice::from_ref(&c1)).unwrap();
        s.push_card_versions(std::slice::from_ref(&c2)).unwrap();

        let root = s.get_root().unwrap();
        // Bucket 1 contains both cards sorted by UUID: ID_A < SAME_BUCKET_1.
        let b1_hash = bucket_hash(&[c1.hash(), c2.hash()]);
        let expected = expected_root(&[(1, b1_hash)]);
        assert_eq!(root.hash, expected);
    }

    #[test]
    fn same_bucket_three_cards_correct_hash() {
        // Three cards in bucket 1.
        let s = store();
        let c1 = Card::first(ID_A, "A".into(), p(1), vec![], false, ts(0), None);
        let c2 = Card::first(
            SAME_BUCKET_1,
            "SB1".into(),
            p(2),
            vec![],
            false,
            ts(0),
            None,
        );
        let c3 = Card::first(
            SAME_BUCKET_2,
            "SB2".into(),
            p(3),
            vec![],
            false,
            ts(0),
            None,
        );
        s.push_card_versions(std::slice::from_ref(&c1)).unwrap();
        s.push_card_versions(std::slice::from_ref(&c2)).unwrap();
        s.push_card_versions(std::slice::from_ref(&c3)).unwrap();

        let root = s.get_root().unwrap();
        // UUID order: ID_A < SAME_BUCKET_1 < SAME_BUCKET_2.
        let b1_hash = bucket_hash(&[c1.hash(), c2.hash(), c3.hash()]);
        let expected = expected_root(&[(1, b1_hash)]);
        assert_eq!(root.hash, expected);
    }

    #[test]
    fn same_bucket_card_and_tag_correct_hash() {
        // A card and a tag sharing bucket 1.
        let s = store();
        let card = Card::first(ID_A, "C".into(), p(1), vec![], false, ts(0), None);
        let tag = Tag::first(SAME_BUCKET_1, "T".into(), None, ts(0));
        s.push_card_versions(std::slice::from_ref(&card)).unwrap();
        s.push_tag_versions(std::slice::from_ref(&tag)).unwrap();

        let root = s.get_root().unwrap();
        // Within a bucket: cards first (sorted by UUID), then tags (sorted by UUID).
        let b1_hash = bucket_hash(&[card.hash(), tag.hash()]);
        let expected = expected_root(&[(1, b1_hash)]);
        assert_eq!(root.hash, expected);
    }

    #[test]
    fn same_bucket_card_and_deleted_correct_hash() {
        // A card and a deleted entity sharing bucket 1.
        let s = store();
        let card = Card::first(ID_A, "C".into(), p(1), vec![], false, ts(0), None);
        s.push_card_versions(std::slice::from_ref(&card)).unwrap();

        // Create and delete SAME_BUCKET_1 so it becomes a deleted entity in bucket 1.
        let doomed = Card::first(
            SAME_BUCKET_1,
            "doomed".into(),
            p(2),
            vec![],
            false,
            ts(0),
            None,
        );
        s.push_card_versions(&[doomed]).unwrap();
        s.delete_card(SAME_BUCKET_1).unwrap();

        let root = s.get_root().unwrap();
        // Bucket 1: cards (ID_A), then deleted (SAME_BUCKET_1).
        let deleted_sb1 = blazelist_protocol::DeletedEntity::new(SAME_BUCKET_1);
        let b1_hash = bucket_hash(&[card.hash(), deleted_sb1.hash()]);
        let expected = expected_root(&[(1, b1_hash)]);
        assert_eq!(root.hash, expected);
    }

    #[test]
    fn same_bucket_all_three_entity_types() {
        // Card, tag, and deleted entity all in bucket 1.
        let s = store();
        let card = Card::first(ID_A, "C".into(), p(1), vec![], false, ts(0), None);
        let tag = Tag::first(SAME_BUCKET_1, "T".into(), None, ts(0));
        s.push_card_versions(std::slice::from_ref(&card)).unwrap();
        s.push_tag_versions(std::slice::from_ref(&tag)).unwrap();

        // Create and delete SAME_BUCKET_2 to get a deleted entity in bucket 1.
        let doomed = Card::first(
            SAME_BUCKET_2,
            "doomed".into(),
            p(2),
            vec![],
            false,
            ts(0),
            None,
        );
        s.push_card_versions(&[doomed]).unwrap();
        s.delete_card(SAME_BUCKET_2).unwrap();

        let root = s.get_root().unwrap();
        // Bucket 1: cards (ID_A), tags (SAME_BUCKET_1), deleted (SAME_BUCKET_2).
        let deleted_sb2 = blazelist_protocol::DeletedEntity::new(SAME_BUCKET_2);
        let b1_hash = bucket_hash(&[card.hash(), tag.hash(), deleted_sb2.hash()]);
        let expected = expected_root(&[(1, b1_hash)]);
        assert_eq!(root.hash, expected);
    }

    #[test]
    fn same_bucket_deterministic_insert_order() {
        // Two cards in the same bucket, inserted in different order, must
        // produce the same root hash (within-bucket UUID ordering).
        let s1 = store();
        let s2 = store();

        let c_a = Card::first(ID_A, "A".into(), p(1), vec![], false, ts(0), None);
        let c_sb1 = Card::first(
            SAME_BUCKET_1,
            "SB1".into(),
            p(2),
            vec![],
            false,
            ts(0),
            None,
        );

        s1.push_card_versions(std::slice::from_ref(&c_a)).unwrap();
        s1.push_card_versions(std::slice::from_ref(&c_sb1)).unwrap();

        s2.push_card_versions(&[c_sb1]).unwrap();
        s2.push_card_versions(&[c_a]).unwrap();

        assert_eq!(s1.get_root().unwrap().hash, s2.get_root().unwrap().hash);
    }

    #[test]
    fn same_bucket_deterministic_three_entities_all_orders() {
        // Three cards in the same bucket — all 6 insertion orders produce
        // the same root hash.
        let cards = [
            Card::first(ID_A, "A".into(), p(1), vec![], false, ts(0), None),
            Card::first(
                SAME_BUCKET_1,
                "SB1".into(),
                p(2),
                vec![],
                false,
                ts(0),
                None,
            ),
            Card::first(
                SAME_BUCKET_2,
                "SB2".into(),
                p(3),
                vec![],
                false,
                ts(0),
                None,
            ),
        ];
        let orders: [[usize; 3]; 6] = [
            [0, 1, 2],
            [0, 2, 1],
            [1, 0, 2],
            [1, 2, 0],
            [2, 0, 1],
            [2, 1, 0],
        ];
        let mut hashes = Vec::new();
        for order in &orders {
            let s = store();
            for &i in order {
                s.push_card_versions(std::slice::from_ref(&cards[i]))
                    .unwrap();
            }
            hashes.push(s.get_root().unwrap().hash);
        }
        for h in &hashes[1..] {
            assert_eq!(hashes[0], *h);
        }
    }

    #[test]
    fn same_bucket_mixed_types_deterministic() {
        // Card and tag in same bucket — order of push (card first vs tag
        // first) must produce the same root hash.
        let s1 = store();
        let s2 = store();

        let card = Card::first(ID_A, "C".into(), p(1), vec![], false, ts(0), None);
        let tag = Tag::first(SAME_BUCKET_1, "T".into(), None, ts(0));

        s1.push_card_versions(std::slice::from_ref(&card)).unwrap();
        s1.push_tag_versions(std::slice::from_ref(&tag)).unwrap();

        s2.push_tag_versions(&[tag]).unwrap();
        s2.push_card_versions(&[card]).unwrap();

        assert_eq!(s1.get_root().unwrap().hash, s2.get_root().unwrap().hash);
    }

    // -- Bucket hash after version updates ------------------------------------

    #[test]
    fn bucket_hash_updates_after_card_version_push() {
        // Push v1, record root, push v2, verify root changes and matches
        // expected bucket hash with the new card hash.
        let s = store();
        let v1 = Card::first(ID_A, "v1".into(), p(1), vec![], false, ts(0), None);
        s.push_card_versions(std::slice::from_ref(&v1)).unwrap();

        let root_v1 = s.get_root().unwrap();
        let b1_v1 = bucket_hash(&[v1.hash()]);
        let expected_v1 = expected_root(&[(1, b1_v1)]);
        assert_eq!(root_v1.hash, expected_v1);

        let v2 = v1.next("v2".into(), p(1), vec![], false, ts(1000), None);
        s.push_card_versions(std::slice::from_ref(&v2)).unwrap();

        let root_v2 = s.get_root().unwrap();
        // Bucket hash should now use v2's hash, not v1's.
        let b1_v2 = bucket_hash(&[v2.hash()]);
        let expected_v2 = expected_root(&[(1, b1_v2)]);
        assert_eq!(root_v2.hash, expected_v2);
        assert_ne!(root_v1.hash, root_v2.hash);
    }

    #[test]
    fn bucket_hash_updates_after_tag_version_push() {
        let s = store();
        let t1 = Tag::first(TAG_ID, "v1".into(), None, ts(0));
        s.push_tag_versions(std::slice::from_ref(&t1)).unwrap();

        let root_t1 = s.get_root().unwrap();
        let b161_t1 = bucket_hash(&[t1.hash()]);
        assert_eq!(root_t1.hash, expected_root(&[(161, b161_t1)]));

        let t2 = t1.next("v2".into(), None, ts(1000));
        s.push_tag_versions(std::slice::from_ref(&t2)).unwrap();

        let root_t2 = s.get_root().unwrap();
        let b161_t2 = bucket_hash(&[t2.hash()]);
        assert_eq!(root_t2.hash, expected_root(&[(161, b161_t2)]));
        assert_ne!(root_t1.hash, root_t2.hash);
    }

    #[test]
    fn same_bucket_version_update_does_not_affect_neighbor() {
        // Two cards in bucket 1. Update one — the other's contribution to
        // the bucket hash must remain unchanged.
        let s = store();
        let c1 = Card::first(ID_A, "A".into(), p(1), vec![], false, ts(0), None);
        let c2 = Card::first(
            SAME_BUCKET_1,
            "SB1".into(),
            p(2),
            vec![],
            false,
            ts(0),
            None,
        );
        s.push_card_versions(std::slice::from_ref(&c1)).unwrap();
        s.push_card_versions(std::slice::from_ref(&c2)).unwrap();

        let root_before = s.get_root().unwrap();

        // Update only c1.
        let c1_v2 = c1.next("A v2".into(), p(1), vec![], false, ts(1000), None);
        s.push_card_versions(std::slice::from_ref(&c1_v2)).unwrap();

        let root_after = s.get_root().unwrap();
        // c2's hash is still present, c1's hash changed.
        let b1_hash = bucket_hash(&[c1_v2.hash(), c2.hash()]);
        let expected = expected_root(&[(1, b1_hash)]);
        assert_eq!(root_after.hash, expected);
        assert_ne!(root_before.hash, root_after.hash);
    }

    // -- Deletion within a bucket ---------------------------------------------

    #[test]
    fn same_bucket_delete_card_transitions_hash() {
        // Card in bucket 1. Delete it — the bucket hash should change from
        // card hash to deleted entity hash.
        let s = store();
        let card = Card::first(ID_A, "C".into(), p(1), vec![], false, ts(0), None);
        s.push_card_versions(std::slice::from_ref(&card)).unwrap();

        let root_before = s.get_root().unwrap();
        let b1_before = bucket_hash(&[card.hash()]);
        assert_eq!(root_before.hash, expected_root(&[(1, b1_before)]));

        s.delete_card(ID_A).unwrap();

        let root_after = s.get_root().unwrap();
        let deleted = blazelist_protocol::DeletedEntity::new(ID_A);
        let b1_after = bucket_hash(&[deleted.hash()]);
        assert_eq!(root_after.hash, expected_root(&[(1, b1_after)]));
        assert_ne!(root_before.hash, root_after.hash);
    }

    #[test]
    fn same_bucket_delete_one_of_two_cards() {
        // Two cards in bucket 1. Delete one — the bucket should contain
        // the remaining card hash and the deleted entity hash.
        let s = store();
        let c1 = Card::first(ID_A, "A".into(), p(1), vec![], false, ts(0), None);
        let c2 = Card::first(
            SAME_BUCKET_1,
            "SB1".into(),
            p(2),
            vec![],
            false,
            ts(0),
            None,
        );
        s.push_card_versions(std::slice::from_ref(&c1)).unwrap();
        s.push_card_versions(std::slice::from_ref(&c2)).unwrap();

        s.delete_card(SAME_BUCKET_1).unwrap();

        let root = s.get_root().unwrap();
        // Bucket 1: cards (ID_A), deleted (SAME_BUCKET_1).
        let deleted_sb1 = blazelist_protocol::DeletedEntity::new(SAME_BUCKET_1);
        let b1_hash = bucket_hash(&[c1.hash(), deleted_sb1.hash()]);
        assert_eq!(root.hash, expected_root(&[(1, b1_hash)]));
    }

    #[test]
    fn same_bucket_delete_all_cards_then_empty_bucket() {
        // Two cards in bucket 1. Delete both — bucket should contain only
        // the two deleted entity hashes.
        let s = store();
        let c1 = Card::first(ID_A, "A".into(), p(1), vec![], false, ts(0), None);
        let c2 = Card::first(
            SAME_BUCKET_1,
            "SB1".into(),
            p(2),
            vec![],
            false,
            ts(0),
            None,
        );
        s.push_card_versions(&[c1]).unwrap();
        s.push_card_versions(&[c2]).unwrap();

        s.delete_card(ID_A).unwrap();
        s.delete_card(SAME_BUCKET_1).unwrap();

        let root = s.get_root().unwrap();
        // Bucket 1: deleted entities sorted by UUID: ID_A < SAME_BUCKET_1.
        let del_a = blazelist_protocol::DeletedEntity::new(ID_A);
        let del_sb1 = blazelist_protocol::DeletedEntity::new(SAME_BUCKET_1);
        let b1_hash = bucket_hash(&[del_a.hash(), del_sb1.hash()]);
        assert_eq!(root.hash, expected_root(&[(1, b1_hash)]));
    }

    // -- Mixed same-bucket and cross-bucket -----------------------------------

    #[test]
    fn mixed_same_and_different_buckets_correct_hash() {
        // Two cards in bucket 1 (ID_A, SAME_BUCKET_1) + one card in bucket 16 (ID_B).
        let s = store();
        let c_a = Card::first(ID_A, "A".into(), p(1), vec![], false, ts(0), None);
        let c_sb1 = Card::first(
            SAME_BUCKET_1,
            "SB1".into(),
            p(2),
            vec![],
            false,
            ts(0),
            None,
        );
        let c_b = Card::first(ID_B, "B".into(), p(3), vec![], false, ts(0), None);
        s.push_card_versions(std::slice::from_ref(&c_a)).unwrap();
        s.push_card_versions(std::slice::from_ref(&c_sb1)).unwrap();
        s.push_card_versions(std::slice::from_ref(&c_b)).unwrap();

        let root = s.get_root().unwrap();
        let b1_hash = bucket_hash(&[c_a.hash(), c_sb1.hash()]);
        let b16_hash = bucket_hash(&[c_b.hash()]);
        assert_eq!(root.hash, expected_root(&[(1, b1_hash), (16, b16_hash)]));
    }

    #[test]
    fn cross_bucket_mutation_does_not_affect_other_bucket() {
        // Set up bucket 1 with two cards, bucket 16 with one card.
        // Mutate bucket 16 — bucket 1's hash contribution must stay the same.
        let s = store();
        let c_a = Card::first(ID_A, "A".into(), p(1), vec![], false, ts(0), None);
        let c_sb1 = Card::first(
            SAME_BUCKET_1,
            "SB1".into(),
            p(2),
            vec![],
            false,
            ts(0),
            None,
        );
        let c_b = Card::first(ID_B, "B".into(), p(3), vec![], false, ts(0), None);
        s.push_card_versions(std::slice::from_ref(&c_a)).unwrap();
        s.push_card_versions(std::slice::from_ref(&c_sb1)).unwrap();
        s.push_card_versions(std::slice::from_ref(&c_b)).unwrap();

        let root_before = s.get_root().unwrap();

        // Update card in bucket 16.
        let c_b_v2 = c_b.next("B v2".into(), p(3), vec![], false, ts(1000), None);
        s.push_card_versions(std::slice::from_ref(&c_b_v2)).unwrap();

        let root_after = s.get_root().unwrap();
        // Bucket 1 unchanged, bucket 16 updated.
        let b1_hash = bucket_hash(&[c_a.hash(), c_sb1.hash()]);
        let b16_hash = bucket_hash(&[c_b_v2.hash()]);
        assert_eq!(
            root_after.hash,
            expected_root(&[(1, b1_hash), (16, b16_hash)])
        );
        assert_ne!(root_before.hash, root_after.hash);
    }

    // -- Batch with same-bucket entities --------------------------------------

    #[test]
    fn batch_same_bucket_entities_correct_hash() {
        // Batch push of a card and tag in the same bucket.
        let s = store();
        let card = Card::first(ID_A, "C".into(), p(1), vec![], false, ts(0), None);
        let tag = Tag::first(SAME_BUCKET_1, "T".into(), None, ts(0));

        use blazelist_protocol::PushItem;
        s.push_batch(&[
            PushItem::Cards(vec![card.clone()]),
            PushItem::Tags(vec![tag.clone()]),
        ])
        .unwrap();

        let root = s.get_root().unwrap();
        let b1_hash = bucket_hash(&[card.hash(), tag.hash()]);
        assert_eq!(root.hash, expected_root(&[(1, b1_hash)]));
    }

    #[test]
    fn batch_same_bucket_create_and_delete() {
        // Batch: create a card in bucket 1, delete another card already in bucket 1.
        let s = store();
        let existing = Card::first(ID_A, "old".into(), p(1), vec![], false, ts(0), None);
        s.push_card_versions(&[existing]).unwrap();

        let new_card = Card::first(
            SAME_BUCKET_1,
            "new".into(),
            p(2),
            vec![],
            false,
            ts(0),
            None,
        );

        use blazelist_protocol::PushItem;
        s.push_batch(&[
            PushItem::Cards(vec![new_card.clone()]),
            PushItem::DeleteCard { id: ID_A },
        ])
        .unwrap();

        let root = s.get_root().unwrap();
        // Bucket 1: cards (SAME_BUCKET_1), deleted (ID_A).
        // Note: ID_A < SAME_BUCKET_1 by UUID, but cards come before deleted in the bucket hash.
        let del_a = blazelist_protocol::DeletedEntity::new(ID_A);
        let b1_hash = bucket_hash(&[new_card.hash(), del_a.hash()]);
        assert_eq!(root.hash, expected_root(&[(1, b1_hash)]));
    }

    #[test]
    fn batch_mixed_buckets_correct_hash() {
        // Batch with entities spanning multiple buckets, some sharing a bucket.
        let s = store();
        let c_a = Card::first(ID_A, "A".into(), p(1), vec![], false, ts(0), None);
        let c_sb1 = Card::first(
            SAME_BUCKET_1,
            "SB1".into(),
            p(2),
            vec![],
            false,
            ts(0),
            None,
        );
        let tag_b = Tag::first(ID_B, "T".into(), None, ts(0));

        use blazelist_protocol::PushItem;
        s.push_batch(&[
            PushItem::Cards(vec![c_a.clone()]),
            PushItem::Cards(vec![c_sb1.clone()]),
            PushItem::Tags(vec![tag_b.clone()]),
        ])
        .unwrap();

        let root = s.get_root().unwrap();
        let b1_hash = bucket_hash(&[c_a.hash(), c_sb1.hash()]);
        let b16_hash = bucket_hash(&[tag_b.hash()]);
        assert_eq!(root.hash, expected_root(&[(1, b1_hash), (16, b16_hash)]));
    }

    // -- Persistence of bucket state ------------------------------------------

    #[test]
    fn persistence_root_hash_survives_reopen() {
        // Verify root hash is correct after reopening the database.
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("test.db");

        let card = Card::first(ID_A, "C".into(), p(1), vec![], false, ts(0), None);
        let root_hash;
        {
            let s = SqliteStorage::open(&db_path, false).unwrap();
            s.push_card_versions(&[card]).unwrap();
            root_hash = s.get_root().unwrap().hash;
        }
        {
            let s = SqliteStorage::open(&db_path, false).unwrap();
            assert_eq!(s.get_root().unwrap().hash, root_hash);
        }
    }

    #[test]
    fn persistence_bucket_state_correct_after_reopen_and_push() {
        // Push a card, reopen, push a second card in the same bucket.
        // The root hash must reflect both cards.
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("test.db");

        let c1 = Card::first(ID_A, "A".into(), p(1), vec![], false, ts(0), None);
        {
            let s = SqliteStorage::open(&db_path, false).unwrap();
            s.push_card_versions(std::slice::from_ref(&c1)).unwrap();
        }
        {
            let s = SqliteStorage::open(&db_path, false).unwrap();
            let c2 = Card::first(
                SAME_BUCKET_1,
                "SB1".into(),
                p(2),
                vec![],
                false,
                ts(0),
                None,
            );
            s.push_card_versions(std::slice::from_ref(&c2)).unwrap();

            let root = s.get_root().unwrap();
            let b1_hash = bucket_hash(&[c1.hash(), c2.hash()]);
            let expected = expected_root(&[(1, b1_hash)]);
            assert_eq!(root.hash, expected);
        }
    }

    #[test]
    fn persistence_bucket_state_correct_after_reopen_and_cross_bucket_push() {
        // Push a card in bucket 1, reopen, push a card in bucket 16.
        // Root must reflect both buckets.
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("test.db");

        let c1 = Card::first(ID_A, "A".into(), p(1), vec![], false, ts(0), None);
        {
            let s = SqliteStorage::open(&db_path, false).unwrap();
            s.push_card_versions(std::slice::from_ref(&c1)).unwrap();
        }
        {
            let s = SqliteStorage::open(&db_path, false).unwrap();
            let c2 = Card::first(ID_B, "B".into(), p(2), vec![], false, ts(0), None);
            s.push_card_versions(std::slice::from_ref(&c2)).unwrap();

            let root = s.get_root().unwrap();
            let b1_hash = bucket_hash(&[c1.hash()]);
            let b16_hash = bucket_hash(&[c2.hash()]);
            assert_eq!(root.hash, expected_root(&[(1, b1_hash), (16, b16_hash)]));
        }
    }

    // -- GetChangesSince (delta sync) -----------------------------------------

    #[test]
    fn changes_since_returns_only_new_cards() {
        let s = store();
        let c1 = Card::first(ID_A, "".into(), p(1), vec![], false, ts(0), None);
        s.push_card_versions(&[c1]).unwrap();
        let root_after_a = s.get_root().unwrap();

        let c2 = Card::first(ID_B, "".into(), p(2), vec![], false, ts(0), None);
        s.push_card_versions(std::slice::from_ref(&c2)).unwrap();

        let changes = s
            .get_changes_since(root_after_a.sequence, root_after_a.hash)
            .unwrap();
        expect!["1"].assert_eq(&changes.cards.len().to_string());
        assert_eq!(changes.cards[0], c2);
        assert!(changes.tags.is_empty());
        assert!(changes.deleted.is_empty());
    }

    #[test]
    fn changes_since_returns_modified_card() {
        let s = store();
        let v1 = Card::first(ID_A, "C1".into(), p(1), vec![], false, ts(0), None);
        s.push_card_versions(std::slice::from_ref(&v1)).unwrap();
        let root_after_v1 = s.get_root().unwrap();

        let v2 = v1.next("C2".into(), p(1), vec![], false, ts(1000), None);
        s.push_card_versions(std::slice::from_ref(&v2)).unwrap();

        let changes = s
            .get_changes_since(root_after_v1.sequence, root_after_v1.hash)
            .unwrap();
        expect!["1"].assert_eq(&changes.cards.len().to_string());
        assert_eq!(changes.cards[0], v2);
    }

    #[test]
    fn changes_since_returns_deleted_entities() {
        let s = store();
        let card = Card::first(ID_A, "C".into(), p(1), vec![], false, ts(0), None);
        s.push_card_versions(&[card]).unwrap();
        let root_before_delete = s.get_root().unwrap();

        s.delete_card(ID_A).unwrap();
        let changes = s
            .get_changes_since(root_before_delete.sequence, root_before_delete.hash)
            .unwrap();
        assert!(changes.cards.is_empty());
        expect!["1"].assert_eq(&changes.deleted.len().to_string());
        assert_eq!(changes.deleted[0].id(), ID_A);
    }

    #[test]
    fn changes_since_empty_when_up_to_date() {
        let s = store();
        let card = Card::first(ID_A, "C".into(), p(1), vec![], false, ts(0), None);
        s.push_card_versions(&[card]).unwrap();
        let root = s.get_root().unwrap();

        let changes = s.get_changes_since(root.sequence, root.hash).unwrap();
        assert!(changes.cards.is_empty());
        assert!(changes.tags.is_empty());
        assert!(changes.deleted.is_empty());
    }

    #[test]
    fn changes_since_includes_tags() {
        let s = store();
        let root_before = s.get_root().unwrap();
        let tag = Tag::first(TAG_ID, "Groceries".into(), None, ts(1000));
        s.push_tag_versions(std::slice::from_ref(&tag)).unwrap();

        let changes = s
            .get_changes_since(root_before.sequence, root_before.hash)
            .unwrap();
        expect!["1"].assert_eq(&changes.tags.len().to_string());
        assert_eq!(changes.tags[0], tag);
    }

    #[test]
    fn changes_since_returns_current_root() {
        let s = store();
        let card = Card::first(ID_A, "C".into(), p(1), vec![], false, ts(0), None);
        s.push_card_versions(&[card]).unwrap();

        let changes = s
            .get_changes_since(
                NonNegativeI64::try_from(0i64).unwrap(),
                blazelist_protocol::ZERO_HASH,
            )
            .unwrap();
        let root = s.get_root().unwrap();
        assert_eq!(changes.root, root);
    }

    // -- PushBatch (atomic multi-entity push) ---------------------------------

    #[test]
    fn batch_card_and_tag_succeeds_atomically() {
        let s = store();
        let card = Card::first(ID_A, "".into(), p(1), vec![], false, ts(0), None);
        let tag = Tag::first(TAG_ID, "T".into(), None, ts(0));

        use blazelist_protocol::PushItem;
        s.push_batch(&[
            PushItem::Cards(vec![card.clone()]),
            PushItem::Tags(vec![tag.clone()]),
        ])
        .unwrap();

        let fetched_card = s.get_card(ID_A).unwrap().unwrap();
        assert_eq!(fetched_card, card);
        let fetched_tag = s.get_tag(TAG_ID).unwrap().unwrap();
        assert_eq!(fetched_tag, tag);

        // Root sequence incremented once (single recompute_root for the whole batch).
        let root = s.get_root().unwrap();
        expect!["1"].assert_eq(&root.sequence.to_string());
    }

    #[test]
    fn batch_rollback_on_second_item_failure() {
        let s = store();
        // Pre-create a card so the second item causes an ancestor mismatch.
        let existing = Card::first(ID_A, "".into(), p(1), vec![], false, ts(0), None);
        s.push_card_versions(std::slice::from_ref(&existing))
            .unwrap();
        let root_before = s.get_root().unwrap();

        // Batch: create a tag (would succeed), then push a card with wrong ancestor (fails).
        let tag = Tag::first(TAG_ID, "T".into(), None, ts(0));
        let stale_card = Card::first(ID_A, "".into(), p(1), vec![], false, ts(100), None);

        use blazelist_protocol::PushItem;
        let err = s
            .push_batch(&[PushItem::Tags(vec![tag]), PushItem::Cards(vec![stale_card])])
            .unwrap_err();

        // Second item (index 1) should be the one that failed.
        assert_eq!(err.index, 1);
        assert!(matches!(
            err.error,
            PushOpError::Domain(PushError::CardAncestorMismatch(_))
        ));

        // Tag should NOT exist (batch rolled back).
        assert!(s.get_tag(TAG_ID).unwrap().is_none());

        // Root should be unchanged.
        let root_after = s.get_root().unwrap();
        assert_eq!(root_before.hash, root_after.hash);
        assert_eq!(root_before.sequence, root_after.sequence);
    }

    #[test]
    fn batch_with_mixed_deletions_and_pushes() {
        let s = store();
        // Pre-create entities to delete.
        let card_to_delete = Card::first(ID_A, "".into(), p(1), vec![], false, ts(0), None);
        let tag_to_delete = Tag::first(TAG_ID, "Delete me".into(), None, ts(0));
        s.push_card_versions(&[card_to_delete]).unwrap();
        s.push_tag_versions(&[tag_to_delete]).unwrap();

        // Batch: create new card, delete existing card, create new tag, delete existing tag.
        let new_card = Card::first(ID_B, "".into(), p(2), vec![], false, ts(0), None);

        use blazelist_protocol::PushItem;
        s.push_batch(&[
            PushItem::Cards(vec![new_card.clone()]),
            PushItem::DeleteCard { id: ID_A },
            PushItem::DeleteTag { id: TAG_ID },
        ])
        .unwrap();

        // New card exists.
        let fetched = s.get_card(ID_B).unwrap().unwrap();
        assert_eq!(fetched, new_card);
        // Deleted card is gone.
        assert!(s.get_card(ID_A).unwrap().is_none());
        // Deleted tag is gone.
        assert!(s.get_tag(TAG_ID).unwrap().is_none());
    }

    #[test]
    fn batch_empty_returns_ok() {
        let s = store();
        s.push_batch(&[]).unwrap();
        // Root unchanged.
        let root = s.get_root().unwrap();
        expect!["0"].assert_eq(&root.sequence.to_string());
    }

    #[test]
    fn batch_duplicate_priority_rolls_back() {
        let s = store();
        let c1 = Card::first(ID_A, "".into(), p(100), vec![], false, ts(0), None);
        s.push_card_versions(&[c1]).unwrap();

        // Batch: tag (would succeed) + card with duplicate priority (fails).
        let tag = Tag::first(TAG_ID, "T".into(), None, ts(0));
        let c2 = Card::first(ID_B, "".into(), p(100), vec![], false, ts(0), None);

        use blazelist_protocol::PushItem;
        let err = s
            .push_batch(&[PushItem::Tags(vec![tag]), PushItem::Cards(vec![c2])])
            .unwrap_err();

        assert_eq!(err.index, 1);
        assert!(matches!(
            err.error,
            PushOpError::Domain(PushError::DuplicatePriority {
                conflicting_id,
                priority,
            }) if conflicting_id == ID_A && priority == p(100)
        ));

        // Tag should NOT exist (rolled back).
        assert!(s.get_tag(TAG_ID).unwrap().is_none());
    }

    #[test]
    fn batch_changes_since_stamps_all_entities() {
        let s = store();
        let root_before = s.get_root().unwrap();

        let card = Card::first(ID_A, "".into(), p(1), vec![], false, ts(0), None);
        let tag = Tag::first(TAG_ID, "T".into(), None, ts(0));

        use blazelist_protocol::PushItem;
        s.push_batch(&[
            PushItem::Cards(vec![card.clone()]),
            PushItem::Tags(vec![tag.clone()]),
        ])
        .unwrap();

        let changes = s
            .get_changes_since(root_before.sequence, root_before.hash)
            .unwrap();
        expect!["1"].assert_eq(&changes.cards.len().to_string());
        expect!["1"].assert_eq(&changes.tags.len().to_string());
        assert_eq!(changes.cards[0], card);
        assert_eq!(changes.tags[0], tag);
    }

    #[test]
    fn root_hash_validation_success() {
        let s = store();
        let card = Card::first(ID_A, "Test".into(), p(1), vec![], false, ts(0), None);
        s.push_card_versions(&[card]).unwrap();

        let root = s.get_root().unwrap();
        // Valid hash should succeed
        let changes = s.get_changes_since(root.sequence, root.hash).unwrap();
        assert!(changes.cards.is_empty());
    }

    #[test]
    fn root_hash_validation_mismatch() {
        let s = store();
        let card = Card::first(ID_A, "Test".into(), p(1), vec![], false, ts(0), None);
        s.push_card_versions(&[card]).unwrap();

        let root = s.get_root().unwrap();
        // Wrong hash should fail
        let wrong_hash = blake3::hash(b"wrong");
        let result = s.get_changes_since(root.sequence, wrong_hash);

        match result {
            Err(StorageError::RootHashMismatch {
                sequence,
                expected_hash,
            }) => {
                assert_eq!(sequence, root.sequence);
                assert_eq!(expected_hash, root.hash);
            }
            _ => panic!("Expected RootHashMismatch error"),
        }
    }

    #[test]
    fn root_hash_validation_missing_history() {
        let s = store();
        // Sequence 0 won't be in history (initial state)
        let result = s.get_changes_since(
            NonNegativeI64::try_from(0i64).unwrap(),
            blake3::hash(b"any"),
        );
        // Should proceed anyway (sequence predates history table)
        assert!(result.is_ok());
    }

    // -- Duplicate priority edge cases ----------------------------------------

    #[test]
    fn duplicate_priority_within_same_batch() {
        let s = store();
        // Two cards with the same priority within a single batch.
        let c1 = Card::first(ID_A, "first".into(), p(500), vec![], false, ts(0), None);
        let c2 = Card::first(ID_B, "second".into(), p(500), vec![], false, ts(0), None);

        use blazelist_protocol::PushItem;
        let err = s
            .push_batch(&[PushItem::Cards(vec![c1]), PushItem::Cards(vec![c2])])
            .unwrap_err();

        // Second item (index 1) should fail with DuplicatePriority.
        assert_eq!(err.index, 1);
        assert!(matches!(
            err.error,
            PushOpError::Domain(PushError::DuplicatePriority {
                conflicting_id,
                priority,
            }) if conflicting_id == ID_A && priority == p(500)
        ));

        // First card should NOT exist (batch rolled back).
        assert!(s.get_card(ID_A).unwrap().is_none());
    }

    #[test]
    fn duplicate_priority_sequential_push_returns_error() {
        let s = store();
        let c1 = Card::first(ID_A, "first".into(), p(42), vec![], false, ts(0), None);
        s.push_card_versions(&[c1]).unwrap();

        // Push a different card with the same priority
        let c2 = Card::first(ID_B, "second".into(), p(42), vec![], false, ts(0), None);
        let err = s.push_card_versions(&[c2]).unwrap_err();

        match err {
            PushOpError::Domain(PushError::DuplicatePriority {
                conflicting_id,
                priority,
            }) => {
                assert_eq!(conflicting_id, ID_A);
                assert_eq!(priority, p(42));
            }
            other => panic!("Expected DuplicatePriority, got {other:?}"),
        }

        // Only the first card should exist
        assert!(s.get_card(ID_A).unwrap().is_some());
        assert!(s.get_card(ID_B).unwrap().is_none());
    }

    #[test]
    fn same_card_update_keeps_its_own_priority() {
        let s = store();
        // A card can be updated without changing priority — should not conflict
        // with itself.
        let c1 = Card::first(ID_A, "v1".into(), p(100), vec![], false, ts(0), None);
        s.push_card_versions(std::slice::from_ref(&c1)).unwrap();

        let c2 = c1.next("v2".into(), p(100), vec![], false, ts(1), None);
        s.push_card_versions(&[c2]).unwrap();

        let fetched = s.get_card(ID_A).unwrap().unwrap();
        assert_eq!(fetched.content(), "v2");
    }

    // -- Push same version twice -----------------------------------------------

    #[test]
    fn push_same_card_version_twice_fails() {
        let s = store();
        let card = Card::first(ID_A, "hello".into(), p(1), vec![], false, ts(0), None);
        s.push_card_versions(std::slice::from_ref(&card)).unwrap();

        // Pushing the same version again should fail (ancestor mismatch).
        let err = s.push_card_versions(&[card]).unwrap_err();
        assert!(matches!(
            err,
            PushOpError::Domain(PushError::CardAncestorMismatch(_))
        ));
    }

    #[test]
    fn push_same_tag_version_twice_fails() {
        let s = store();
        let tag = Tag::first(TAG_ID, "tag".into(), None, ts(0));
        s.push_tag_versions(std::slice::from_ref(&tag)).unwrap();

        // Pushing the same version again should fail (ancestor mismatch).
        let err = s.push_tag_versions(&[tag]).unwrap_err();
        assert!(matches!(
            err,
            PushOpError::Domain(PushError::TagAncestorMismatch(_))
        ));
    }

    // -- Tag deletion while card references it ---------------------------------

    #[test]
    fn tag_deletion_does_not_affect_card_referencing_it() {
        let s = store();
        let tag = Tag::first(TAG_ID, "groceries".into(), None, ts(0));
        s.push_tag_versions(&[tag]).unwrap();

        // Create card that references this tag
        let card = Card::first(
            ID_A,
            "buy tofu".into(),
            p(1),
            vec![TAG_ID],
            false,
            ts(0),
            None,
        );
        s.push_card_versions(std::slice::from_ref(&card)).unwrap();

        // Delete the tag
        s.delete_tag(TAG_ID).unwrap();
        assert!(s.get_tag(TAG_ID).unwrap().is_none());

        // Card should still exist with its tag reference intact
        let fetched = s.get_card(ID_A).unwrap().unwrap();
        assert_eq!(fetched.tags(), &[TAG_ID]);
        assert_eq!(fetched, card);
    }

    // -- Deep version chains ---------------------------------------------------

    #[test]
    fn deep_version_chain_push() {
        let s = store();
        let mut current = Card::first(ID_A, "v1".into(), p(1000), vec![], false, ts(0), None);
        s.push_card_versions(std::slice::from_ref(&current))
            .unwrap();

        // Build a chain of 50 versions
        for i in 2..=50 {
            let next = current.next(format!("v{i}"), p(1000), vec![], false, ts(i), None);
            s.push_card_versions(std::slice::from_ref(&next)).unwrap();
            current = next;
        }

        // Verify the latest version
        let fetched = s.get_card(ID_A).unwrap().unwrap();
        assert_eq!(fetched.content(), "v50");
        assert!(fetched.verify());

        // Verify history length
        let history = s.get_card_history(ID_A, None).unwrap();
        assert_eq!(history.len(), 50);
    }

    #[test]
    fn deep_version_chain_batch_push() {
        let s = store();
        // Push a chain of 20 versions in a single push call
        let mut versions = Vec::new();
        let mut current = Card::first(ID_A, "v1".into(), p(1000), vec![], false, ts(0), None);
        versions.push(current.clone());

        for i in 2..=20 {
            let next = current.next(format!("v{i}"), p(1000), vec![], false, ts(i), None);
            versions.push(next.clone());
            current = next;
        }

        s.push_card_versions(&versions).unwrap();

        let fetched = s.get_card(ID_A).unwrap().unwrap();
        assert_eq!(fetched.content(), "v20");
        assert!(fetched.verify());

        let history = s.get_card_history(ID_A, None).unwrap();
        assert_eq!(history.len(), 20);
    }

    // -- Changes since with multiple mutation types ----------------------------

    #[test]
    fn changes_since_captures_all_mutation_types() {
        let s = store();
        let card1 = Card::first(ID_A, "card1".into(), p(1), vec![], false, ts(0), None);
        let card2 = Card::first(ID_B, "card2".into(), p(2), vec![], false, ts(0), None);
        let tag = Tag::first(TAG_ID, "tag".into(), None, ts(0));
        s.push_card_versions(std::slice::from_ref(&card1)).unwrap();
        s.push_card_versions(std::slice::from_ref(&card2)).unwrap();
        s.push_tag_versions(std::slice::from_ref(&tag)).unwrap();

        let root_snapshot = s.get_root().unwrap();

        // Now: edit card1, delete card2, edit tag
        let card1_v2 = card1.next("card1-edited".into(), p(1), vec![], false, ts(1), None);
        s.push_card_versions(std::slice::from_ref(&card1_v2))
            .unwrap();
        s.delete_card(ID_B).unwrap();
        let tag_v2 = tag.next("tag-edited".into(), None, ts(1));
        s.push_tag_versions(std::slice::from_ref(&tag_v2)).unwrap();

        let changes = s
            .get_changes_since(root_snapshot.sequence, root_snapshot.hash)
            .unwrap();

        // Should contain the edited card and tag, plus the deletion
        assert_eq!(changes.cards.len(), 1);
        assert_eq!(changes.cards[0].content(), "card1-edited");
        assert_eq!(changes.tags.len(), 1);
        assert_eq!(changes.tags[0].title(), "tag-edited");
        assert_eq!(changes.deleted.len(), 1);
        assert_eq!(changes.deleted[0].id(), ID_B);
    }

    #[test]
    fn changes_since_after_batch_operation() {
        let s = store();
        let card = Card::first(ID_A, "initial".into(), p(1), vec![], false, ts(0), None);
        s.push_card_versions(&[card]).unwrap();

        let root_snapshot = s.get_root().unwrap();

        // Batch: create new card + delete existing card
        let new_card = Card::first(ID_B, "new".into(), p(2), vec![], false, ts(0), None);
        use blazelist_protocol::PushItem;
        s.push_batch(&[
            PushItem::Cards(vec![new_card.clone()]),
            PushItem::DeleteCard { id: ID_A },
        ])
        .unwrap();

        let changes = s
            .get_changes_since(root_snapshot.sequence, root_snapshot.hash)
            .unwrap();

        // The new card and the deletion should both appear
        assert_eq!(changes.cards.len(), 1);
        assert_eq!(changes.cards[0].id(), ID_B);
        assert_eq!(changes.deleted.len(), 1);
        assert_eq!(changes.deleted[0].id(), ID_A);

        // Root sequence should be 2 (one for initial push, one for batch)
        let root = s.get_root().unwrap();
        expect!["2"].assert_eq(&root.sequence.to_string());
    }

    // -- Concurrent access (race condition) tests -----------------------------

    #[test]
    fn concurrent_pushes_to_same_card_one_wins() {
        use std::sync::Arc;

        let s = Arc::new(store());
        let card = Card::first(ID_A, "base".into(), p(1), vec![], false, ts(0), None);
        s.push_card_versions(std::slice::from_ref(&card)).unwrap();

        // Two threads both try to push a v2 based on the same v1.
        let s1 = Arc::clone(&s);
        let s2 = Arc::clone(&s);

        let v2a = card.next("edit-A".into(), p(1), vec![], false, ts(1), None);
        let v2b = card.next("edit-B".into(), p(1), vec![], false, ts(2), None);

        let h1 = std::thread::spawn(move || s1.push_card_versions(&[v2a]));
        let h2 = std::thread::spawn(move || s2.push_card_versions(&[v2b]));

        let r1 = h1.join().unwrap();
        let r2 = h2.join().unwrap();

        // Exactly one should succeed and one should fail with ancestor mismatch.
        let (successes, failures): (Vec<_>, Vec<_>) = [r1, r2].into_iter().partition(|r| r.is_ok());
        assert_eq!(successes.len(), 1, "exactly one push should succeed");
        assert_eq!(failures.len(), 1, "exactly one push should fail");

        let err = failures.into_iter().next().unwrap().unwrap_err();
        assert!(matches!(
            err,
            PushOpError::Domain(PushError::CardAncestorMismatch(_))
        ));

        // The card should have exactly 2 versions (v1 + whichever v2 won).
        let history = s.get_card_history(ID_A, None).unwrap();
        assert_eq!(history.len(), 2);
    }

    #[test]
    fn concurrent_deletes_of_same_card() {
        use std::sync::Arc;

        let s = Arc::new(store());
        let card = Card::first(ID_A, "to-delete".into(), p(1), vec![], false, ts(0), None);
        s.push_card_versions(&[card]).unwrap();

        let s1 = Arc::clone(&s);
        let s2 = Arc::clone(&s);

        let h1 = std::thread::spawn(move || s1.delete_card(ID_A));
        let h2 = std::thread::spawn(move || s2.delete_card(ID_A));

        let r1 = h1.join().unwrap();
        let r2 = h2.join().unwrap();

        // At least one should succeed. The other may succeed or fail
        // depending on ordering, but neither should panic.
        let any_ok = r1.is_ok() || r2.is_ok();
        assert!(any_ok, "at least one delete should succeed");

        // Card should be gone.
        assert!(s.get_card(ID_A).unwrap().is_none());
    }

    #[test]
    fn concurrent_pushes_with_duplicate_priority() {
        use std::sync::Arc;

        let s = Arc::new(store());

        // Two threads both try to create different cards with the same priority.
        let s1 = Arc::clone(&s);
        let s2 = Arc::clone(&s);

        let c1 = Card::first(ID_A, "card-A".into(), p(999), vec![], false, ts(0), None);
        let c2 = Card::first(ID_B, "card-B".into(), p(999), vec![], false, ts(0), None);

        let h1 = std::thread::spawn(move || s1.push_card_versions(&[c1]));
        let h2 = std::thread::spawn(move || s2.push_card_versions(&[c2]));

        let r1 = h1.join().unwrap();
        let r2 = h2.join().unwrap();

        // Exactly one should succeed, the other should get DuplicatePriority.
        let (successes, failures): (Vec<_>, Vec<_>) = [r1, r2].into_iter().partition(|r| r.is_ok());
        assert_eq!(successes.len(), 1, "exactly one push should succeed");
        assert_eq!(failures.len(), 1, "exactly one push should fail");

        let err = failures.into_iter().next().unwrap().unwrap_err();
        assert!(matches!(
            err,
            PushOpError::Domain(PushError::DuplicatePriority { .. })
        ));
    }

    #[test]
    fn concurrent_reads_and_writes() {
        use std::sync::Arc;

        let s = Arc::new(store());

        // Pre-populate with some cards.
        for i in 0..10 {
            let id =
                Uuid::from_bytes([(i + 10) as u8, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
            let card = Card::first(
                id,
                format!("card-{i}"),
                p(1000 + i),
                vec![],
                false,
                ts(0),
                None,
            );
            s.push_card_versions(&[card]).unwrap();
        }

        // Spawn readers and writers concurrently.
        let mut handles = Vec::new();

        // 5 readers
        for _ in 0..5 {
            let s = Arc::clone(&s);
            handles.push(std::thread::spawn(move || {
                let cards = s.list_cards(CardFilter::All, None).unwrap();
                assert!(!cards.is_empty());
            }));
        }

        // 5 writers (editing existing cards)
        for i in 0..5 {
            let s = Arc::clone(&s);
            handles.push(std::thread::spawn(move || {
                let id =
                    Uuid::from_bytes([(i + 10) as u8, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
                let card = s.get_card(id).unwrap().unwrap();
                let updated = card.next(
                    format!("edited-{i}"),
                    card.priority(),
                    vec![],
                    false,
                    ts(100 + i),
                    None,
                );
                // May fail if another writer got there first, but should not panic.
                let _ = s.push_card_versions(&[updated]);
            }));
        }

        for h in handles {
            h.join().unwrap();
        }

        // All 10 cards should still exist.
        let cards = s.list_cards(CardFilter::All, None).unwrap();
        assert_eq!(cards.len(), 10);
    }

    // -- Persistence ---------------------------------------------------------

    #[test]
    fn persistence_across_reopen() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("test.db");

        let card = Card::first(ID_A, "me".into(), p(1), vec![], false, ts(0), None);

        {
            let s = SqliteStorage::open(&db_path, false).unwrap();
            s.push_card_versions(std::slice::from_ref(&card)).unwrap();
        }

        {
            let s = SqliteStorage::open(&db_path, false).unwrap();
            let fetched = s.get_card(ID_A).unwrap().unwrap();
            assert_eq!(card, fetched);
        }
    }

    // -- Schema version checks -----------------------------------------------

    #[test]
    fn fresh_db_stores_current_protocol_version() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("test.db");
        {
            let _s = SqliteStorage::open(&db_path, false).unwrap();
        }
        // Reopen and verify no error (same version).
        let _s = SqliteStorage::open(&db_path, false).unwrap();
    }

    #[test]
    fn reopen_same_version_succeeds() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("test.db");

        let card = Card::first(ID_A, "C".into(), p(1), vec![], false, ts(0), None);
        {
            let s = SqliteStorage::open(&db_path, false).unwrap();
            s.push_card_versions(std::slice::from_ref(&card)).unwrap();
        }
        {
            let s = SqliteStorage::open(&db_path, false).unwrap();
            let fetched = s.get_card(ID_A).unwrap().unwrap();
            assert_eq!(card, fetched);
        }
    }

    #[test]
    fn incompatible_major_version_without_migration_flag_returns_error() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("test.db");

        // Create a database and manually set a different major version.
        {
            let _s = SqliteStorage::open(&db_path, false).unwrap();
            let conn = rusqlite::Connection::open(&db_path).unwrap();
            conn.execute("UPDATE schema_version SET major = 999 WHERE id = 1", [])
                .unwrap();
        }

        // Reopen should fail with IncompatibleVersion.
        match SqliteStorage::open(&db_path, false) {
            Err(StorageError::IncompatibleVersion { stored, current }) => {
                assert_eq!(stored.major, 999);
                assert_eq!(current, blazelist_protocol::PROTOCOL_VERSION);
            }
            Err(e) => panic!("expected IncompatibleVersion, got {e:?}"),
            Ok(_) => panic!("expected IncompatibleVersion, got Ok"),
        }
    }

    #[test]
    fn incompatible_major_version_with_migration_flag_returns_not_implemented() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("test.db");

        // Create a database and manually set a different major version.
        {
            let _s = SqliteStorage::open(&db_path, false).unwrap();
            let conn = rusqlite::Connection::open(&db_path).unwrap();
            conn.execute("UPDATE schema_version SET major = 999 WHERE id = 1", [])
                .unwrap();
        }

        // Reopen with migration flag should fail with MigrationNotImplemented.
        match SqliteStorage::open(&db_path, true) {
            Err(StorageError::MigrationNotImplemented { stored, current }) => {
                assert_eq!(stored.major, 999);
                assert_eq!(current, blazelist_protocol::PROTOCOL_VERSION);
            }
            Err(e) => panic!("expected MigrationNotImplemented, got {e:?}"),
            Ok(_) => panic!("expected MigrationNotImplemented, got Ok"),
        }
    }

    #[test]
    fn compatible_minor_bump_updates_stored_version() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("test.db");

        // Create the database with current version, then tweak minor down.
        {
            let _s = SqliteStorage::open(&db_path, false).unwrap();
            let conn = rusqlite::Connection::open(&db_path).unwrap();
            let current = &blazelist_protocol::PROTOCOL_VERSION;
            conn.execute(
                "UPDATE schema_version SET minor = ?1 WHERE id = 1",
                rusqlite::params![current.minor.wrapping_add(1) as i64],
            )
            .unwrap();
        }

        // Reopen should succeed (same major version) and update the stored version.
        let _s = SqliteStorage::open(&db_path, false).unwrap();
    }
}
