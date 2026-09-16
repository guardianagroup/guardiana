//! Per-request logic: validate, ask the policy, answer, then report.

use std::net::{Ipv4Addr, Ipv6Addr};
use std::sync::Arc;

use hickory_proto::op::{Header, HeaderCounts, MessageType, Metadata, OpCode, ResponseCode};
use hickory_proto::rr::rdata::{A, AAAA};
use hickory_proto::rr::{DNSClass, Name, RData, Record, RecordType};
use hickory_resolver::net::runtime::Time;
use hickory_resolver::net::{DnsError, NetError, NoRecords};
use hickory_resolver::TokioResolver;
use hickory_server::server::{Request, RequestHandler, ResponseHandler, ResponseInfo};
use hickory_server::zone_handler::MessageResponseBuilder;

use crate::{BlockMode, Decision, Outcome, Policy, Query, BUILTIN_TTL, CANARY_NAME, CHECKER_NAME};

pub(crate) struct Handler<P: Policy> {
    pub(crate) policy: Arc<P>,
    pub(crate) upstream: TokioResolver,
    pub(crate) block_mode: BlockMode,
    pub(crate) canary_enabled: bool,
    pub(crate) checker_ip: Option<Ipv4Addr>,
}

/// Records to put in a response, owned so they outlive the builder.
#[derive(Default)]
struct Answer {
    code: ResponseCode,
    answers: Vec<Record>,
    authorities: Vec<Record>,
}

fn unix_ms() -> i64 {
    guardiana_core::time::now_ms()
}

/// Lowercase, no trailing dot: the form stored in the ledger.
fn normalize(name: &Name) -> String {
    let mut s = name.to_ascii().to_lowercase();
    if s.ends_with('.') {
        s.pop();
    }
    s
}

fn failed_info(request: &Request) -> ResponseInfo {
    let mut metadata = Metadata::response_from_request(&request.metadata);
    metadata.response_code = ResponseCode::ServFail;
    ResponseInfo::from(Header {
        metadata,
        counts: HeaderCounts::default(),
    })
}

impl<P: Policy> Handler<P> {
    async fn send<R: ResponseHandler>(
        &self,
        request: &Request,
        response: &mut R,
        answer: &Answer,
    ) -> Option<ResponseInfo> {
        let mut metadata = Metadata::response_from_request(&request.metadata);
        metadata.recursion_available = true;
        metadata.response_code = answer.code;
        let builder = MessageResponseBuilder::from_message_request(request);
        let msg = builder.build(
            metadata,
            answer.answers.iter(),
            answer.authorities.iter(),
            std::iter::empty::<&Record>(),
            std::iter::empty::<&Record>(),
        );
        response.send_response(msg).await.ok()
    }

    async fn refuse<R: ResponseHandler>(
        &self,
        request: &Request,
        response: &mut R,
        code: ResponseCode,
    ) -> ResponseInfo {
        let builder = MessageResponseBuilder::from_message_request(request);
        let msg = builder.error_msg(&request.metadata, code);
        response
            .send_response(msg)
            .await
            .unwrap_or_else(|_| failed_info(request))
    }

    fn blocked_answer(&self, name: &Name, qtype: RecordType) -> Answer {
        match self.block_mode {
            BlockMode::NxDomain => Answer {
                code: ResponseCode::NXDomain,
                ..Answer::default()
            },
            BlockMode::ZeroIp => {
                let answers = match qtype {
                    RecordType::A => vec![Record::from_rdata(
                        name.clone(),
                        BUILTIN_TTL,
                        RData::A(A::from(Ipv4Addr::UNSPECIFIED)),
                    )],
                    RecordType::AAAA => vec![Record::from_rdata(
                        name.clone(),
                        BUILTIN_TTL,
                        RData::AAAA(AAAA::from(Ipv6Addr::UNSPECIFIED)),
                    )],
                    _ => Vec::new(),
                };
                Answer {
                    code: ResponseCode::NoError,
                    answers,
                    authorities: Vec::new(),
                }
            }
        }
    }

    fn checker_answer(name: &Name, qtype: RecordType, ip: Ipv4Addr) -> Answer {
        let answers = if qtype == RecordType::A {
            vec![Record::from_rdata(
                name.clone(),
                BUILTIN_TTL,
                RData::A(A::from(ip)),
            )]
        } else {
            Vec::new()
        };
        Answer {
            code: ResponseCode::NoError,
            answers,
            authorities: Vec::new(),
        }
    }

    async fn forward(&self, name: &Name, qtype: RecordType) -> (Answer, Outcome) {
        match self.upstream.lookup(name.clone(), qtype).await {
            Ok(lookup) => (
                Answer {
                    code: ResponseCode::NoError,
                    answers: lookup.answers().to_vec(),
                    authorities: Vec::new(),
                },
                Outcome::Forwarded {
                    rcode: ResponseCode::NoError,
                },
            ),
            Err(NetError::Dns(DnsError::NoRecordsFound(NoRecords {
                response_code,
                authorities,
                ..
            }))) => (
                Answer {
                    code: response_code,
                    answers: Vec::new(),
                    authorities: authorities.map(|a| a.to_vec()).unwrap_or_default(),
                },
                Outcome::Forwarded {
                    rcode: response_code,
                },
            ),
            Err(_) => (
                Answer {
                    code: ResponseCode::ServFail,
                    ..Answer::default()
                },
                Outcome::UpstreamFailed,
            ),
        }
    }
}

#[async_trait::async_trait]
impl<P: Policy> RequestHandler for Handler<P> {
    async fn handle_request<R: ResponseHandler, T: Time>(
        &self,
        request: &Request,
        mut response: R,
    ) -> ResponseInfo {
        let ts = unix_ms();
        if request.metadata.message_type != MessageType::Query
            || request.metadata.op_code != OpCode::Query
        {
            return self
                .refuse(request, &mut response, ResponseCode::NotImp)
                .await;
        }
        let queries = request.queries.queries();
        let [lower] = queries else {
            return self
                .refuse(request, &mut response, ResponseCode::FormErr)
                .await;
        };
        if lower.query_class() != DNSClass::IN {
            return self
                .refuse(request, &mut response, ResponseCode::Refused)
                .await;
        }
        let name: Name = lower.original().name().clone();
        let qtype = lower.query_type();
        let query = Query {
            client: request.src(),
            name: normalize(&name),
            qtype,
            ts,
        };

        let (answer, outcome) = if self.canary_enabled && query.name == CANARY_NAME {
            (
                Answer {
                    code: ResponseCode::NXDomain,
                    ..Answer::default()
                },
                Outcome::Canary,
            )
        } else if let (CHECKER_NAME, Some(ip)) = (query.name.as_str(), self.checker_ip) {
            (Self::checker_answer(&name, qtype, ip), Outcome::Checker)
        } else {
            match self.policy.decide(&query) {
                Decision::Block { rule_id } => (
                    self.blocked_answer(&name, qtype),
                    Outcome::Blocked { rule_id },
                ),
                Decision::Forward => self.forward(&name, qtype).await,
            }
        };

        let info = self.send(request, &mut response, &answer).await;
        self.policy.record(&query, outcome);
        info.unwrap_or_else(|| failed_info(request))
    }
}
