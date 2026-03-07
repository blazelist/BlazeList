use crate::Version;
use crate::is_compatible;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use super::error::HandshakeError;
use crate::wire::{read_message, write_message};
use crate::{VersionCheck, VersionResult};

/// Perform the client side of the version handshake.
///
/// Sends a [`VersionCheck`] with the given `client_version`, reads the
/// server's [`VersionResult`], and returns `Ok(())` if the versions are
/// compatible.
pub async fn client_handshake<S, R>(
    send: &mut S,
    recv: &mut R,
    client_version: &Version,
) -> Result<(), HandshakeError>
where
    S: AsyncWriteExt + Unpin,
    R: AsyncReadExt + Unpin,
{
    let check = VersionCheck {
        version: client_version.clone(),
    };
    write_message(send, &check).await?;
    let result: VersionResult = read_message(recv).await?;
    match result {
        VersionResult::Ok => Ok(()),
        VersionResult::Mismatch { server_version } => Err(HandshakeError::VersionMismatch {
            local: client_version.clone(),
            remote: server_version,
        }),
    }
}

/// Perform the server side of the version handshake.
///
/// Reads a [`VersionCheck`] from the client, checks compatibility with
/// `server_version`, and writes the [`VersionResult`]. Returns `Ok(())`
/// if the versions are compatible.
pub async fn server_handshake<S, R>(
    send: &mut S,
    recv: &mut R,
    server_version: &Version,
) -> Result<(), HandshakeError>
where
    S: AsyncWriteExt + Unpin,
    R: AsyncReadExt + Unpin,
{
    let check: VersionCheck = read_message(recv).await?;
    let result = if is_compatible(&check.version, server_version) {
        VersionResult::Ok
    } else {
        VersionResult::Mismatch {
            server_version: server_version.clone(),
        }
    };
    write_message(send, &result).await?;
    if !matches!(result, VersionResult::Ok) {
        return Err(HandshakeError::VersionMismatch {
            local: server_version.clone(),
            remote: check.version,
        });
    }
    Ok(())
}
