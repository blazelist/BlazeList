#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use blazelist_protocol::{
        BatchItemError, CardFilter, ProtocolError, PushError, Request, Response,
    };
    use blazelist_protocol::{Card, DateTime, Entity, NonNegativeI64, RootState, Tag, Utc};
    use expect_test::expect;
    use quinn::Endpoint;
    use tokio::sync::broadcast;
    use uuid::Uuid;

    use crate::SqliteStorage;
    use crate::handler::handle_request;
    use crate::quic::server::{perform_version_handshake, run_server, send_request};
    use crate::quic::tls::{client_config_for_cert, self_signed_server_config};
    use crate::quic::{read_message, write_message};

    const ID_A: Uuid =
        uuid::Uuid::from_bytes([1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16]);
    const TAG_ID: Uuid = uuid::Uuid::from_bytes([
        0xa1, 0xb2, 0xc3, 0xd4, 0xe5, 0xf6, 0x47, 0x08, 0x89, 0x0a, 0x0b, 0x0c, 0x0d, 0x0e, 0x0f,
        0x10,
    ]);

    fn ts(ms: i64) -> DateTime<Utc> {
        DateTime::from_timestamp_millis(ms).unwrap()
    }

    fn p(v: i64) -> NonNegativeI64 {
        NonNegativeI64::try_from(v).unwrap()
    }

    // -- QUIC test environment -----------------------------------------------

    /// Spins up a QUIC server with an in-memory database and connects a client.
    struct TestEnv {
        connection: quinn::Connection,
        server_endpoint: Endpoint,
        server_handle: tokio::task::JoinHandle<()>,
    }

    impl TestEnv {
        async fn start() -> Self {
            let (tx, _) = broadcast::channel::<RootState>(64);

            let (server_config, cert_material) = self_signed_server_config().unwrap();
            let server_endpoint =
                Endpoint::server(server_config, "127.0.0.1:0".parse().unwrap()).unwrap();
            let server_addr = server_endpoint.local_addr().unwrap();

            let storage = Arc::new(SqliteStorage::open_in_memory().unwrap());
            let server_handle = tokio::spawn(run_server(
                server_endpoint.clone(),
                Arc::clone(&storage),
                tx,
            ));

            let client_config = client_config_for_cert(&cert_material.cert_der).unwrap();
            let mut client_endpoint = Endpoint::client("0.0.0.0:0".parse().unwrap()).unwrap();
            client_endpoint.set_default_client_config(client_config);

            let connection = client_endpoint
                .connect(server_addr, "localhost")
                .unwrap()
                .await
                .unwrap();
            perform_version_handshake(&connection).await.unwrap();

            Self {
                connection,
                server_endpoint,
                server_handle,
            }
        }

        async fn request(&self, req: &Request) -> Response {
            send_request(&self.connection, req).await.unwrap()
        }

        async fn shutdown(self) {
            self.connection.close(0u32.into(), b"done");
            self.server_endpoint.close(0u32.into(), b"done");
            let _ =
                tokio::time::timeout(std::time::Duration::from_secs(1), self.server_handle).await;
        }
    }

    // -- QUIC integration tests ----------------------------------------------

    #[tokio::test]
    async fn quic_round_trip() {
        let env = TestEnv::start().await;

        // GetRoot on empty database.
        match env.request(&Request::GetRoot).await {
            Response::Root(root) => {
                expect!["0"].assert_eq(&root.sequence.to_string());
            }
            other => panic!("expected Root, got {other:?}"),
        }

        // Create a card.
        let card = Card::first(ID_A, "body".into(), 42, vec![], false, ts(1000), None);
        assert!(matches!(
            env.request(&Request::PushCardVersions(vec![card.clone()]))
                .await,
            Response::Root(_)
        ));

        // GetCard.
        match env.request(&Request::GetCard { id: ID_A }).await {
            Response::Card(c) => {
                expect!["true"].assert_eq(&c.verify().to_string());
            }
            other => panic!("expected Card, got {other:?}"),
        }

        // ListCards.
        match env
            .request(&Request::ListCards {
                filter: CardFilter::All,
                limit: None,
            })
            .await
        {
            Response::Cards(cards) => {
                expect!["1"].assert_eq(&cards.len().to_string());
            }
            other => panic!("expected Cards, got {other:?}"),
        }

        // Create a tag.
        let tag = Tag::first(TAG_ID, "Test tag".into(), None, ts(1000));
        assert!(matches!(
            env.request(&Request::PushTagVersions(vec![tag.clone()]))
                .await,
            Response::Root(_)
        ));

        // GetTag.
        match env.request(&Request::GetTag { id: TAG_ID }).await {
            Response::Tag(t) => {
                expect!["Test tag"].assert_eq(t.title());
            }
            other => panic!("expected Tag, got {other:?}"),
        }

        // Delete card.
        assert!(matches!(
            env.request(&Request::DeleteCard { id: ID_A }).await,
            Response::Deleted(_)
        ));

        // Card is gone.
        assert!(matches!(
            env.request(&Request::GetCard { id: ID_A }).await,
            Response::Error(ProtocolError::NotFound)
        ));

        env.shutdown().await;
    }

    #[tokio::test]
    async fn quic_concurrent_requests() {
        let env = TestEnv::start().await;

        // Create 10 cards sequentially first.
        for i in 0u8..10 {
            let id = Uuid::from_bytes([i + 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
            let card = Card::first(
                id,
                String::new(),
                i64::from(i + 1),
                vec![],
                false,
                ts(0),
                None,
            );
            assert!(matches!(
                env.request(&Request::PushCardVersions(vec![card])).await,
                Response::Root(_)
            ));
        }

        // Now read all 10 cards concurrently using separate bi-streams.
        let mut handles = Vec::new();
        for i in 0u8..10 {
            let conn = env.connection.clone();
            let id = Uuid::from_bytes([i + 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
            handles.push(tokio::spawn(async move {
                send_request(&conn, &Request::GetCard { id }).await
            }));
        }

        for (i, handle) in handles.into_iter().enumerate() {
            let resp = handle.await.unwrap().unwrap();
            match resp {
                Response::Card(c) => {
                    assert!(c.verify());
                }
                other => panic!("expected Card for index {i}, got {other:?}"),
            }
        }

        // Concurrent mixed operations: list cards + get root at the same time.
        let (list_resp, root_resp) = tokio::join!(
            send_request(
                &env.connection,
                &Request::ListCards {
                    filter: CardFilter::All,
                    limit: None
                }
            ),
            send_request(&env.connection, &Request::GetRoot)
        );
        match list_resp.unwrap() {
            Response::Cards(cards) => assert_eq!(cards.len(), 10),
            other => panic!("expected Cards, got {other:?}"),
        }
        match root_resp.unwrap() {
            Response::Root(root) => assert_eq!(root.sequence, p(10)),
            other => panic!("expected Root, got {other:?}"),
        }

        env.shutdown().await;
    }

    #[tokio::test]
    async fn quic_subscribe_notifications() {
        let env = TestEnv::start().await;

        // Open a subscribe stream.
        let (mut sub_send, mut sub_recv) = env.connection.open_bi().await.unwrap();
        write_message::<Request, _>(&mut sub_send, &Request::Subscribe)
            .await
            .unwrap();
        let _ = sub_send.finish();
        let sub_resp: Response = read_message::<Response, _>(&mut sub_recv).await.unwrap();
        assert!(matches!(sub_resp, Response::Ok));

        // Push a card -- subscriber should get notified.
        let card = Card::first(ID_A, String::new(), 1, vec![], false, ts(0), None);
        assert!(matches!(
            env.request(&Request::PushCardVersions(vec![card])).await,
            Response::Root(_)
        ));

        // Read the notification from the subscribe stream.
        let notification: Response = tokio::time::timeout(
            std::time::Duration::from_secs(2),
            read_message::<Response, _>(&mut sub_recv),
        )
        .await
        .expect("notification should arrive within 2s")
        .unwrap();

        match notification {
            Response::Notification(root) => {
                expect!["1"].assert_eq(&root.sequence.to_string());
                assert_ne!(root.hash, blazelist_protocol::ZERO_HASH);
            }
            other => panic!("expected Notification, got {other:?}"),
        }

        // Push another card -- should get a second notification.
        let id_c = Uuid::from_bytes([0xcc; 16]);
        let card2 = Card::first(id_c, String::new(), 2, vec![], false, ts(0), None);
        assert!(matches!(
            env.request(&Request::PushCardVersions(vec![card2])).await,
            Response::Root(_)
        ));

        let notification2: Response = tokio::time::timeout(
            std::time::Duration::from_secs(2),
            read_message::<Response, _>(&mut sub_recv),
        )
        .await
        .expect("second notification should arrive")
        .unwrap();

        match notification2 {
            Response::Notification(root) => {
                expect!["2"].assert_eq(&root.sequence.to_string());
            }
            other => panic!("expected Notification, got {other:?}"),
        }

        // Read operations should NOT trigger notifications.
        assert!(matches!(
            env.request(&Request::GetRoot).await,
            Response::Root(_)
        ));

        // Give the server a moment -- no notification should come.
        let timeout_result: Result<Result<Response, _>, _> = tokio::time::timeout(
            std::time::Duration::from_millis(100),
            read_message::<Response, _>(&mut sub_recv),
        )
        .await;
        assert!(
            timeout_result.is_err(),
            "read should not produce a notification"
        );

        // Delete triggers notification.
        assert!(matches!(
            env.request(&Request::DeleteCard { id: ID_A }).await,
            Response::Deleted(_)
        ));

        let notification3: Response = tokio::time::timeout(
            std::time::Duration::from_secs(2),
            read_message::<Response, _>(&mut sub_recv),
        )
        .await
        .expect("delete notification should arrive")
        .unwrap();
        assert!(matches!(notification3, Response::Notification(_)));

        env.shutdown().await;
    }

    #[tokio::test]
    async fn quic_push_batch() {
        use blazelist_protocol::PushItem;

        let env = TestEnv::start().await;

        // Batch: card + tag.
        let card = Card::first(ID_A, "body".into(), 10, vec![], false, ts(0), None);
        let tag = Tag::first(TAG_ID, "Batch tag".into(), None, ts(0));
        assert!(matches!(
            env.request(&Request::PushBatch(vec![
                PushItem::Cards(vec![card.clone()]),
                PushItem::Tags(vec![tag.clone()]),
            ]))
            .await,
            Response::Root(_)
        ));

        // Verify card.
        match env.request(&Request::GetCard { id: ID_A }).await {
            Response::Card(c) => assert_eq!(c, card),
            other => panic!("expected Card, got {other:?}"),
        }

        // Verify tag.
        match env.request(&Request::GetTag { id: TAG_ID }).await {
            Response::Tag(t) => assert_eq!(t, tag),
            other => panic!("expected Tag, got {other:?}"),
        }

        // Root count is 1 (single batch = single recompute).
        match env.request(&Request::GetRoot).await {
            Response::Root(root) => {
                expect!["1"].assert_eq(&root.sequence.to_string());
            }
            other => panic!("expected Root, got {other:?}"),
        }

        env.shutdown().await;
    }

    // -- Handler unit tests (no network) -------------------------------------

    #[test]
    fn handle_request_get_root_empty() {
        let storage = SqliteStorage::open_in_memory().unwrap();
        let resp = handle_request(&storage, Request::GetRoot);
        match resp {
            Response::Root(root) => {
                expect!["0"].assert_eq(&root.sequence.to_string());
                expect!["0000000000000000000000000000000000000000000000000000000000000000"]
                    .assert_eq(&root.hash.to_string());
            }
            other => panic!("expected Root, got {other:?}"),
        }
    }

    #[test]
    fn handle_request_card_lifecycle() {
        let storage = SqliteStorage::open_in_memory().unwrap();

        // Create card.
        let card = Card::first(ID_A, "Body".into(), 1, vec![], false, ts(0), None);
        let resp = handle_request(&storage, Request::PushCardVersions(vec![card.clone()]));
        assert!(matches!(resp, Response::Root(_)));

        // Get card.
        let resp = handle_request(&storage, Request::GetCard { id: ID_A });
        match resp {
            Response::Card(c) => assert_eq!(c, card),
            other => panic!("expected Card, got {other:?}"),
        }

        // Delete card.
        let resp = handle_request(&storage, Request::DeleteCard { id: ID_A });
        assert!(matches!(resp, Response::Deleted(_)));

        // Card is gone.
        let resp = handle_request(&storage, Request::GetCard { id: ID_A });
        assert!(matches!(resp, Response::Error(ProtocolError::NotFound)));
    }

    #[test]
    fn handle_request_tag_lifecycle() {
        let storage = SqliteStorage::open_in_memory().unwrap();

        // Create tag.
        let tag = Tag::first(TAG_ID, "Test".into(), None, ts(0));
        let resp = handle_request(&storage, Request::PushTagVersions(vec![tag.clone()]));
        assert!(matches!(resp, Response::Root(_)));

        // Get tag.
        let resp = handle_request(&storage, Request::GetTag { id: TAG_ID });
        match resp {
            Response::Tag(t) => assert_eq!(t, tag),
            other => panic!("expected Tag, got {other:?}"),
        }

        // List tags.
        let resp = handle_request(&storage, Request::ListTags);
        match resp {
            Response::Tags(tags) => {
                expect!["1"].assert_eq(&tags.len().to_string());
            }
            other => panic!("expected Tags, got {other:?}"),
        }

        // Delete tag.
        let resp = handle_request(&storage, Request::DeleteTag { id: TAG_ID });
        assert!(matches!(resp, Response::Deleted(_)));

        // Tag is gone.
        let resp = handle_request(&storage, Request::GetTag { id: TAG_ID });
        assert!(matches!(resp, Response::Error(ProtocolError::NotFound)));
    }

    #[test]
    fn handle_request_empty_push() {
        let storage = SqliteStorage::open_in_memory().unwrap();
        let resp = handle_request(&storage, Request::PushCardVersions(vec![]));
        assert!(matches!(
            resp,
            Response::Error(ProtocolError::PushFailed(PushError::EmptyChain))
        ));
    }

    #[test]
    fn handle_request_push_batch_card_and_tag() {
        use blazelist_protocol::PushItem;
        let storage = SqliteStorage::open_in_memory().unwrap();
        let card = Card::first(ID_A, "".into(), 1, vec![], false, ts(0), None);
        let tag = Tag::first(TAG_ID, "T".into(), None, ts(0));
        let resp = handle_request(
            &storage,
            Request::PushBatch(vec![PushItem::Cards(vec![card]), PushItem::Tags(vec![tag])]),
        );
        assert!(matches!(resp, Response::Root(_)));

        // Verify both exist.
        let resp = handle_request(&storage, Request::GetCard { id: ID_A });
        assert!(matches!(resp, Response::Card(_)));
        let resp = handle_request(&storage, Request::GetTag { id: TAG_ID });
        assert!(matches!(resp, Response::Tag(_)));
    }

    #[test]
    fn handle_request_push_batch_empty() {
        let storage = SqliteStorage::open_in_memory().unwrap();
        let resp = handle_request(&storage, Request::PushBatch(vec![]));
        assert!(matches!(resp, Response::Root(_)));
    }

    #[test]
    fn handle_request_push_batch_rollback() {
        use blazelist_protocol::PushItem;
        let storage = SqliteStorage::open_in_memory().unwrap();
        // Pre-create a card.
        let card = Card::first(ID_A, "".into(), 1, vec![], false, ts(0), None);
        handle_request(&storage, Request::PushCardVersions(vec![card.clone()]));

        // Batch: tag + stale card push.
        let tag = Tag::first(TAG_ID, "T".into(), None, ts(0));
        let stale = Card::first(ID_A, "".into(), 1, vec![], false, ts(100), None);
        let resp = handle_request(
            &storage,
            Request::PushBatch(vec![
                PushItem::Tags(vec![tag]),
                PushItem::Cards(vec![stale]),
            ]),
        );
        match resp {
            Response::Error(ProtocolError::BatchFailed { index, error }) => {
                assert_eq!(index, 1);
                assert!(matches!(
                    error,
                    BatchItemError::Push(PushError::CardAncestorMismatch(_))
                ));
            }
            other => panic!("expected BatchFailed, got {other:?}"),
        }

        // Tag should not exist (rolled back).
        let resp = handle_request(&storage, Request::GetTag { id: TAG_ID });
        assert!(matches!(resp, Response::Error(ProtocolError::NotFound)));
    }

    #[test]
    fn handle_request_already_deleted() {
        let storage = SqliteStorage::open_in_memory().unwrap();
        let card = Card::first(ID_A, "C".into(), 1, vec![], false, ts(0), None);
        handle_request(&storage, Request::PushCardVersions(vec![card]));
        handle_request(&storage, Request::DeleteCard { id: ID_A });
        let new_card = Card::first(ID_A, "C2".into(), 1, vec![], false, ts(1000), None);
        let resp = handle_request(&storage, Request::PushCardVersions(vec![new_card]));
        assert!(matches!(
            resp,
            Response::Error(ProtocolError::PushFailed(PushError::AlreadyDeleted))
        ));
    }

    #[test]
    fn handle_request_delete_tag_rejected_when_card_references_it() {
        use blazelist_protocol::PushItem;
        let storage = SqliteStorage::open_in_memory().unwrap();

        // Create a tag and a card referencing it
        let tag = Tag::first(TAG_ID, "T".into(), None, ts(0));
        handle_request(&storage, Request::PushTagVersions(vec![tag]));
        let card = Card::first(ID_A, "C".into(), 1, vec![TAG_ID], false, ts(0), None);
        handle_request(&storage, Request::PushCardVersions(vec![card.clone()]));

        // Direct delete should fail
        let resp = handle_request(&storage, Request::DeleteTag { id: TAG_ID });
        match resp {
            Response::Error(ProtocolError::PushFailed(PushError::OrphanedTagReference {
                tag_id,
                referencing_card_ids,
            })) => {
                assert_eq!(tag_id, TAG_ID);
                assert_eq!(referencing_card_ids, vec![ID_A]);
            }
            other => panic!("expected OrphanedTagReference, got {other:?}"),
        }

        // Batch with cleanup should succeed
        let updated = card.next("C".into(), 1, vec![], false, ts(1), None);
        let resp = handle_request(
            &storage,
            Request::PushBatch(vec![
                PushItem::Cards(vec![updated]),
                PushItem::DeleteTag { id: TAG_ID },
            ]),
        );
        assert!(matches!(resp, Response::Root(_)));

        // Tag should no longer exist
        let resp = handle_request(&storage, Request::GetTag { id: TAG_ID });
        assert!(matches!(resp, Response::Error(ProtocolError::NotFound)));
    }
}
