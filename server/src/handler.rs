//! Request handler — maps protocol messages to storage operations.
//!
//! This module is transport-agnostic and shared between QUIC and WebTransport.

use blazelist_protocol::{BatchItemError, ProtocolError, PushError, Request, Response};

use crate::storage::{BatchError, PushOpError, Storage, StorageError};

/// Handle a single request against the given storage and return a response.
pub fn handle_request<S: Storage>(storage: &S, request: Request) -> Response {
    match request {
        Request::PushCardVersions(versions) => match storage.push_card_versions(&versions) {
            Ok(()) => match storage.get_root() {
                Ok(root) => Response::Root(root),
                Err(e) => Response::Error(storage_error_to_protocol(e)),
            },
            Err(e) => Response::Error(push_op_error_to_protocol(e)),
        },

        Request::GetCard { id } => match storage.get_card(id) {
            Ok(Some(card)) => Response::Card(card),
            Ok(None) => Response::Error(ProtocolError::NotFound),
            Err(e) => Response::Error(storage_error_to_protocol(e)),
        },

        Request::GetCardHistory { id, limit } => match storage.get_card_history(id, limit) {
            Ok(cards) => Response::CardHistory(cards),
            Err(e) => Response::Error(storage_error_to_protocol(e)),
        },

        Request::ListCards { filter, limit } => match storage.list_cards(filter, limit) {
            Ok(cards) => Response::Cards(cards),
            Err(e) => Response::Error(storage_error_to_protocol(e)),
        },

        Request::DeleteCard { id } => match storage.delete_card(id) {
            Ok(deleted) => Response::Deleted(deleted),
            Err(e) => Response::Error(storage_error_to_protocol(e)),
        },

        Request::PushTagVersions(versions) => match storage.push_tag_versions(&versions) {
            Ok(()) => match storage.get_root() {
                Ok(root) => Response::Root(root),
                Err(e) => Response::Error(storage_error_to_protocol(e)),
            },
            Err(e) => Response::Error(push_op_error_to_protocol(e)),
        },

        Request::GetTag { id } => match storage.get_tag(id) {
            Ok(Some(tag)) => Response::Tag(tag),
            Ok(None) => Response::Error(ProtocolError::NotFound),
            Err(e) => Response::Error(storage_error_to_protocol(e)),
        },

        Request::ListTags => match storage.list_tags() {
            Ok(tags) => Response::Tags(tags),
            Err(e) => Response::Error(storage_error_to_protocol(e)),
        },

        Request::DeleteTag { id } => match storage.delete_tag(id) {
            Ok(deleted) => Response::Deleted(deleted),
            Err(e) => Response::Error(storage_error_to_protocol(e)),
        },

        Request::GetRoot => match storage.get_root() {
            Ok(root) => Response::Root(root),
            Err(e) => Response::Error(storage_error_to_protocol(e)),
        },

        Request::GetChangesSince {
            sequence,
            root_hash,
        } => match storage.get_changes_since(sequence, root_hash) {
            Ok(changes) => Response::Changes(changes),
            Err(e) => Response::Error(storage_error_to_protocol(e)),
        },

        Request::PushBatch(items) => match storage.push_batch(&items) {
            Ok(()) => match storage.get_root() {
                Ok(root) => Response::Root(root),
                Err(e) => Response::Error(storage_error_to_protocol(e)),
            },
            Err(BatchError { index, error }) => {
                let batch_item_error = match error {
                    PushOpError::Domain(push_err) => BatchItemError::Push(push_err),
                    PushOpError::Internal(_) => BatchItemError::Internal,
                };
                Response::Error(ProtocolError::BatchFailed {
                    index: index as u32,
                    error: batch_item_error,
                })
            }
        },

        // Subscribe is handled at the transport layer, not here.
        Request::Subscribe => Response::Error(ProtocolError::UnsupportedRequest),

        Request::GetTagHistory { id, limit } => match storage.get_tag_history(id, limit) {
            Ok(tags) => Response::TagHistory(tags),
            Err(e) => Response::Error(storage_error_to_protocol(e)),
        },

        Request::GetSequenceHistory {
            after_sequence,
            limit,
        } => match storage.get_sequence_history(after_sequence, limit) {
            Ok(entries) => Response::SequenceHistory(entries),
            Err(e) => Response::Error(storage_error_to_protocol(e)),
        },
    }
}

/// Convert a [`PushOpError`] into a [`ProtocolError`].
fn push_op_error_to_protocol(e: PushOpError) -> ProtocolError {
    match e {
        PushOpError::Domain(push_err) => ProtocolError::PushFailed(push_err),
        PushOpError::Internal(_) => ProtocolError::Internal,
    }
}

/// Convert a [`StorageError`] into a [`ProtocolError`].
fn storage_error_to_protocol(e: StorageError) -> ProtocolError {
    match e {
        StorageError::NotFound => ProtocolError::NotFound,
        StorageError::AlreadyDeleted => ProtocolError::AlreadyDeleted,
        StorageError::OrphanedTagReference {
            tag_id,
            referencing_card_ids,
        } => ProtocolError::PushFailed(PushError::OrphanedTagReference {
            tag_id,
            referencing_card_ids,
        }),
        StorageError::RootHashMismatch {
            sequence,
            expected_hash,
        } => ProtocolError::RootHashMismatch {
            sequence,
            expected_hash,
        },
        StorageError::Internal(_) => ProtocolError::Internal,
        // These errors are startup-only and should never reach the request
        // handler; map to Internal as a safety fallback.
        StorageError::IncompatibleVersion { .. } | StorageError::MigrationNotImplemented { .. } => {
            ProtocolError::Internal
        }
    }
}
