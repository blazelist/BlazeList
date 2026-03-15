//! Length-prefixed postcard wire framing for WebTransport streams.
//!
//! Reimplements the protocol crate's wire format using web-sys
//! `ReadableStreamDefaultReader` / `WritableStreamDefaultWriter` instead
//! of tokio's `AsyncRead` / `AsyncWrite`.
//!
//! Wire format: `[4-byte big-endian u32 length][postcard payload]`

use js_sys::Uint8Array;
use serde::{Serialize, de::DeserializeOwned};
use wasm_bindgen::JsValue;
use wasm_bindgen_futures::JsFuture;
use web_sys::{ReadableStreamDefaultReader, WritableStreamDefaultWriter};

/// Maximum message size (16 MiB), matching `blazelist_protocol::wire::MAX_MSG_SIZE`.
pub const MAX_MSG_SIZE: u32 = 16 * 1024 * 1024;

/// Wire-level errors for the WebTransport framing layer.
#[derive(Debug)]
pub enum WireError {
    /// The remote side closed the stream before a complete message was read.
    StreamClosed,
    /// The incoming message exceeds [`MAX_MSG_SIZE`].
    MessageTooLarge,
    /// Postcard deserialization failed.
    Deserialize(String),
    /// Postcard serialization failed.
    Serialize(String),
    /// Writing to the stream failed.
    WriteFailed(String),
    /// A JavaScript error occurred during a read or write operation.
    JsError(String),
}

impl std::fmt::Display for WireError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WireError::StreamClosed => write!(f, "stream closed"),
            WireError::MessageTooLarge => write!(f, "message too large"),
            WireError::Deserialize(e) => write!(f, "deserialization error: {e}"),
            WireError::Serialize(e) => write!(f, "serialization error: {e}"),
            WireError::WriteFailed(e) => write!(f, "write failed: {e}"),
            WireError::JsError(e) => write!(f, "JS error: {e}"),
        }
    }
}

impl std::error::Error for WireError {}

impl From<JsValue> for WireError {
    fn from(val: JsValue) -> Self {
        let msg = val
            .as_string()
            .or_else(|| {
                js_sys::JSON::stringify(&val)
                    .ok()
                    .and_then(|s| s.as_string())
            })
            .unwrap_or_else(|| format!("{val:?}"));
        WireError::JsError(msg)
    }
}

/// Buffered reader over a `ReadableStreamDefaultReader`.
///
/// WebTransport streams may deliver data in arbitrarily-sized chunks that
/// don't align with message boundaries. This wrapper accumulates leftover
/// bytes so that consecutive `read_exact` calls (e.g. length prefix then
/// payload) work correctly even when the data arrives in a single chunk.
pub struct BufReader {
    reader: ReadableStreamDefaultReader,
    buffer: Vec<u8>,
}

impl BufReader {
    /// Wrap a `ReadableStreamDefaultReader` in a buffered reader.
    pub fn new(reader: ReadableStreamDefaultReader) -> Self {
        Self {
            reader,
            buffer: Vec::new(),
        }
    }

    /// Read exactly `len` bytes, pulling from the internal buffer first
    /// and then from the underlying stream as needed.
    async fn read_exact(&mut self, len: usize) -> Result<Vec<u8>, WireError> {
        while self.buffer.len() < len {
            let raw_result = JsFuture::from(self.reader.read())
                .await
                .map_err(WireError::from)?;

            let done_val =
                js_sys::Reflect::get(&raw_result, &"done".into()).unwrap_or(JsValue::UNDEFINED);
            let value_val =
                js_sys::Reflect::get(&raw_result, &"value".into()).unwrap_or(JsValue::UNDEFINED);

            let done = done_val.as_bool().unwrap_or(false);
            if done {
                tracing::warn!(
                    buffered = self.buffer.len(),
                    expected = len,
                    "read_exact: stream done prematurely",
                );
                return Err(WireError::StreamClosed);
            }

            if value_val.is_undefined() || value_val.is_null() {
                return Err(WireError::StreamClosed);
            }

            let chunk = Uint8Array::new(&value_val);
            let chunk_len = chunk.length() as usize;

            if chunk_len == 0 {
                return Err(WireError::StreamClosed);
            }

            let mut tmp = vec![0u8; chunk_len];
            chunk.copy_to(&mut tmp);
            self.buffer.extend_from_slice(&tmp);
        }

        let result = self.buffer[..len].to_vec();
        self.buffer = self.buffer.split_off(len);
        Ok(result)
    }
}

/// Read a length-prefixed postcard message from a WebTransport stream.
///
/// Reads a 4-byte big-endian length prefix, then reads that many bytes
/// of payload and deserializes with postcard.
pub async fn read_message<T: DeserializeOwned>(reader: &mut BufReader) -> Result<T, WireError> {
    // Read the 4-byte length prefix.
    let len_bytes = reader.read_exact(4).await?;
    let len = u32::from_be_bytes([len_bytes[0], len_bytes[1], len_bytes[2], len_bytes[3]]);

    if len > MAX_MSG_SIZE {
        return Err(WireError::MessageTooLarge);
    }

    // Read the payload.
    let payload = reader.read_exact(len as usize).await?;

    postcard::from_bytes(&payload).map_err(|e| WireError::Deserialize(e.to_string()))
}

/// Write a length-prefixed postcard message to a WebTransport stream.
///
/// Serializes the message with postcard, prepends a 4-byte big-endian
/// length prefix, and writes the combined buffer in a single chunk.
pub async fn write_message<T: Serialize>(
    writer: &WritableStreamDefaultWriter,
    msg: &T,
) -> Result<(), WireError> {
    let payload = postcard::to_allocvec(msg).map_err(|e| WireError::Serialize(e.to_string()))?;
    let len = payload.len() as u32;

    // Build a single buffer with the length prefix and payload.
    let mut buf = Vec::with_capacity(4 + payload.len());
    buf.extend_from_slice(&len.to_be_bytes());
    buf.extend_from_slice(&payload);

    let uint8array = Uint8Array::from(buf.as_slice());

    JsFuture::from(writer.write_with_chunk(&uint8array))
        .await
        .map_err(|e| WireError::WriteFailed(format!("{e:?}")))?;

    Ok(())
}
