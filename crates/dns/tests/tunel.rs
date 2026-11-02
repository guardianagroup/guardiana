//! The tunnel's resolver (1.0.54): while a `guardiana_core::tunel_dns::Desvio` lives, every forwarded name goes to it
//! and never to the network's resolvers, not even from their cache; when it goes away, back to them. A test binary of
//! its own: the value is the whole process's, and the other tests must not see it.
#![allow(clippy::unwrap_used, clippy::expect_used, missing_docs)]

use std::net::{Ipv4Addr, SocketAddr};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use guardiana_core::tunel_dns::Desvio;
use guardiana_dns::{start, Config, Decision, Outcome, Policy, Query, RecordType};
use hickory_proto::op::{Message, MessageType, Metadata, OpCode, Query as DnsQuery, ResponseCode};
use hickory_proto::rr::rdata::A;
use hickory_proto::rr::{Name, RData, Record};
use hickory_resolver::net::runtime::Time;
use hickory_server::server::{Request, RequestHandler, ResponseHandler, ResponseInfo, Server};
use hickory_server::zone_handler::MessageResponseBuilder;
use tokio::net::{TcpListener, UdpSocket};

/// A resolver that answers every A question with `ip`, and counts the questions.
struct Fijo {
    ip: Ipv4Addr,
    preguntas: Arc<AtomicUsize>,
}

#[async_trait::async_trait]
impl RequestHandler for Fijo {
    async fn handle_request<R: ResponseHandler, T: Time>(
        &self,
        request: &Request,
        mut response: R,
    ) -> ResponseInfo {
        self.preguntas.fetch_add(1, Ordering::SeqCst);
        let q = &request.queries.queries()[0];
        let name: Name = q.original().name().clone();
        let mut metadata = Metadata::response_from_request(&request.metadata);
        metadata.recursion_available = true;
        let answers = if q.query_type() == RecordType::A {
            vec![Record::from_rdata(name, 300, RData::A(A::from(self.ip)))]
        } else {
            Vec::new()
        };
        let msg = MessageResponseBuilder::from_message_request(request).build(
            metadata,
            answers.iter(),
            std::iter::empty::<&Record>(),
            std::iter::empty::<&Record>(),
            std::iter::empty::<&Record>(),
        );
        response.send_response(msg).await.unwrap()
    }
}

async fn fijo(ip: Ipv4Addr) -> (SocketAddr, Arc<AtomicUsize>, Server<Fijo>) {
    let preguntas = Arc::new(AtomicUsize::new(0));
    let mut server = Server::new(Fijo {
        ip,
        preguntas: preguntas.clone(),
    });
    let udp = UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let addr = udp.local_addr().unwrap();
    server.register_socket(udp);
    let tcp = TcpListener::bind(addr).await.unwrap();
    server.register_listener(tcp, Duration::from_secs(5), 4096);
    (addr, preguntas, server)
}

/// Lets everything through and keeps nothing.
struct Todo;

impl Policy for Todo {
    fn decide(&self, _: &Query) -> Decision {
        Decision::Forward
    }
    fn record(&self, _: &Query, _: Outcome) {}
}

async fn pregunta(server: SocketAddr, name: &str) -> Message {
    let mut msg = Message::new(4242, MessageType::Query, OpCode::Query);
    msg.metadata.recursion_desired = true;
    msg.add_query(DnsQuery::query(
        Name::from_ascii(name).unwrap(),
        RecordType::A,
    ));
    let sock = UdpSocket::bind("127.0.0.1:0").await.unwrap();
    sock.send_to(&msg.to_vec().unwrap(), server).await.unwrap();
    let mut buf = vec![0u8; 4096];
    let (n, _) = tokio::time::timeout(Duration::from_secs(10), sock.recv_from(&mut buf))
        .await
        .expect("sin respuesta")
        .unwrap();
    Message::from_vec(&buf[..n]).unwrap()
}

fn direcciones(msg: &Message) -> Vec<Ipv4Addr> {
    msg.answers
        .iter()
        .filter_map(|r| match &r.data {
            RData::A(a) => Some(a.0),
            _ => None,
        })
        .collect()
}

/// One test for the whole binary, in order: the value is the process's.
#[tokio::test]
async fn con_la_vpn_los_nombres_van_solo_al_resolutor_del_tunel() {
    let red_ip = Ipv4Addr::new(1, 1, 1, 1);
    let tunel_ip = Ipv4Addr::new(2, 2, 2, 2);
    let (red, en_la_red, _s1) = fijo(red_ip).await;
    let (tunel, en_el_tunel, _s2) = fijo(tunel_ip).await;
    let mut cfg = Config::local(vec![red]);
    cfg.listen = vec!["127.0.0.1:0".parse().unwrap()];
    cfg.upstream_timeout = Duration::from_millis(300);
    let g = start(cfg, Todo).await.unwrap();
    let yo = g.udp_addrs[0];

    // without the VPN: the network's resolver
    assert_eq!(direcciones(&pregunta(yo, "a.test").await), vec![red_ip]);
    let antes = en_la_red.load(Ordering::SeqCst);
    assert!(antes >= 1);

    // with it: only the tunnel's, even for a name the network's cache already holds
    let desvio = Desvio::pon(tunel);
    assert_eq!(direcciones(&pregunta(yo, "a.test").await), vec![tunel_ip]);
    assert_eq!(direcciones(&pregunta(yo, "b.test").await), vec![tunel_ip]);
    assert!(en_el_tunel.load(Ordering::SeqCst) >= 2);
    assert_eq!(en_la_red.load(Ordering::SeqCst), antes);

    // when it goes away: the network's again
    drop(desvio);
    assert_eq!(direcciones(&pregunta(yo, "c.test").await), vec![red_ip]);
    let despues = en_la_red.load(Ordering::SeqCst);
    assert!(despues > antes);

    // a tunnel's resolver that does not answer: the name fails, it never goes to the network's instead
    let muerto = Desvio::pon("127.0.0.1:1".parse().unwrap());
    let msg = pregunta(yo, "d.test").await;
    assert_eq!(msg.metadata.response_code, ResponseCode::ServFail);
    assert_eq!(en_la_red.load(Ordering::SeqCst), despues);
    drop(muerto);
    g.shutdown();
}
