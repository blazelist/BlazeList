use tokio::io::{AsyncReadExt, AsyncWriteExt};

use super::constants::MAX_MSG_SIZE;
use super::error::WireError;

/// Read a length-prefixed postcard message from an async reader.
pub async fn read_message<T, R>(recv: &mut R) -> Result<T, WireError>
where
    T: serde::de::DeserializeOwned,
    R: AsyncReadExt + Unpin,
{
    let len = recv.read_u32().await.map_err(|_| WireError::StreamClosed)?;
    if len > MAX_MSG_SIZE {
        return Err(WireError::MessageTooLarge);
    }
    let mut buf = vec![0u8; len as usize];
    recv.read_exact(&mut buf)
        .await
        .map_err(|_| WireError::StreamClosed)?;
    postcard::from_bytes(&buf).map_err(|_| WireError::Deserialize)
}

/// Write a length-prefixed postcard message to an async writer.
pub async fn write_message<T, W>(send: &mut W, msg: &T) -> Result<(), WireError>
where
    T: serde::Serialize,
    W: AsyncWriteExt + Unpin,
{
    let bytes = postcard::to_allocvec(msg).map_err(|_| WireError::Serialize)?;
    if bytes.len() > MAX_MSG_SIZE as usize {
        return Err(WireError::MessageTooLarge);
    }
    let len = bytes.len() as u32;
    send.write_u32(len)
        .await
        .map_err(|_| WireError::WriteFailed)?;
    send.write_all(&bytes)
        .await
        .map_err(|_| WireError::WriteFailed)?;
    Ok(())
}
