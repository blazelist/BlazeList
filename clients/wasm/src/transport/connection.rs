//! WebTransport connection management via web-sys bindings.
//!
//! Wraps the browser's WebTransport API to provide a Rust-friendly interface
//! for creating connections and opening bidirectional streams.

use js_sys::Uint8Array;
use wasm_bindgen::JsCast;
use wasm_bindgen::JsValue;
use wasm_bindgen_futures::JsFuture;
use web_sys::{
    ReadableStreamDefaultReader, WebTransport, WebTransportBidirectionalStream, WebTransportHash,
    WebTransportOptions, WritableStreamDefaultWriter,
};

/// Errors arising from the WebTransport connection layer.
#[derive(Debug)]
pub enum ConnectionError {
    /// A JavaScript exception occurred.
    JsError(String),
    /// The connection was closed or failed.
    ConnectionFailed(String),
}

impl std::fmt::Display for ConnectionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConnectionError::JsError(msg) => write!(f, "JS error: {msg}"),
            ConnectionError::ConnectionFailed(msg) => write!(f, "connection failed: {msg}"),
        }
    }
}

impl std::error::Error for ConnectionError {}

impl From<JsValue> for ConnectionError {
    fn from(val: JsValue) -> Self {
        let msg = val
            .as_string()
            .or_else(|| {
                js_sys::JSON::stringify(&val)
                    .ok()
                    .and_then(|s| s.as_string())
            })
            .unwrap_or_else(|| format!("{val:?}"));
        ConnectionError::JsError(msg)
    }
}

/// A WebTransport connection to a BlazeList server.
///
/// Wraps a `web_sys::WebTransport` instance and provides methods for
/// opening bidirectional streams.
pub struct WtConnection {
    transport: WebTransport,
}

impl WtConnection {
    /// Create a new WebTransport connection to the given URL.
    ///
    /// `cert_hash` is the SHA-256 digest of the server's self-signed
    /// certificate, passed via `serverCertificateHashes`.
    pub async fn connect(url: &str, cert_hash: &[u8]) -> Result<Self, ConnectionError> {
        let options = WebTransportOptions::new();

        let wt_hash = WebTransportHash::new();
        wt_hash.set_algorithm("sha-256");
        let hash_array = Uint8Array::from(cert_hash);
        wt_hash.set_value_u8_array(&hash_array);

        options.set_server_certificate_hashes(&[wt_hash]);

        let transport =
            WebTransport::new_with_options(url, &options).map_err(ConnectionError::from)?;

        // Wait for the connection to be established.
        JsFuture::from(transport.ready()).await.map_err(|e| {
            ConnectionError::ConnectionFailed(
                e.as_string()
                    .unwrap_or_else(|| "ready promise rejected".to_string()),
            )
        })?;

        Ok(Self { transport })
    }

    /// Open a new bidirectional stream.
    ///
    /// Returns a `(writer, reader)` pair for sending and receiving data
    /// on the stream.
    pub async fn open_bi(
        &self,
    ) -> Result<(WritableStreamDefaultWriter, ReadableStreamDefaultReader), ConnectionError> {
        let bi_stream: WebTransportBidirectionalStream =
            JsFuture::from(self.transport.create_bidirectional_stream())
                .await
                .map_err(ConnectionError::from)?
                .unchecked_into();

        let writable = bi_stream.writable();
        let readable = bi_stream.readable();

        let writer = writable.get_writer().map_err(ConnectionError::from)?;

        // `get_reader()` returns a generic Object; cast to the concrete reader type.
        let reader: ReadableStreamDefaultReader = readable.get_reader().unchecked_into();

        Ok((writer, reader))
    }
}
