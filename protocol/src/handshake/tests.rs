use crate::Version;

use super::*;

#[tokio::test]
async fn compatible_versions_succeed() {
    let (client_io, server_io) = tokio::io::duplex(4096);
    let (mut client_read, mut client_write) = tokio::io::split(client_io);
    let (mut server_read, mut server_write) = tokio::io::split(server_io);

    let client_version = Version::new(0, 1, 0);
    let server_version = Version::new(0, 1, 2);

    let (client_result, server_result) = tokio::join!(
        client_handshake(&mut client_write, &mut client_read, &client_version),
        server_handshake(&mut server_write, &mut server_read, &server_version),
    );

    client_result.unwrap();
    server_result.unwrap();
}

#[tokio::test]
async fn incompatible_versions_fail() {
    let (client_io, server_io) = tokio::io::duplex(4096);
    let (mut client_read, mut client_write) = tokio::io::split(client_io);
    let (mut server_read, mut server_write) = tokio::io::split(server_io);

    let client_version = Version::new(1, 0, 0);
    let server_version = Version::new(2, 0, 0);

    let (client_result, server_result) = tokio::join!(
        client_handshake(&mut client_write, &mut client_read, &client_version),
        server_handshake(&mut server_write, &mut server_read, &server_version),
    );

    match client_result {
        Err(HandshakeError::VersionMismatch { local, remote }) => {
            assert_eq!(local, client_version);
            assert_eq!(remote, server_version);
        }
        other => panic!("expected VersionMismatch, got {other:?}"),
    }

    match server_result {
        Err(HandshakeError::VersionMismatch { local, remote }) => {
            assert_eq!(local, server_version);
            assert_eq!(remote, client_version);
        }
        other => panic!("expected VersionMismatch, got {other:?}"),
    }
}

#[tokio::test]
async fn handshake_error_display() {
    let err = HandshakeError::VersionMismatch {
        local: Version::new(1, 0, 0),
        remote: Version::new(2, 0, 0),
    };
    assert_eq!(
        err.to_string(),
        "protocol version mismatch: local=1.0.0, remote=2.0.0"
    );
}

#[tokio::test]
async fn handshake_error_is_std_error() {
    let err: Box<dyn std::error::Error> = Box::new(HandshakeError::VersionMismatch {
        local: Version::new(1, 0, 0),
        remote: Version::new(2, 0, 0),
    });
    assert!(err.to_string().contains("version mismatch"));
}
