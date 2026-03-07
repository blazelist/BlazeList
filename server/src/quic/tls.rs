//! TLS configuration helpers.

use std::sync::Arc;

use quinn::ServerConfig;
use rustls::pki_types::{CertificateDer, PrivatePkcs8KeyDer};

/// Generated self-signed certificate material.
pub struct SelfSignedCert {
    /// DER-encoded certificate bytes.
    pub cert_der: Vec<u8>,
    /// DER-encoded PKCS#8 private key bytes.
    pub key_der: Vec<u8>,
}

/// Generate an ECDSA P-256 self-signed certificate valid for 14 days and
/// return a `quinn::ServerConfig` along with the raw certificate material.
///
/// WebTransport's `serverCertificateHashes` requires:
/// - ECDSA P-256 (not RSA)
/// - Validity period ≤ 14 days
pub fn self_signed_server_config()
-> Result<(ServerConfig, SelfSignedCert), Box<dyn std::error::Error>> {
    let key_pair = rcgen::KeyPair::generate_for(&rcgen::PKCS_ECDSA_P256_SHA256)?;

    let mut params = rcgen::CertificateParams::new(vec!["localhost".into()])?;
    params.not_before = time::OffsetDateTime::now_utc();
    params.not_after = time::OffsetDateTime::now_utc() + time::Duration::days(14);

    let cert = params.self_signed(&key_pair)?;
    let cert_der = CertificateDer::from(cert.der().to_vec());
    let key_der_bytes = key_pair.serialize_der();
    let key_der = PrivatePkcs8KeyDer::from(key_der_bytes.clone());

    let der_bytes = cert_der.to_vec();

    let mut server_crypto = rustls::ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(vec![cert_der], key_der.into())?;
    server_crypto.alpn_protocols = vec![b"blazelist/0".to_vec()];

    let mut transport = quinn::TransportConfig::default();
    transport.max_idle_timeout(Some(
        quinn::IdleTimeout::try_from(std::time::Duration::from_secs(300))
            .expect("idle timeout value should be valid"),
    ));

    let mut server_config = ServerConfig::with_crypto(Arc::new(
        quinn::crypto::rustls::QuicServerConfig::try_from(server_crypto)?,
    ));
    server_config.transport_config(Arc::new(transport));

    let material = SelfSignedCert {
        cert_der: der_bytes,
        key_der: key_der_bytes,
    };

    Ok((server_config, material))
}

/// Create a `quinn::ClientConfig` that trusts a specific self-signed
/// certificate (DER bytes).
pub fn client_config_for_cert(
    cert_der: &[u8],
) -> Result<quinn::ClientConfig, Box<dyn std::error::Error>> {
    let mut roots = rustls::RootCertStore::empty();
    roots.add(CertificateDer::from(cert_der.to_vec()))?;

    let mut client_crypto = rustls::ClientConfig::builder()
        .with_root_certificates(roots)
        .with_no_client_auth();
    client_crypto.alpn_protocols = vec![b"blazelist/0".to_vec()];

    let client_config = quinn::ClientConfig::new(Arc::new(
        quinn::crypto::rustls::QuicClientConfig::try_from(client_crypto)?,
    ));

    Ok(client_config)
}
