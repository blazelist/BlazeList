use super::*;
use crate::RootState;
use crate::{Request, Response};
use tokio::io::AsyncWriteExt;

#[tokio::test]
async fn round_trip_request_through_duplex() {
    let (mut client, mut server) = tokio::io::duplex(4096);
    let req = Request::GetRoot;
    write_message(&mut client, &req).await.unwrap();
    drop(client); // close write side so read sees EOF after the message
    let decoded: Request = read_message(&mut server).await.unwrap();
    assert_eq!(req, decoded);
}

#[tokio::test]
async fn round_trip_response_through_duplex() {
    let (mut client, mut server) = tokio::io::duplex(4096);
    let resp = Response::Root(RootState::empty());
    write_message(&mut client, &resp).await.unwrap();
    drop(client);
    let decoded: Response = read_message(&mut server).await.unwrap();
    assert_eq!(resp, decoded);
}

#[tokio::test]
async fn rejects_message_too_large() {
    let (mut writer, mut reader) = tokio::io::duplex(64);
    // Write a length header claiming a huge payload.
    writer.write_u32(MAX_MSG_SIZE + 1).await.unwrap();
    drop(writer);
    let result: Result<Request, _> = read_message(&mut reader).await;
    assert!(matches!(result, Err(WireError::MessageTooLarge)));
}

#[tokio::test]
async fn stream_closed_on_empty_read() {
    let (_writer, mut reader) = tokio::io::duplex(64);
    drop(_writer);
    let result: Result<Request, _> = read_message(&mut reader).await;
    assert!(matches!(result, Err(WireError::StreamClosed)));
}

#[tokio::test]
async fn write_rejects_message_too_large() {
    // A Vec<u8> larger than MAX_MSG_SIZE should be rejected during write.
    let oversized = vec![0u8; MAX_MSG_SIZE as usize + 1];
    let (mut writer, _reader) = tokio::io::duplex(64);
    let result = write_message(&mut writer, &oversized).await;
    assert!(matches!(result, Err(WireError::MessageTooLarge)));
}
