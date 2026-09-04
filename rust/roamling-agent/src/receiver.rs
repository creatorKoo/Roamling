// SPDX-FileCopyrightText: 2026 GooBeom Jeoung
// SPDX-License-Identifier: GPL-3.0-only

//! The authenticated loopback endpoint an agent's hook posts to.
//!
//! Ported from `LoopbackHookReceiver`. The macOS side had to hand-roll BSD
//! sockets to get off Apple's `Network` framework; here it is `TcpListener`.
//! What did not change is everything the endpoint promises:
//!
//! - **Loopback only.** Bound to `127.0.0.1`, never a routable address.
//! - **A token on every request**, checked before the body is used.
//! - **A 1 MiB cap**, because the token can only be checked once the body has
//!   arrived -- without it any local process could make Roamling buffer without
//!   knowing the secret. 1 MiB is where curl starts sending
//!   `Expect: 100-continue`, which this never answers, so a larger cap could not
//!   accept more anyway.
//! - **An empty 204 back**, so nothing here can steer the agent's decision.

use crate::normalize::{self, Agent, TOKEN_HEADER};
use roamling_core::CompanionEvent;
use std::io::{Read, Write};
use std::net::{Ipv4Addr, SocketAddrV4, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::sync::Arc;
use std::time::{Duration, Instant};

const MAXIMUM_REQUEST_BYTES: usize = 1_024 * 1_024;

pub struct Receiver {
    agent: Agent,
    running: Arc<AtomicBool>,
}

impl Receiver {
    pub fn agent(&self) -> Agent {
        self.agent
    }

    /// Start listening, handing each event to `events`.
    ///
    /// The listener owns a thread and the tick drains the channel, so nothing
    /// here ever touches the window or the runtime -- the message loop stays
    /// the only place the pet is stepped.
    pub fn start(agent: Agent, token: String, started: Instant, events: Sender<CompanionEvent>) -> std::io::Result<Self> {
        let address = SocketAddrV4::new(Ipv4Addr::LOCALHOST, agent.port());
        let listener = TcpListener::bind(address)?;
        let running = Arc::new(AtomicBool::new(true));
        let flag = running.clone();

        std::thread::Builder::new()
            .name(format!("roamling-{}", agent.id()))
            .spawn(move || {
                for incoming in listener.incoming() {
                    if !flag.load(Ordering::Relaxed) {
                        break;
                    }
                    let Ok(stream) = incoming else { continue };
                    let now = started.elapsed().as_secs_f64();
                    if let Some(event) = serve(agent, &token, stream, now) {
                        // A closed channel means the app is going away.
                        if events.send(event).is_err() {
                            break;
                        }
                    }
                }
            })?;

        Ok(Self { agent, running })
    }

    pub fn stop(&self) {
        self.running.store(false, Ordering::Relaxed);
        // Unblock `accept` by connecting to it once.
        let _ = TcpStream::connect(SocketAddrV4::new(Ipv4Addr::LOCALHOST, self.agent.port()));
    }
}

impl Drop for Receiver {
    fn drop(&mut self) {
        self.stop();
    }
}

fn serve(agent: Agent, token: &str, mut stream: TcpStream, now: f64) -> Option<CompanionEvent> {
    // A hook that stalls must not hold the thread: the agent side gives up
    // after 0.3 s anyway.
    let _ = stream.set_read_timeout(Some(Duration::from_millis(500)));
    let _ = stream.set_write_timeout(Some(Duration::from_millis(500)));

    let mut raw = Vec::new();
    let mut chunk = [0u8; 4096];
    let mut event = None;
    let mut status: (u16, &str) = (400, "Bad Request");

    loop {
        match stream.read(&mut chunk) {
            Ok(0) => break,
            Ok(read) => {
                raw.extend_from_slice(&chunk[..read]);
                if raw.len() > MAXIMUM_REQUEST_BYTES {
                    status = (413, "Payload Too Large");
                    raw.clear();
                    break;
                }
                match parse(&raw) {
                    Parsed::Incomplete => continue,
                    Parsed::Malformed => break,
                    Parsed::Complete(request) => {
                        status = answer(agent, token, &request, now, &mut event);
                        break;
                    }
                }
            }
            Err(_) => break,
        }
    }

    let response = format!(
        "HTTP/1.1 {} {}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
        status.0, status.1
    );
    let _ = stream.write_all(response.as_bytes());
    let _ = stream.flush();
    event
}

fn answer(
    agent: Agent,
    token: &str,
    request: &Request<'_>,
    now: f64,
    out: &mut Option<CompanionEvent>,
) -> (u16, &'static str) {
    if request.method != "POST" || request.path != agent.path() {
        return (404, "Not Found");
    }
    if request.token.as_deref() != Some(token) {
        return (401, "Unauthorized");
    }
    // A payload we have no answer for is still a well-formed request. Saying so
    // keeps the agent from logging an error on every compaction.
    *out = normalize::event(agent, request.body, now);
    (204, "No Content")
}

struct Request<'a> {
    method: &'a str,
    path: &'a str,
    token: Option<String>,
    body: &'a [u8],
}

enum Parsed<'a> {
    Incomplete,
    Malformed,
    Complete(Request<'a>),
}

fn parse(raw: &[u8]) -> Parsed<'_> {
    let Some(split) = raw.windows(4).position(|window| window == b"\r\n\r\n") else {
        return Parsed::Incomplete;
    };
    let (head, rest) = raw.split_at(split);
    let body = &rest[4..];
    let Ok(head) = std::str::from_utf8(head) else {
        return Parsed::Malformed;
    };

    let mut lines = head.split("\r\n");
    let Some(request_line) = lines.next() else {
        return Parsed::Malformed;
    };
    let mut parts = request_line.split(' ');
    let (Some(method), Some(path)) = (parts.next(), parts.next()) else {
        return Parsed::Malformed;
    };

    let mut token = None;
    let mut length = 0usize;
    for line in lines {
        let Some((name, value)) = line.split_once(':') else {
            continue;
        };
        let value = value.trim();
        // Header names are case-insensitive and curl does not promise a case.
        match name.trim().to_ascii_lowercase().as_str() {
            name if name == TOKEN_HEADER.to_ascii_lowercase() => token = Some(value.to_string()),
            "content-length" => length = value.parse().unwrap_or(0),
            _ => {}
        }
    }

    if body.len() < length {
        return Parsed::Incomplete;
    }
    Parsed::Complete(Request {
        method,
        path,
        token,
        body: &body[..length.min(body.len())],
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(head: &str, body: &str) -> Vec<u8> {
        format!("{head}\r\n\r\n{body}").into_bytes()
    }

    /// A request still arriving must not be answered as if it were malformed:
    /// curl writes the head and the body in separate packets often enough.
    #[test]
    fn a_half_arrived_request_waits() {
        assert!(matches!(parse(b"POST /v1 HTTP/1.1\r\nHost: x"), Parsed::Incomplete));
        let head = "POST /v1 HTTP/1.1\r\nContent-Length: 10";
        assert!(matches!(parse(&request(head, "short")), Parsed::Incomplete));
    }

    /// The token header's case is curl's business, not ours.
    #[test]
    fn the_token_header_is_matched_without_case() {
        let head = "POST /v1/hooks/codex HTTP/1.1\r\nx-roamling-token: abc\r\nContent-Length: 2";
        let raw = request(head, "{}");
        let Parsed::Complete(parsed) = parse(&raw) else {
            panic!("did not parse");
        };
        assert_eq!(parsed.token.as_deref(), Some("abc"));
        assert_eq!(parsed.method, "POST");
        assert_eq!(parsed.path, "/v1/hooks/codex");
        assert_eq!(parsed.body, b"{}");
    }

    /// Wrong token, wrong path and wrong method each have to be refused, and
    /// none of them may produce an event.
    #[test]
    fn only_an_authenticated_post_to_the_right_path_is_answered() {
        let body = br#"{"session_id":"s","hook_event_name":"Stop"}"#;
        let mut event = None;

        let wrong_token = Request {
            method: "POST",
            path: "/v1/hooks/codex",
            token: Some("nope".into()),
            body,
        };
        assert_eq!(answer(Agent::Codex, "secret", &wrong_token, 1.0, &mut event).0, 401);
        assert!(event.is_none());

        let wrong_path = Request {
            method: "POST",
            path: "/v1/hooks/claude-code",
            token: Some("secret".into()),
            body,
        };
        assert_eq!(answer(Agent::Codex, "secret", &wrong_path, 1.0, &mut event).0, 404);
        assert!(event.is_none());

        let good = Request {
            method: "POST",
            path: "/v1/hooks/codex",
            token: Some("secret".into()),
            body,
        };
        assert_eq!(answer(Agent::Codex, "secret", &good, 1.0, &mut event).0, 204);
        assert!(event.is_some(), "an authenticated Stop should produce an event");
    }
}
