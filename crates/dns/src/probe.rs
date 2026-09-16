//! A tiny DNS client used only for self-tests: send one query to a resolver
//! and wait for any answer. Never used for real resolution.

use std::net::SocketAddr;
use std::time::Duration;

use hickory_proto::op::{Message, MessageType, OpCode, Query};
use hickory_proto::rr::{Name, RecordType};
use tokio::net::UdpSocket;

/// Send an `A` query for `name` to `server` over UDP and wait up to `timeout`.
/// Returns `true` when an answer of any kind came back.
pub async fn probe(server: SocketAddr, name: &str, timeout: Duration) -> bool {
    let Ok(qname) = Name::from_ascii(name) else {
        return false;
    };
    let mut msg = Message::new(0x4741, MessageType::Query, OpCode::Query);
    msg.metadata.recursion_desired = true;
    msg.add_query(Query::query(qname, RecordType::A));
    let Ok(bytes) = msg.to_vec() else {
        return false;
    };
    let bind: SocketAddr = if server.is_ipv4() {
        "0.0.0.0:0".parse().unwrap_or(server)
    } else {
        "[::]:0".parse().unwrap_or(server)
    };
    let Ok(sock) = UdpSocket::bind(bind).await else {
        return false;
    };
    if sock.send_to(&bytes, server).await.is_err() {
        return false;
    }
    let mut buf = [0u8; 4096];
    tokio::time::timeout(timeout, sock.recv_from(&mut buf))
        .await
        .is_ok_and(|r| r.is_ok())
}
