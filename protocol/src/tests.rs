use super::*;
use crate::{Card, DateTime, DeletedEntity, NonNegativeI64, RootState, Tag, Version};
use expect_test::expect;
use uuid::Uuid;

// -- Shared test helpers -----------------------------------------------------

const TEST_UUID_A: Uuid = Uuid::from_bytes([1; 16]);
const TEST_UUID_B: Uuid = Uuid::from_bytes([2; 16]);
const TEST_UUID_C: Uuid = Uuid::from_bytes([3; 16]);
const TEST_UUID_D: Uuid = Uuid::from_bytes([4; 16]);
const TEST_UUID_E: Uuid = Uuid::from_bytes([5; 16]);

fn ts(ms: i64) -> DateTime<chrono::Utc> {
    DateTime::from_timestamp_millis(ms).unwrap()
}

fn p(v: i64) -> NonNegativeI64 {
    NonNegativeI64::try_from(v).unwrap()
}

fn test_card() -> Card {
    Card::first(TEST_UUID_A, "C".into(), 1, vec![], false, ts(0), None)
}

fn test_tag() -> Tag {
    Tag::first(TEST_UUID_B, "T".into(), None, ts(0))
}

// -- Protocol round-trip tests -----------------------------------------------

#[test]
fn request_round_trip() {
    let req = Request::GetRoot;
    let bytes = postcard::to_allocvec(&req).unwrap();
    let decoded: Request = postcard::from_bytes(&bytes).unwrap();
    assert_eq!(req, decoded);
}

#[test]
fn response_round_trip() {
    let resp = Response::Ok;
    let bytes = postcard::to_allocvec(&resp).unwrap();
    let decoded: Response = postcard::from_bytes(&bytes).unwrap();
    assert_eq!(resp, decoded);
}

#[test]
fn card_filter_round_trip() {
    for filter in [CardFilter::All, CardFilter::Blazed, CardFilter::Extinguished] {
        let req = Request::ListCards {
            filter,
            limit: None,
        };
        let bytes = postcard::to_allocvec(&req).unwrap();
        let decoded: Request = postcard::from_bytes(&bytes).unwrap();
        assert_eq!(req, decoded);
    }
}

#[test]
fn root_response_round_trip() {
    let resp = Response::Root(RootState::empty());
    let bytes = postcard::to_allocvec(&resp).unwrap();
    let decoded: Response = postcard::from_bytes(&bytes).unwrap();
    assert_eq!(resp, decoded);
}

#[test]
fn card_ancestor_mismatch_round_trip() {
    let resp = Response::Error(ProtocolError::PushFailed(PushError::CardAncestorMismatch(
        Box::new(test_card()),
    )));
    let bytes = postcard::to_allocvec(&resp).unwrap();
    let decoded: Response = postcard::from_bytes(&bytes).unwrap();
    assert_eq!(resp, decoded);
}

#[test]
fn tag_ancestor_mismatch_round_trip() {
    let resp = Response::Error(ProtocolError::PushFailed(PushError::TagAncestorMismatch(
        Box::new(test_tag()),
    )));
    let bytes = postcard::to_allocvec(&resp).unwrap();
    let decoded: Response = postcard::from_bytes(&bytes).unwrap();
    assert_eq!(resp, decoded);
}

#[test]
fn deleted_response_round_trip() {
    let deleted = DeletedEntity::new(TEST_UUID_C);
    let resp = Response::Deleted(deleted);
    let bytes = postcard::to_allocvec(&resp).unwrap();
    let decoded: Response = postcard::from_bytes(&bytes).unwrap();
    assert_eq!(resp, decoded);
}

#[test]
fn already_deleted_round_trip() {
    let resp = Response::Error(ProtocolError::AlreadyDeleted);
    let bytes = postcard::to_allocvec(&resp).unwrap();
    let decoded: Response = postcard::from_bytes(&bytes).unwrap();
    assert_eq!(resp, decoded);
}

#[test]
fn hash_verification_failed_round_trip() {
    let resp = Response::Error(ProtocolError::PushFailed(PushError::HashVerificationFailed));
    let bytes = postcard::to_allocvec(&resp).unwrap();
    let decoded: Response = postcard::from_bytes(&bytes).unwrap();
    assert_eq!(resp, decoded);
}

#[test]
fn empty_push_round_trip() {
    let resp = Response::Error(ProtocolError::PushFailed(PushError::EmptyChain));
    let bytes = postcard::to_allocvec(&resp).unwrap();
    let decoded: Response = postcard::from_bytes(&bytes).unwrap();
    assert_eq!(resp, decoded);
}

#[test]
fn not_found_round_trip() {
    let resp = Response::Error(ProtocolError::NotFound);
    let bytes = postcard::to_allocvec(&resp).unwrap();
    let decoded: Response = postcard::from_bytes(&bytes).unwrap();
    assert_eq!(resp, decoded);
}

#[test]
fn internal_round_trip() {
    let resp = Response::Error(ProtocolError::Internal);
    let bytes = postcard::to_allocvec(&resp).unwrap();
    let decoded: Response = postcard::from_bytes(&bytes).unwrap();
    assert_eq!(resp, decoded);
}

#[test]
fn version_check_round_trip() {
    let check = VersionCheck {
        version: Version::new(0, 0, 0),
    };
    let bytes = postcard::to_allocvec(&check).unwrap();
    let decoded: VersionCheck = postcard::from_bytes(&bytes).unwrap();
    assert_eq!(check, decoded);
}

#[test]
fn version_result_ok_round_trip() {
    let result = VersionResult::Ok;
    let bytes = postcard::to_allocvec(&result).unwrap();
    let decoded: VersionResult = postcard::from_bytes(&bytes).unwrap();
    assert_eq!(result, decoded);
}

#[test]
fn version_result_mismatch_round_trip() {
    let result = VersionResult::Mismatch {
        server_version: Version::new(2, 0, 0),
    };
    let bytes = postcard::to_allocvec(&result).unwrap();
    let decoded: VersionResult = postcard::from_bytes(&bytes).unwrap();
    assert_eq!(result, decoded);
}

#[test]
fn changeset_round_trip() {
    let changeset = ChangeSet {
        cards: vec![],
        tags: vec![],
        deleted: vec![DeletedEntity::new(TEST_UUID_E)],
        root: RootState::empty(),
    };
    let resp = Response::Changes(changeset);
    let bytes = postcard::to_allocvec(&resp).unwrap();
    let decoded: Response = postcard::from_bytes(&bytes).unwrap();
    assert_eq!(resp, decoded);
}

#[test]
fn get_changes_since_round_trip() {
    let req = Request::GetChangesSince {
        sequence: p(42),
        root_hash: blake3::hash(b"test"),
    };
    let bytes = postcard::to_allocvec(&req).unwrap();
    let decoded: Request = postcard::from_bytes(&bytes).unwrap();
    assert_eq!(req, decoded);
}

#[test]
fn root_hash_mismatch_round_trip() {
    let resp = Response::Error(ProtocolError::RootHashMismatch {
        sequence: p(5),
        expected_hash: blake3::hash(b"expected"),
    });
    let bytes = postcard::to_allocvec(&resp).unwrap();
    let decoded: Response = postcard::from_bytes(&bytes).unwrap();
    assert_eq!(resp, decoded);
}

#[test]
fn duplicate_priority_round_trip() {
    let resp = Response::Error(ProtocolError::PushFailed(PushError::DuplicatePriority {
        conflicting_id: TEST_UUID_D,
        priority: 500,
    }));
    let bytes = postcard::to_allocvec(&resp).unwrap();
    let decoded: Response = postcard::from_bytes(&bytes).unwrap();
    assert_eq!(resp, decoded);
}

#[test]
fn orphaned_tag_reference_round_trip() {
    let resp = Response::Error(ProtocolError::PushFailed(
        PushError::OrphanedTagReference {
            tag_id: TEST_UUID_A,
            referencing_card_ids: vec![TEST_UUID_B, TEST_UUID_C],
        },
    ));
    let bytes = postcard::to_allocvec(&resp).unwrap();
    let decoded: Response = postcard::from_bytes(&bytes).unwrap();
    assert_eq!(resp, decoded);
}

#[test]
fn push_batch_round_trip() {
    let req = Request::PushBatch(vec![
        PushItem::Cards(vec![test_card()]),
        PushItem::Tags(vec![test_tag()]),
        PushItem::DeleteCard { id: TEST_UUID_C },
        PushItem::DeleteTag { id: TEST_UUID_D },
    ]);
    let bytes = postcard::to_allocvec(&req).unwrap();
    let decoded: Request = postcard::from_bytes(&bytes).unwrap();
    assert_eq!(req, decoded);
}

#[test]
fn batch_failed_round_trip() {
    let resp = Response::Error(ProtocolError::BatchFailed {
        index: 2,
        error: BatchItemError::Push(PushError::CardAncestorMismatch(Box::new(test_card()))),
    });
    let bytes = postcard::to_allocvec(&resp).unwrap();
    let decoded: Response = postcard::from_bytes(&bytes).unwrap();
    assert_eq!(resp, decoded);
}

#[test]
fn batch_failed_not_found_round_trip() {
    let resp = Response::Error(ProtocolError::BatchFailed {
        index: 0,
        error: BatchItemError::NotFound,
    });
    let bytes = postcard::to_allocvec(&resp).unwrap();
    let decoded: Response = postcard::from_bytes(&bytes).unwrap();
    assert_eq!(resp, decoded);
}

#[test]
fn batch_failed_internal_round_trip() {
    let resp = Response::Error(ProtocolError::BatchFailed {
        index: 1,
        error: BatchItemError::Internal,
    });
    let bytes = postcard::to_allocvec(&resp).unwrap();
    let decoded: Response = postcard::from_bytes(&bytes).unwrap();
    assert_eq!(resp, decoded);
}

#[test]
fn unsupported_request_round_trip() {
    let resp = Response::Error(ProtocolError::UnsupportedRequest);
    let bytes = postcard::to_allocvec(&resp).unwrap();
    let decoded: Response = postcard::from_bytes(&bytes).unwrap();
    assert_eq!(resp, decoded);
}

#[test]
fn push_batch_empty_round_trip() {
    let req = Request::PushBatch(vec![]);
    let bytes = postcard::to_allocvec(&req).unwrap();
    let decoded: Request = postcard::from_bytes(&bytes).unwrap();
    assert_eq!(req, decoded);
}

#[test]
fn subscribe_round_trip() {
    let req = Request::Subscribe;
    let bytes = postcard::to_allocvec(&req).unwrap();
    let decoded: Request = postcard::from_bytes(&bytes).unwrap();
    assert_eq!(req, decoded);
}

#[test]
fn notification_round_trip() {
    let root = RootState::empty();
    let resp = Response::Notification(root);
    let bytes = postcard::to_allocvec(&resp).unwrap();
    let decoded: Response = postcard::from_bytes(&bytes).unwrap();
    assert_eq!(resp, decoded);
}

#[test]
fn get_sequence_history_round_trip() {
    let req = Request::GetSequenceHistory {
        after_sequence: None,
        limit: None,
    };
    let bytes = postcard::to_allocvec(&req).unwrap();
    let decoded: Request = postcard::from_bytes(&bytes).unwrap();
    assert_eq!(req, decoded);
}

#[test]
fn sequence_history_response_round_trip() {
    let entry = SequenceHistoryEntry {
        sequence: p(1),
        hash: blake3::hash(b"test"),
        operations: vec![SequenceOperation {
            entity_id: TEST_UUID_A,
            kind: SequenceOperationKind::CardCreated,
        }],
        created_at: ts(1_000_000),
    };
    let resp = Response::SequenceHistory(vec![entry]);
    let bytes = postcard::to_allocvec(&resp).unwrap();
    let decoded: Response = postcard::from_bytes(&bytes).unwrap();
    assert_eq!(resp, decoded);
}

// -- ProtocolError Display tests --

#[test]
fn protocol_error_display() {
    expect!["entity not found"].assert_eq(&ProtocolError::NotFound.to_string());
    expect!["entity already deleted"].assert_eq(&ProtocolError::AlreadyDeleted.to_string());
    expect!["push failed: hash verification failed"]
        .assert_eq(&ProtocolError::PushFailed(PushError::HashVerificationFailed).to_string());
    expect!["push failed: empty version chain"]
        .assert_eq(&ProtocolError::PushFailed(PushError::EmptyChain).to_string());
    expect!["unsupported request"].assert_eq(&ProtocolError::UnsupportedRequest.to_string());
    expect!["internal error"].assert_eq(&ProtocolError::Internal.to_string());
}

#[test]
fn protocol_error_is_std_error() {
    let err: Box<dyn std::error::Error> = Box::new(ProtocolError::NotFound);
    expect!["entity not found"].assert_eq(&err.to_string());
}

#[test]
fn push_error_display() {
    expect!["entity already deleted"].assert_eq(&PushError::AlreadyDeleted.to_string());
    expect!["hash verification failed"].assert_eq(&PushError::HashVerificationFailed.to_string());
    expect!["empty version chain"].assert_eq(&PushError::EmptyChain.to_string());
}

#[test]
fn push_error_is_std_error() {
    let err: Box<dyn std::error::Error> = Box::new(PushError::AlreadyDeleted);
    expect!["entity already deleted"].assert_eq(&err.to_string());
}

#[test]
fn batch_item_error_display() {
    expect!["entity not found"].assert_eq(&BatchItemError::NotFound.to_string());
    expect!["entity already deleted"].assert_eq(&BatchItemError::AlreadyDeleted.to_string());
    expect!["internal error"].assert_eq(&BatchItemError::Internal.to_string());
    expect!["hash verification failed"]
        .assert_eq(&BatchItemError::Push(PushError::HashVerificationFailed).to_string());
}

// -- Response helper method tests --

#[test]
fn into_ok_success() {
    assert!(Response::Ok.into_ok().is_ok());
}

#[test]
fn into_ok_error() {
    let resp = Response::Error(ProtocolError::NotFound);
    assert_eq!(
        resp.into_ok().unwrap_err(),
        ResponseExtractError::Protocol(ProtocolError::NotFound)
    );
}

#[test]
fn into_ok_wrong_variant() {
    let resp = Response::Root(RootState::empty());
    assert!(resp.into_ok().is_err());
}

#[test]
fn into_card_success() {
    let card = test_card();
    let resp = Response::Card(card.clone());
    assert_eq!(resp.into_card().unwrap(), card);
}

#[test]
fn into_card_error() {
    let resp = Response::Error(ProtocolError::NotFound);
    assert_eq!(
        resp.into_card().unwrap_err(),
        ResponseExtractError::Protocol(ProtocolError::NotFound)
    );
}

#[test]
fn into_card_wrong_variant() {
    let resp = Response::Ok;
    assert!(resp.into_card().is_err());
}

#[test]
fn into_cards_success() {
    let resp = Response::Cards(vec![]);
    assert_eq!(resp.into_cards().unwrap(), vec![]);
}

#[test]
fn into_tag_success() {
    let tag = test_tag();
    let resp = Response::Tag(tag.clone());
    assert_eq!(resp.into_tag().unwrap(), tag);
}

#[test]
fn into_tags_success() {
    let resp = Response::Tags(vec![]);
    assert_eq!(resp.into_tags().unwrap(), vec![]);
}

#[test]
fn into_root_success() {
    let root = RootState::empty();
    let resp = Response::Root(root.clone());
    assert_eq!(resp.into_root().unwrap(), root);
}

#[test]
fn into_deleted_success() {
    let deleted = DeletedEntity::new(TEST_UUID_C);
    let resp = Response::Deleted(deleted.clone());
    assert_eq!(resp.into_deleted().unwrap(), deleted);
}

#[test]
fn into_changes_success() {
    let changes = ChangeSet {
        cards: vec![],
        tags: vec![],
        deleted: vec![],
        root: RootState::empty(),
    };
    let resp = Response::Changes(changes.clone());
    assert_eq!(resp.into_changes().unwrap(), changes);
}

#[test]
fn into_notification_success() {
    let root = RootState::empty();
    let resp = Response::Notification(root.clone());
    assert_eq!(resp.into_notification().unwrap(), root);
}

#[test]
fn into_notification_error() {
    let resp = Response::Error(ProtocolError::NotFound);
    assert_eq!(
        resp.into_notification().unwrap_err(),
        ResponseExtractError::Protocol(ProtocolError::NotFound)
    );
}

#[test]
fn into_notification_wrong_variant() {
    let resp = Response::Ok;
    assert!(matches!(
        resp.into_notification().unwrap_err(),
        ResponseExtractError::UnexpectedVariant
    ));
}

#[test]
fn into_card_history_success() {
    let resp = Response::CardHistory(vec![test_card()]);
    assert_eq!(resp.into_card_history().unwrap().len(), 1);
}

#[test]
fn into_tag_history_success() {
    let resp = Response::TagHistory(vec![test_tag()]);
    assert_eq!(resp.into_tag_history().unwrap().len(), 1);
}

#[test]
fn into_sequence_history_success() {
    let resp = Response::SequenceHistory(vec![]);
    assert_eq!(resp.into_sequence_history().unwrap(), vec![]);
}

// -- ResponseExtractError tests --

#[test]
fn response_extract_error_display_protocol() {
    let err = ResponseExtractError::Protocol(ProtocolError::NotFound);
    expect!["entity not found"].assert_eq(&err.to_string());
}

#[test]
fn response_extract_error_display_unexpected() {
    let err = ResponseExtractError::UnexpectedVariant;
    expect!["unexpected response variant"].assert_eq(&err.to_string());
}

#[test]
fn response_extract_error_from_protocol_error() {
    let protocol_err = ProtocolError::NotFound;
    let extract_err: ResponseExtractError = protocol_err.into();
    assert_eq!(
        extract_err,
        ResponseExtractError::Protocol(ProtocolError::NotFound)
    );
}

#[test]
fn push_error_into_protocol_error() {
    let push_err = PushError::HashVerificationFailed;
    let protocol_err: ProtocolError = push_err.into();
    assert_eq!(
        protocol_err,
        ProtocolError::PushFailed(PushError::HashVerificationFailed)
    );
}

// -- Discriminant stability tests --
//
// Postcard encodes enum variants by zero-based index. These tests verify
// that the first byte of the serialized form (the discriminant) stays fixed
// for every variant. If a variant is reordered or a new variant is inserted
// before an existing one, these tests will fail — alerting us to a
// wire-breaking change.

/// Helper: serialize a value and return the first byte (postcard discriminant).
fn discriminant<T: serde::Serialize>(val: &T) -> u8 {
    postcard::to_allocvec(val).unwrap()[0]
}

#[test]
fn request_discriminant_stability() {
    expect!["0"]
        .assert_eq(&discriminant(&Request::PushCardVersions(vec![test_card()])).to_string());
    expect!["1"].assert_eq(&discriminant(&Request::GetCard { id: Uuid::nil() }).to_string());
    expect!["2"].assert_eq(
        &discriminant(&Request::GetCardHistory {
            id: Uuid::nil(),
            limit: None,
        })
        .to_string(),
    );
    expect!["3"].assert_eq(
        &discriminant(&Request::ListCards {
            filter: CardFilter::All,
            limit: None,
        })
        .to_string(),
    );
    expect!["4"].assert_eq(&discriminant(&Request::DeleteCard { id: Uuid::nil() }).to_string());
    expect!["5"].assert_eq(&discriminant(&Request::PushTagVersions(vec![test_tag()])).to_string());
    expect!["6"].assert_eq(&discriminant(&Request::GetTag { id: Uuid::nil() }).to_string());
    expect!["7"].assert_eq(&discriminant(&Request::ListTags).to_string());
    expect!["8"].assert_eq(&discriminant(&Request::DeleteTag { id: Uuid::nil() }).to_string());
    expect!["9"].assert_eq(&discriminant(&Request::GetRoot).to_string());
    expect!["10"].assert_eq(
        &discriminant(&Request::GetChangesSince {
            sequence: p(0),
            root_hash: blake3::Hash::from_bytes([0; 32]),
        })
        .to_string(),
    );
    expect!["11"].assert_eq(&discriminant(&Request::PushBatch(vec![])).to_string());
    expect!["12"].assert_eq(&discriminant(&Request::Subscribe).to_string());
    expect!["13"].assert_eq(
        &discriminant(&Request::GetTagHistory {
            id: Uuid::nil(),
            limit: None,
        })
        .to_string(),
    );
    expect!["14"].assert_eq(
        &discriminant(&Request::GetSequenceHistory {
            after_sequence: None,
            limit: None,
        })
        .to_string(),
    );
}

#[test]
fn response_discriminant_stability() {
    expect!["0"].assert_eq(&discriminant(&Response::Ok).to_string());
    expect!["1"].assert_eq(
        &discriminant(&Response::Card(Card::first(
            TEST_UUID_A,
            "".into(),
            1,
            vec![],
            false,
            ts(0),
            None,
        )))
        .to_string(),
    );
    expect!["2"].assert_eq(&discriminant(&Response::Cards(vec![])).to_string());
    expect!["3"].assert_eq(
        &discriminant(&Response::Tag(Tag::first(
            TEST_UUID_A,
            "".into(),
            None,
            ts(0),
        )))
        .to_string(),
    );
    expect!["4"].assert_eq(&discriminant(&Response::Tags(vec![])).to_string());
    expect!["5"].assert_eq(&discriminant(&Response::Root(RootState::empty())).to_string());
    expect!["6"]
        .assert_eq(&discriminant(&Response::Deleted(DeletedEntity::new(Uuid::nil()))).to_string());
    expect!["7"].assert_eq(
        &discriminant(&Response::Changes(ChangeSet {
            cards: vec![],
            tags: vec![],
            deleted: vec![],
            root: RootState::empty(),
        }))
        .to_string(),
    );
    expect!["8"].assert_eq(&discriminant(&Response::Notification(RootState::empty())).to_string());
    expect!["9"].assert_eq(&discriminant(&Response::Error(ProtocolError::NotFound)).to_string());
    expect!["10"].assert_eq(&discriminant(&Response::CardHistory(vec![])).to_string());
    expect!["11"].assert_eq(&discriminant(&Response::TagHistory(vec![])).to_string());
    expect!["12"].assert_eq(&discriminant(&Response::SequenceHistory(vec![])).to_string());
}

#[test]
fn protocol_error_discriminant_stability() {
    expect!["0"].assert_eq(&discriminant(&ProtocolError::NotFound).to_string());
    expect!["1"].assert_eq(&discriminant(&ProtocolError::AlreadyDeleted).to_string());
    expect!["2"].assert_eq(
        &discriminant(&ProtocolError::PushFailed(PushError::AlreadyDeleted)).to_string(),
    );
    expect!["3"].assert_eq(
        &discriminant(&ProtocolError::BatchFailed {
            index: 0,
            error: BatchItemError::NotFound,
        })
        .to_string(),
    );
    expect!["4"].assert_eq(
        &discriminant(&ProtocolError::RootHashMismatch {
            sequence: p(0),
            expected_hash: blake3::Hash::from_bytes([0; 32]),
        })
        .to_string(),
    );
    expect!["5"].assert_eq(&discriminant(&ProtocolError::UnsupportedRequest).to_string());
    expect!["6"].assert_eq(&discriminant(&ProtocolError::Internal).to_string());
}

#[test]
fn push_error_discriminant_stability() {
    expect!["0"].assert_eq(
        &discriminant(&PushError::CardAncestorMismatch(Box::new(test_card()))).to_string(),
    );
    expect!["1"].assert_eq(
        &discriminant(&PushError::TagAncestorMismatch(Box::new(test_tag()))).to_string(),
    );
    expect!["2"].assert_eq(&discriminant(&PushError::AlreadyDeleted).to_string());
    expect!["3"].assert_eq(
        &discriminant(&PushError::OrphanedTagReference {
            tag_id: Uuid::nil(),
            referencing_card_ids: vec![],
        })
        .to_string(),
    );
    expect!["4"].assert_eq(&discriminant(&PushError::HashVerificationFailed).to_string());
    expect!["5"].assert_eq(&discriminant(&PushError::EmptyChain).to_string());
    expect!["6"].assert_eq(
        &discriminant(&PushError::DuplicatePriority {
            conflicting_id: Uuid::nil(),
            priority: 0,
        })
        .to_string(),
    );
}

#[test]
fn batch_item_error_discriminant_stability() {
    expect!["0"]
        .assert_eq(&discriminant(&BatchItemError::Push(PushError::AlreadyDeleted)).to_string());
    expect!["1"].assert_eq(&discriminant(&BatchItemError::NotFound).to_string());
    expect!["2"].assert_eq(&discriminant(&BatchItemError::AlreadyDeleted).to_string());
    expect!["3"].assert_eq(&discriminant(&BatchItemError::Internal).to_string());
}

#[test]
fn push_item_discriminant_stability() {
    expect!["0"].assert_eq(&discriminant(&PushItem::Cards(vec![])).to_string());
    expect!["1"].assert_eq(&discriminant(&PushItem::Tags(vec![])).to_string());
    expect!["2"].assert_eq(&discriminant(&PushItem::DeleteCard { id: Uuid::nil() }).to_string());
    expect!["3"].assert_eq(&discriminant(&PushItem::DeleteTag { id: Uuid::nil() }).to_string());
}

#[test]
fn card_filter_discriminant_stability() {
    expect!["0"].assert_eq(&discriminant(&CardFilter::All).to_string());
    expect!["1"].assert_eq(&discriminant(&CardFilter::Blazed).to_string());
    expect!["2"].assert_eq(&discriminant(&CardFilter::Extinguished).to_string());
}

#[test]
fn version_result_discriminant_stability() {
    expect!["0"].assert_eq(&discriminant(&VersionResult::Ok).to_string());
    expect!["1"].assert_eq(
        &discriminant(&VersionResult::Mismatch {
            server_version: Version::new(0, 0, 0),
        })
        .to_string(),
    );
}

#[test]
fn sequence_operation_kind_discriminant_stability() {
    expect!["0"].assert_eq(&discriminant(&SequenceOperationKind::CardCreated).to_string());
    expect!["1"].assert_eq(&discriminant(&SequenceOperationKind::CardUpdated).to_string());
    expect!["2"].assert_eq(&discriminant(&SequenceOperationKind::TagCreated).to_string());
    expect!["3"].assert_eq(&discriminant(&SequenceOperationKind::TagUpdated).to_string());
    expect!["4"].assert_eq(&discriminant(&SequenceOperationKind::EntityDeleted).to_string());
}
