//! Integration tests (brief §11): resolve, cache, block, canary, checker,
//! upstream down, TCP, private-address check.
#![allow(clippy::unwrap_used, clippy::expect_used, missing_docs)]

use std::net::{Ipv4Addr, Ipv6Addr, SocketAddr};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use guardiana_dns::{
    start, BlockMode, Config, Decision, Error, Outcome, Policy, Query, RecordType, Running,
    CANARY_NAME, CHECKER_NAME,
};
use hickory_proto::op::{Message, MessageType, Metadata, OpCode, Query as DnsQuery, ResponseCode};
use hickory_proto::rr::rdata::A;
use hickory_proto::rr::{Name, RData, Record};
use hickory_resolver::net::runtime::Time;
use hickory_server::server::{Request, RequestHandler, ResponseHandler, ResponseInfo, Server};
use hickory_server::zone_handler::MessageResponseBuilder;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream, UdpSocket};

// ----- a fake upstream resolver ---------------------------------------------

/// `a.test` → 1.2.3.4 (TTL 300); `nx.test` → NXDOMAIN; anything else → empty NOERROR.
struct Upstream {
    hits: Arc<AtomicUsize>,
}

#[async_trait::async_trait]
impl RequestHandler for Upstream {
    async fn handle_request<R: ResponseHandler, T: Time>(
        &self,
        request: &Request,
        mut response: R,
    ) -> ResponseInfo {
        self.hits.fetch_add(1, Ordering::SeqCst);
        let q = &request.queries.queries()[0];
        let name: Name = q.original().name().clone();
        let text = name.to_ascii().to_lowercase();
        let mut metadata = Metadata::response_from_request(&request.metadata);
        metadata.recursion_available = true;
        let answers = if text == "a.test." && q.query_type() == RecordType::A {
            vec![Record::from_rdata(
                name.clone(),
                300,
                RData::A(A::from(Ipv4Addr::new(1, 2, 3, 4))),
            )]
        } else {
            Vec::new()
        };
        if text == "nx.test." {
            metadata.response_code = ResponseCode::NXDomain;
        }
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

async fn fake_upstream() -> (SocketAddr, Arc<AtomicUsize>, Server<Upstream>) {
    let hits = Arc::new(AtomicUsize::new(0));
    let mut server = Server::new(Upstream { hits: hits.clone() });
    let udp = UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let addr = udp.local_addr().unwrap();
    server.register_socket(udp);
    let tcp = TcpListener::bind(addr).await.unwrap();
    server.register_listener(tcp, Duration::from_secs(5), 4096);
    (addr, hits, server)
}

// ----- a recording policy ---------------------------------------------------

#[derive(Clone, Default)]
struct Recorder {
    blocked: Arc<Mutex<Vec<String>>>,
    seen: Arc<Mutex<Vec<(Query, Outcome)>>>,
}

impl Recorder {
    fn block(&self, name: &str) {
        self.blocked.lock().unwrap().push(name.to_owned());
    }
    fn seen(&self) -> Vec<(Query, Outcome)> {
        self.seen.lock().unwrap().clone()
    }
}

impl Policy for Recorder {
    fn decide(&self, query: &Query) -> Decision {
        if self.blocked.lock().unwrap().contains(&query.name) {
            Decision::Block { rule_id: Some(7) }
        } else {
            Decision::Forward
        }
    }
    fn record(&self, query: &Query, outcome: Outcome) {
        self.seen.lock().unwrap().push((query.clone(), outcome));
    }
}

// ----- helpers --------------------------------------------------------------

async fn guardiana(
    upstream: SocketAddr,
    policy: Recorder,
    tweak: impl FnOnce(&mut Config),
) -> Running {
    let mut cfg = Config::local(vec![upstream]);
    cfg.listen = vec!["127.0.0.1:0".parse().unwrap()];
    tweak(&mut cfg);
    start(cfg, policy).await.unwrap()
}

fn question(name: &str, qtype: RecordType) -> Vec<u8> {
    let mut msg = Message::new(4242, MessageType::Query, OpCode::Query);
    msg.metadata.recursion_desired = true;
    msg.add_query(DnsQuery::query(Name::from_ascii(name).unwrap(), qtype));
    msg.to_vec().unwrap()
}

async fn ask_udp(server: SocketAddr, name: &str, qtype: RecordType) -> Message {
    let sock = UdpSocket::bind("127.0.0.1:0").await.unwrap();
    sock.send_to(&question(name, qtype), server).await.unwrap();
    let mut buf = vec![0u8; 4096];
    let (n, _) = tokio::time::timeout(Duration::from_secs(10), sock.recv_from(&mut buf))
        .await
        .expect("timeout waiting for answer")
        .unwrap();
    Message::from_vec(&buf[..n]).unwrap()
}

async fn ask_tcp(server: SocketAddr, name: &str, qtype: RecordType) -> Message {
    let mut stream = TcpStream::connect(server).await.unwrap();
    let q = question(name, qtype);
    stream
        .write_all(&u16::try_from(q.len()).unwrap().to_be_bytes())
        .await
        .unwrap();
    stream.write_all(&q).await.unwrap();
    let mut len = [0u8; 2];
    stream.read_exact(&mut len).await.unwrap();
    let mut buf = vec![0u8; usize::from(u16::from_be_bytes(len))];
    stream.read_exact(&mut buf).await.unwrap();
    Message::from_vec(&buf).unwrap()
}

fn a_records(msg: &Message) -> Vec<Ipv4Addr> {
    msg.answers
        .iter()
        .filter_map(|r| match &r.data {
            RData::A(a) => Some(a.0),
            _ => None,
        })
        .collect()
}

fn aaaa_records(msg: &Message) -> Vec<Ipv6Addr> {
    msg.answers
        .iter()
        .filter_map(|r| match &r.data {
            RData::AAAA(a) => Some(a.0),
            _ => None,
        })
        .collect()
}

// ----- tests ----------------------------------------------------------------

#[tokio::test]
async fn forwards_and_serves_second_query_from_cache() {
    let (up, hits, _upstream) = fake_upstream().await;
    let policy = Recorder::default();
    let g = guardiana(up, policy.clone(), |_| {}).await;

    let first = ask_udp(g.udp_addrs[0], "A.Test", RecordType::A).await;
    assert_eq!(first.metadata.response_code, ResponseCode::NoError);
    assert_eq!(a_records(&first), vec![Ipv4Addr::new(1, 2, 3, 4)]);
    assert_eq!(hits.load(Ordering::SeqCst), 1);

    let second = ask_udp(g.udp_addrs[0], "a.test", RecordType::A).await;
    assert_eq!(a_records(&second), vec![Ipv4Addr::new(1, 2, 3, 4)]);
    assert_eq!(
        hits.load(Ordering::SeqCst),
        1,
        "second answer must come from cache"
    );

    let seen = policy.seen();
    assert_eq!(seen.len(), 2);
    assert_eq!(
        seen[0].0.name, "a.test",
        "name is lowercased without trailing dot"
    );
    assert_eq!(seen[0].0.qtype, RecordType::A);
    assert!(seen[0].0.ts > 0);
    assert_eq!(
        seen[1].1,
        Outcome::Forwarded {
            rcode: ResponseCode::NoError
        }
    );
}

#[tokio::test]
async fn nxdomain_from_upstream_passes_through() {
    let (up, _hits, _upstream) = fake_upstream().await;
    let policy = Recorder::default();
    let g = guardiana(up, policy.clone(), |_| {}).await;
    let msg = ask_udp(g.udp_addrs[0], "nx.test", RecordType::A).await;
    assert_eq!(msg.metadata.response_code, ResponseCode::NXDomain);
    assert_eq!(
        policy.seen()[0].1,
        Outcome::Forwarded {
            rcode: ResponseCode::NXDomain
        }
    );
}

#[tokio::test]
async fn blocked_name_gets_nxdomain_without_touching_upstream() {
    let (up, hits, _upstream) = fake_upstream().await;
    let policy = Recorder::default();
    policy.block("ads.test");
    let g = guardiana(up, policy.clone(), |_| {}).await;
    let msg = ask_udp(g.udp_addrs[0], "ads.test", RecordType::A).await;
    assert_eq!(msg.metadata.response_code, ResponseCode::NXDomain);
    assert!(msg.answers.is_empty());
    assert_eq!(hits.load(Ordering::SeqCst), 0);
    assert_eq!(policy.seen()[0].1, Outcome::Blocked { rule_id: Some(7) });
}

#[tokio::test]
async fn zero_ip_mode_answers_unspecified_addresses() {
    let (up, _hits, _upstream) = fake_upstream().await;
    let policy = Recorder::default();
    policy.block("ads.test");
    let g = guardiana(up, policy, |c| c.block_mode = BlockMode::ZeroIp).await;
    let a = ask_udp(g.udp_addrs[0], "ads.test", RecordType::A).await;
    assert_eq!(a.metadata.response_code, ResponseCode::NoError);
    assert_eq!(a_records(&a), vec![Ipv4Addr::UNSPECIFIED]);
    let aaaa = ask_udp(g.udp_addrs[0], "ads.test", RecordType::AAAA).await;
    assert_eq!(aaaa_records(&aaaa), vec![Ipv6Addr::UNSPECIFIED]);
    let txt = ask_udp(g.udp_addrs[0], "ads.test", RecordType::TXT).await;
    assert_eq!(txt.metadata.response_code, ResponseCode::NoError);
    assert!(txt.answers.is_empty());
}

#[tokio::test]
async fn canary_is_nxdomain_only_when_enabled() {
    let (up, hits, _upstream) = fake_upstream().await;
    let policy = Recorder::default();
    let g = guardiana(up, policy.clone(), |c| c.canary_enabled = true).await;
    let msg = ask_udp(g.udp_addrs[0], CANARY_NAME, RecordType::A).await;
    assert_eq!(msg.metadata.response_code, ResponseCode::NXDomain);
    assert_eq!(hits.load(Ordering::SeqCst), 0);
    assert_eq!(policy.seen()[0].1, Outcome::Canary);

    let policy_off = Recorder::default();
    let g_off = guardiana(up, policy_off.clone(), |c| c.canary_enabled = false).await;
    let msg = ask_udp(g_off.udp_addrs[0], CANARY_NAME, RecordType::A).await;
    assert_eq!(msg.metadata.response_code, ResponseCode::NoError);
    assert_eq!(hits.load(Ordering::SeqCst), 1);
    assert!(matches!(policy_off.seen()[0].1, Outcome::Forwarded { .. }));
}

#[tokio::test]
async fn checker_name_resolves_to_lan_ip_only_in_home_mode() {
    let (up, hits, _upstream) = fake_upstream().await;
    let lan = Ipv4Addr::new(192, 168, 1, 10);
    let policy = Recorder::default();
    let g = guardiana(up, policy.clone(), |c| c.checker_ip = Some(lan)).await;
    let msg = ask_udp(g.udp_addrs[0], CHECKER_NAME, RecordType::A).await;
    assert_eq!(a_records(&msg), vec![lan]);
    assert_eq!(hits.load(Ordering::SeqCst), 0);
    assert_eq!(policy.seen()[0].1, Outcome::Checker);
    let aaaa = ask_udp(g.udp_addrs[0], CHECKER_NAME, RecordType::AAAA).await;
    assert_eq!(aaaa.metadata.response_code, ResponseCode::NoError);
    assert!(aaaa.answers.is_empty());

    let g_off = guardiana(up, Recorder::default(), |c| c.checker_ip = None).await;
    let msg = ask_udp(g_off.udp_addrs[0], CHECKER_NAME, RecordType::A).await;
    assert!(a_records(&msg).is_empty());
    assert_eq!(hits.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn upstream_down_yields_servfail_not_a_hang() {
    let dead: SocketAddr = "127.0.0.1:1".parse().unwrap();
    let policy = Recorder::default();
    let g = guardiana(dead, policy.clone(), |c| {
        c.upstream_timeout = Duration::from_millis(300);
    })
    .await;
    let msg = ask_udp(g.udp_addrs[0], "a.test", RecordType::A).await;
    assert_eq!(msg.metadata.response_code, ResponseCode::ServFail);
    assert_eq!(policy.seen()[0].1, Outcome::UpstreamFailed);
}

#[tokio::test]
async fn tcp_works_too() {
    let (up, _hits, _upstream) = fake_upstream().await;
    let g = guardiana(up, Recorder::default(), |_| {}).await;
    let msg = ask_tcp(g.tcp_addrs[0], "a.test", RecordType::A).await;
    assert_eq!(a_records(&msg), vec![Ipv4Addr::new(1, 2, 3, 4)]);
}

#[tokio::test]
async fn refuses_to_listen_on_public_addresses() {
    let mut cfg = Config::local(vec!["127.0.0.1:1".parse().unwrap()]);
    cfg.listen = vec!["0.0.0.0:0".parse().unwrap()];
    assert!(matches!(
        start(cfg, Recorder::default()).await,
        Err(Error::NotPrivate(_))
    ));
    let mut cfg = Config::local(vec![]);
    cfg.listen = vec!["127.0.0.1:0".parse().unwrap()];
    assert!(matches!(
        start(cfg, Recorder::default()).await,
        Err(Error::NoUpstream)
    ));
}

#[tokio::test]
async fn shutdown_stops_the_server() {
    let (up, _hits, _upstream) = fake_upstream().await;
    let g = guardiana(up, Recorder::default(), |_| {}).await;
    g.shutdown();
    tokio::time::timeout(Duration::from_secs(5), g.wait())
        .await
        .expect("server did not stop")
        .unwrap();
}
