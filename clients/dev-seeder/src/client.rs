//! Minimal QUIC client for pushing seed data to a BlazeList server.

use std::net::SocketAddr;
use std::sync::Arc;

use blazelist_protocol::handshake::client_handshake;
use blazelist_protocol::wire::{read_message, write_message};
use blazelist_protocol::{Entity, Version};
use blazelist_protocol::{PushItem, Request, Response};

use crate::seed::SeedData;
use rustls::pki_types::CertificateDer;

const CLIENT_VERSION: Version = blazelist_protocol::PROTOCOL_VERSION;

/// A QUIC client for pushing seed data.
pub struct Client {
    connection: quinn::Connection,
}

impl Client {
    /// Connect to a BlazeList server at the given address.
    pub async fn connect(addr: SocketAddr) -> Result<Self, Box<dyn std::error::Error>> {
        let mut client_crypto = rustls::ClientConfig::builder()
            .dangerous()
            .with_custom_certificate_verifier(Arc::new(SkipServerVerification))
            .with_no_client_auth();
        client_crypto.alpn_protocols = vec![b"blazelist/0".to_vec()];

        let mut transport = quinn::TransportConfig::default();
        transport.keep_alive_interval(Some(std::time::Duration::from_secs(5)));
        transport.max_idle_timeout(Some(
            quinn::IdleTimeout::try_from(std::time::Duration::from_secs(300)).unwrap(),
        ));

        let mut client_config = quinn::ClientConfig::new(Arc::new(
            quinn::crypto::rustls::QuicClientConfig::try_from(client_crypto)?,
        ));
        client_config.transport_config(Arc::new(transport));

        let mut endpoint = quinn::Endpoint::client("0.0.0.0:0".parse()?)?;
        endpoint.set_default_client_config(client_config);

        let connection = endpoint.connect(addr, "localhost")?.await?;

        // Version handshake.
        let (mut send, mut recv) = connection
            .open_bi()
            .await
            .map_err(|e| format!("version handshake: {e}"))?;
        client_handshake(&mut send, &mut recv, &CLIENT_VERSION).await?;

        Ok(Self { connection })
    }

    /// Push seed data in two phases: create all entities first, then delete
    /// the doomed ones in a separate batch so the server persists the
    /// `DeletedEntity` records.
    pub async fn push_seed_data(&self, data: &SeedData) -> Result<(), Box<dyn std::error::Error>> {
        // Phase 1: create every entity (live + doomed).
        let mut create_items: Vec<PushItem> = Vec::new();

        for chain in &data.tag_chains {
            create_items.push(PushItem::Tags(chain.clone()));
        }
        for chain in &data.deleted_tag_chains {
            create_items.push(PushItem::Tags(chain.clone()));
        }
        for chain in &data.card_chains {
            create_items.push(PushItem::Cards(chain.clone()));
        }
        for chain in &data.deleted_card_chains {
            create_items.push(PushItem::Cards(chain.clone()));
        }

        self.send_request(Request::PushBatch(create_items)).await?;
        println!(
            "  Phase 1: created {} tags + {} cards (including doomed).",
            data.tag_chains.len() + data.deleted_tag_chains.len(),
            data.card_chains.len() + data.deleted_card_chains.len(),
        );

        // Phase 2: delete doomed entities in a separate batch so the server
        // records each as a DeletedEntity.
        let has_deletions =
            !data.deleted_tag_chains.is_empty() || !data.deleted_card_chains.is_empty();
        if has_deletions {
            let mut delete_items: Vec<PushItem> = Vec::new();

            for chain in &data.deleted_tag_chains {
                let id = chain.first().unwrap().id();
                delete_items.push(PushItem::DeleteTag { id });
            }
            for chain in &data.deleted_card_chains {
                let id = chain.first().unwrap().id();
                delete_items.push(PushItem::DeleteCard { id });
            }

            self.send_request(Request::PushBatch(delete_items)).await?;
            println!(
                "  Phase 2: deleted {} tags + {} cards.",
                data.deleted_tag_chains.len(),
                data.deleted_card_chains.len(),
            );
        }

        // Phase 3: Extra operations — each pushed individually to create
        // distinct sequence entries (120 ops for a rich history).
        if !data.extra_ops.is_empty() {
            for (i, batch) in data.extra_ops.iter().enumerate() {
                self.send_request(Request::PushBatch(batch.clone())).await?;
                if (i + 1) % 30 == 0 || i + 1 == data.extra_ops.len() {
                    println!(
                        "  Phase 3: pushed {}/{} extra operations.",
                        i + 1,
                        data.extra_ops.len(),
                    );
                }
            }
        }

        Ok(())
    }

    /// Send a single request and check the response.
    async fn send_request(&self, req: Request) -> Result<(), Box<dyn std::error::Error>> {
        let (mut send, mut recv) = self.connection.open_bi().await?;
        write_message(&mut send, &req).await?;
        send.finish()?;
        let resp: Response = read_message(&mut recv).await?;
        match resp {
            Response::Ok | Response::Root(_) | Response::Deleted(_) => Ok(()),
            Response::Error(e) => Err(e.into()),
            other => Err(format!("unexpected response: {other:?}").into()),
        }
    }
}

/// TLS certificate verifier that accepts any certificate (development only).
#[derive(Debug)]
struct SkipServerVerification;

impl rustls::client::danger::ServerCertVerifier for SkipServerVerification {
    fn verify_server_cert(
        &self,
        _end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &rustls::pki_types::ServerName<'_>,
        _ocsp_response: &[u8],
        _now: rustls::pki_types::UnixTime,
    ) -> Result<rustls::client::danger::ServerCertVerified, rustls::Error> {
        Ok(rustls::client::danger::ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        _message: &[u8],
        _cert: &CertificateDer<'_>,
        _dss: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
    }

    fn verify_tls13_signature(
        &self,
        _message: &[u8],
        _cert: &CertificateDer<'_>,
        _dss: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
    }

    fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
        vec![
            rustls::SignatureScheme::RSA_PKCS1_SHA256,
            rustls::SignatureScheme::RSA_PKCS1_SHA384,
            rustls::SignatureScheme::RSA_PKCS1_SHA512,
            rustls::SignatureScheme::ECDSA_NISTP256_SHA256,
            rustls::SignatureScheme::ECDSA_NISTP384_SHA384,
            rustls::SignatureScheme::ECDSA_NISTP521_SHA512,
            rustls::SignatureScheme::ED25519,
            rustls::SignatureScheme::RSA_PSS_SHA256,
            rustls::SignatureScheme::RSA_PSS_SHA384,
            rustls::SignatureScheme::RSA_PSS_SHA512,
        ]
    }
}
