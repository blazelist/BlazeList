//! Shared `Client` trait for BlazeList clients.
//!
//! Both the CLI (QUIC) and WASM (WebTransport) clients implement this trait.
//! The single required method is [`request`](Client::request); all convenience
//! methods are provided as defaults.

use blazelist_protocol::{
    Card, CardFilter, ChangeSet, NonNegativeI64, PushItem, Request, Response, RootState,
    SequenceHistoryEntry, Tag,
};
use uuid::Uuid;

use crate::error::ClientError;

/// A connected BlazeList client.
///
/// Transport-specific details (`connect`, `subscribe`) live on each concrete
/// type. This trait captures the shared request/response surface.
pub trait Client {
    /// Send a request and receive a response.
    fn request(
        &self,
        req: &Request,
    ) -> impl std::future::Future<Output = Result<Response, ClientError>>;

    /// List cards matching the given filter.
    fn list_cards(
        &self,
        filter: CardFilter,
    ) -> impl std::future::Future<Output = Result<Vec<Card>, ClientError>> {
        async move {
            let resp = self
                .request(&Request::ListCards {
                    filter,
                    limit: None,
                })
                .await?;
            Ok(resp.into_cards()?)
        }
    }

    /// Push a single card (as a one-element version chain).
    ///
    /// Returns the new [`RootState`] after the push.
    fn push_card(
        &self,
        card: Card,
    ) -> impl std::future::Future<Output = Result<RootState, ClientError>> {
        async move {
            let resp = self.request(&Request::PushCardVersions(vec![card])).await?;
            Ok(resp.into_root()?)
        }
    }

    /// Push multiple card versions as a chain.
    ///
    /// Returns the new [`RootState`] after the push.
    fn push_card_versions(
        &self,
        versions: Vec<Card>,
    ) -> impl std::future::Future<Output = Result<RootState, ClientError>> {
        async move {
            let resp = self.request(&Request::PushCardVersions(versions)).await?;
            Ok(resp.into_root()?)
        }
    }

    /// Delete a card by UUID.
    fn delete_card(&self, id: Uuid) -> impl std::future::Future<Output = Result<(), ClientError>> {
        async move {
            let resp = self.request(&Request::DeleteCard { id }).await?;
            resp.into_deleted()?;
            Ok(())
        }
    }

    /// Get version history for a card by UUID (newest first from server).
    fn get_card_history(
        &self,
        id: Uuid,
    ) -> impl std::future::Future<Output = Result<Vec<Card>, ClientError>> {
        async move {
            let resp = self
                .request(&Request::GetCardHistory { id, limit: None })
                .await?;
            Ok(resp.into_card_history()?)
        }
    }

    /// Get a single card by UUID.
    fn get_card(&self, id: Uuid) -> impl std::future::Future<Output = Result<Card, ClientError>> {
        async move {
            let resp = self.request(&Request::GetCard { id }).await?;
            Ok(resp.into_card()?)
        }
    }

    /// Get the current root state (hash + sequence).
    fn get_root(&self) -> impl std::future::Future<Output = Result<RootState, ClientError>> {
        async move {
            let resp = self.request(&Request::GetRoot).await?;
            Ok(resp.into_root()?)
        }
    }

    /// Get changes since a given root sequence for incremental sync.
    fn get_changes_since(
        &self,
        sequence: NonNegativeI64,
        root_hash: blake3::Hash,
    ) -> impl std::future::Future<Output = Result<ChangeSet, ClientError>> {
        async move {
            let resp = self
                .request(&Request::GetChangesSince {
                    sequence,
                    root_hash,
                })
                .await?;
            Ok(resp.into_changes()?)
        }
    }

    /// Get version history for a tag by UUID.
    fn get_tag_history(
        &self,
        id: Uuid,
    ) -> impl std::future::Future<Output = Result<Vec<Tag>, ClientError>> {
        async move {
            let resp = self
                .request(&Request::GetTagHistory { id, limit: None })
                .await?;
            Ok(resp.into_tag_history()?)
        }
    }

    /// List all tags.
    fn list_tags(&self) -> impl std::future::Future<Output = Result<Vec<Tag>, ClientError>> {
        async move {
            let resp = self.request(&Request::ListTags).await?;
            Ok(resp.into_tags()?)
        }
    }

    /// Push a single tag (as a one-element version chain).
    ///
    /// Returns the new [`RootState`] after the push.
    fn push_tag(
        &self,
        tag: Tag,
    ) -> impl std::future::Future<Output = Result<RootState, ClientError>> {
        async move {
            let resp = self.request(&Request::PushTagVersions(vec![tag])).await?;
            Ok(resp.into_root()?)
        }
    }

    /// Delete a tag by UUID.
    fn delete_tag(&self, id: Uuid) -> impl std::future::Future<Output = Result<(), ClientError>> {
        async move {
            let resp = self.request(&Request::DeleteTag { id }).await?;
            resp.into_deleted()?;
            Ok(())
        }
    }

    /// Get the full sequence history.
    fn get_sequence_history(
        &self,
    ) -> impl std::future::Future<Output = Result<Vec<SequenceHistoryEntry>, ClientError>> {
        async move {
            let resp = self
                .request(&Request::GetSequenceHistory {
                    after_sequence: None,
                    limit: None,
                })
                .await?;
            Ok(resp.into_sequence_history()?)
        }
    }

    /// Push a batch of items atomically.
    ///
    /// Returns the new [`RootState`] after the batch.
    fn push_batch(
        &self,
        items: Vec<PushItem>,
    ) -> impl std::future::Future<Output = Result<RootState, ClientError>> {
        async move {
            let resp = self.request(&Request::PushBatch(items)).await?;
            Ok(resp.into_root()?)
        }
    }
}
