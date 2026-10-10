//! A tiny DNS client used only for self-tests: send one query to a resolver
//! and wait for any answer. Never used for real resolution.

use std::net::SocketAddr;
use std::time::Duration;

use hickory_proto::op::{Message, MessageType, OpCode, Query, ResponseCode};
use hickory_proto::rr::{Name, RData, RecordType};
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

/// Whether a running guardian answers at `server`: one `A` query for a fresh
/// `<something>.prueba.guardiana.hogar`, a name only a watching Guardiana answers (with the
/// loopback) and never forwards. Any other resolver on that port, or a stood-aside relay
/// (which says the name does not exist), counts as no.
///
/// `guardiana dns --apply` asks this of 127.0.0.1:53 before it points the machine there:
/// pointing it at a port nobody answers is a machine without names.
pub async fn guardian_answers(server: SocketAddr, timeout: Duration) -> bool {
    let now = guardiana_core::time::now_ms();
    let name = format!(
        "g{now}x{}.{}",
        std::process::id(),
        guardiana_core::SELF_CHECK_SUFFIX
    );
    let Ok(qname) = Name::from_ascii(&name) else {
        return false;
    };
    let id = u16::try_from(now.rem_euclid(0x1_0000)).unwrap_or(0x4742);
    let mut msg = Message::new(id, MessageType::Query, OpCode::Query);
    msg.metadata.recursion_desired = true;
    msg.add_query(Query::query(qname.clone(), RecordType::A));
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
    // Connected, so only datagrams from `server` are read.
    if sock.connect(server).await.is_err() || sock.send(&bytes).await.is_err() {
        return false;
    }
    let mut buf = [0u8; 4096];
    tokio::time::timeout(timeout, async {
        loop {
            match sock.recv(&mut buf).await {
                Ok(n) if is_guardian_answer(&buf[..n], id, &qname) => return true,
                // Something else (a late reply to another question): keep waiting.
                Ok(_) => {}
                // Nobody listening (the system says so at once on most machines).
                Err(_) => return false,
            }
        }
    })
    .await
    .unwrap_or(false)
}

/// Whether `reply` is a guardian's answer to the self-check question `id` for `qname`: a
/// response without error to that very question, with at least one address and only loopback
/// ones.
#[must_use]
pub fn is_guardian_answer(reply: &[u8], id: u16, qname: &Name) -> bool {
    let Ok(msg) = Message::from_vec(reply) else {
        return false;
    };
    msg.metadata.id == id
        && msg.metadata.message_type == MessageType::Response
        && msg.metadata.response_code == ResponseCode::NoError
        && msg
            .queries
            .first()
            .is_some_and(|q| q.name().eq_ignore_root(qname))
        && !msg.answers.is_empty()
        && msg
            .answers
            .iter()
            .all(|r| matches!(&r.data, RData::A(a) if a.0.is_loopback()))
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use std::net::Ipv4Addr;

    use hickory_proto::rr::rdata::A;
    use hickory_proto::rr::Record;

    use super::*;

    fn reply(id: u16, name: &str, code: ResponseCode, ips: &[Ipv4Addr]) -> Vec<u8> {
        let qname = Name::from_ascii(name).unwrap();
        let mut msg = Message::new(id, MessageType::Response, OpCode::Query);
        msg.metadata.response_code = code;
        msg.add_query(Query::query(qname.clone(), RecordType::A));
        for ip in ips {
            msg.add_answer(Record::from_rdata(qname.clone(), 1, RData::A(A::from(*ip))));
        }
        msg.to_vec().unwrap()
    }

    #[test]
    fn only_the_loopback_answer_to_this_question_counts() {
        let name = "g1x2.prueba.guardiana.hogar";
        let q = Name::from_ascii(name).unwrap();
        let lo = [Ipv4Addr::LOCALHOST];
        assert!(is_guardian_answer(
            &reply(7, name, ResponseCode::NoError, &lo),
            7,
            &q
        ));
        // Another question, another id, no address, a real address, "does not exist".
        assert!(!is_guardian_answer(
            &reply(7, "g9x9.prueba.guardiana.hogar", ResponseCode::NoError, &lo),
            7,
            &q
        ));
        assert!(!is_guardian_answer(
            &reply(8, name, ResponseCode::NoError, &lo),
            7,
            &q
        ));
        assert!(!is_guardian_answer(
            &reply(7, name, ResponseCode::NoError, &[]),
            7,
            &q
        ));
        assert!(!is_guardian_answer(
            &reply(
                7,
                name,
                ResponseCode::NoError,
                &[Ipv4Addr::new(93, 184, 216, 34)]
            ),
            7,
            &q
        ));
        assert!(!is_guardian_answer(
            &reply(7, name, ResponseCode::NXDomain, &[]),
            7,
            &q
        ));
        assert!(!is_guardian_answer(b"not dns", 7, &q));
    }
}
