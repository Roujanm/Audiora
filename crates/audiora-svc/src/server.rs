//! Loopback servers for the control protocol:
//!
//! * TCP JSON-lines for developer tools and scripts.
//! * WebSocket (one JSON message per frame) for the Svelte/Tauri app, with an
//!   Origin allow-list and Host check so a web page in the user's browser cannot
//!   drive the engine (cross-site WebSocket hijacking / DNS rebinding).
//!
//! Both bind 127.0.0.1 only. A named pipe with an ACL replaces TCP before release.

use std::io::{BufRead, BufReader, ErrorKind, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use serde_json::{json, Value};
use tungstenite::handshake::server::{ErrorResponse, Request, Response};
use tungstenite::{Error as WsError, Message};

use crate::api::Api;

const MAX_LINE: usize = 64 * 1024;
const MAX_CLIENTS: usize = 16;

/// Origins the shipped app and its dev server use. Requests without an
/// `Origin` header (scripts, native clients) are always allowed.
pub const DEFAULT_ORIGINS: [&str; 5] = [
    "tauri://localhost",
    "http://tauri.localhost",
    "https://tauri.localhost",
    "http://localhost:5173",
    "http://127.0.0.1:5173",
];

pub fn serve(api: Arc<Api>, port: u16, stop: Arc<AtomicBool>) -> std::io::Result<()> {
    accept_loop(port, &stop, move |stream, stop| {
        let _ = tcp_client(stream, &api, &stop);
    })
}

pub fn serve_ws(
    api: Arc<Api>,
    port: u16,
    origins: Vec<String>,
    stop: Arc<AtomicBool>,
) -> std::io::Result<()> {
    let origins = Arc::new(origins);
    accept_loop(port, &stop, move |stream, stop| {
        let _ = ws_client(stream, &api, port, &origins, &stop);
    })
}

fn accept_loop(
    port: u16,
    stop: &Arc<AtomicBool>,
    handler: impl Fn(TcpStream, Arc<AtomicBool>) + Send + Sync + 'static,
) -> std::io::Result<()> {
    let listener = TcpListener::bind(("127.0.0.1", port))?;
    listener.set_nonblocking(true)?;
    let handler = Arc::new(handler);
    let clients = Arc::new(AtomicUsize::new(0));
    while !stop.load(Ordering::Relaxed) {
        match listener.accept() {
            Ok((stream, _)) => {
                if clients.load(Ordering::Relaxed) >= MAX_CLIENTS {
                    continue; // dropping the stream closes it
                }
                clients.fetch_add(1, Ordering::Relaxed);
                let (handler, stop, clients) = (handler.clone(), stop.clone(), clients.clone());
                std::thread::spawn(move || {
                    handler(stream, stop);
                    clients.fetch_sub(1, Ordering::Relaxed);
                });
            }
            Err(e) if e.kind() == ErrorKind::WouldBlock => std::thread::sleep(Duration::from_millis(50)),
            Err(e) => return Err(e),
        }
    }
    Ok(())
}

fn tcp_client(stream: TcpStream, api: &Api, stop: &AtomicBool) -> std::io::Result<()> {
    stream.set_nonblocking(false)?;
    stream.set_read_timeout(Some(Duration::from_millis(250)))?;
    let mut writer = stream.try_clone()?;
    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    while !stop.load(Ordering::Relaxed) {
        match reader.read_line(&mut line) {
            Ok(0) => return Ok(()),
            Ok(_) => {
                if !line.trim().is_empty() {
                    let mut reply = api.handle(line.trim());
                    reply.push('\n');
                    writer.write_all(reply.as_bytes())?;
                }
                line.clear();
            }
            // Timeout: keep any partial line and re-check the stop flag.
            Err(e) if matches!(e.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) => {}
            Err(e) => return Err(e),
        }
        if line.len() > MAX_LINE {
            return Err(std::io::Error::new(ErrorKind::InvalidData, "request too large"));
        }
    }
    Ok(())
}

/// Decides whether a WebSocket upgrade request may proceed.
pub fn handshake_allowed(host: Option<&str>, origin: Option<&str>, port: u16, allowed: &[String]) -> bool {
    let host_ok = matches!(host, Some(h) if h == format!("127.0.0.1:{port}")
        || h == format!("localhost:{port}")
        || h == format!("[::1]:{port}"));
    let origin_ok = match origin {
        None => true,
        Some(o) => allowed.iter().any(|a| a == o),
    };
    host_ok && origin_ok
}

fn ws_client(
    stream: TcpStream,
    api: &Api,
    port: u16,
    origins: &[String],
    stop: &AtomicBool,
) -> Result<(), WsError> {
    stream.set_nonblocking(false)?;
    stream.set_nodelay(true)?;
    stream.set_read_timeout(Some(Duration::from_secs(5)))?; // handshake
    stream.set_write_timeout(Some(Duration::from_secs(2)))?;
    let check = |req: &Request, resp: Response| -> Result<Response, ErrorResponse> {
        let h = |name: &str| req.headers().get(name).and_then(|v| v.to_str().ok());
        if handshake_allowed(h("host"), h("origin"), port, origins) {
            Ok(resp)
        } else {
            let mut r = ErrorResponse::new(Some("forbidden origin or host".into()));
            *r.status_mut() = tungstenite::http::StatusCode::FORBIDDEN;
            Err(r)
        }
    };
    let mut ws = tungstenite::accept_hdr(stream, check).map_err(|e| match e {
        tungstenite::HandshakeError::Failure(f) => f,
        tungstenite::HandshakeError::Interrupted(_) => WsError::ConnectionClosed,
    })?;
    ws.get_ref().set_read_timeout(Some(Duration::from_millis(15)))?;

    let meter_paths = api.meter_paths();
    let mut interval: Option<Duration> = None;
    let mut next_push = Instant::now();

    while !stop.load(Ordering::Relaxed) {
        match ws.read() {
            Ok(Message::Text(t)) => {
                if t.len() > MAX_LINE {
                    return Ok(());
                }
                let reply = match parse_cmd(t.as_str()) {
                    Some(("subscribe", v)) => {
                        let hz = v.get("hz").and_then(Value::as_f64).unwrap_or(30.0).clamp(1.0, 60.0);
                        interval = Some(Duration::from_secs_f64(1.0 / hz));
                        next_push = Instant::now();
                        let band_hz: Vec<f32> = (0..audiora_dsp::SPECTRUM_BANDS).map(audiora_dsp::band_frequency).collect();
                        json!({ "ok": true, "id": v.get("id"), "hz": hz, "paths": meter_paths, "band_hz": band_hz }).to_string()
                    }
                    Some(("unsubscribe", v)) => {
                        interval = None;
                        json!({ "ok": true, "id": v.get("id") }).to_string()
                    }
                    _ => api.handle(t.as_str()),
                };
                ws.send(Message::text(reply))?;
            }
            Ok(Message::Close(_)) => {
                let _ = ws.flush();
                return Ok(());
            }
            Ok(_) => {} // pings are answered by tungstenite; binary is ignored
            Err(WsError::Io(e)) if matches!(e.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) => {}
            Err(WsError::ConnectionClosed | WsError::AlreadyClosed) => return Ok(()),
            Err(e) => return Err(e),
        }
        if let Some(iv) = interval {
            if Instant::now() >= next_push {
                next_push += iv;
                if next_push < Instant::now() {
                    next_push = Instant::now() + iv; // do not burst after a stall
                }
                let msg = json!({ "event": "meters", "v": api.meter_values(&meter_paths) });
                ws.send(Message::text(msg.to_string()))?;
                ws.send(Message::text(api.analysis_event().to_string()))?;
            }
        }
    }
    Ok(())
}

fn parse_cmd(text: &str) -> Option<(&'static str, Value)> {
    let v: Value = serde_json::from_str(text).ok()?;
    let name = match v.get("cmd")?.as_str()? {
        "subscribe" => "subscribe",
        "unsubscribe" => "unsubscribe",
        _ => return None,
    };
    Some((name, v))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn allowed() -> Vec<String> {
        DEFAULT_ORIGINS.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn app_and_scripts_are_allowed() {
        assert!(handshake_allowed(Some("127.0.0.1:7879"), Some("tauri://localhost"), 7879, &allowed()));
        assert!(handshake_allowed(Some("localhost:7879"), Some("http://localhost:5173"), 7879, &allowed()));
        assert!(handshake_allowed(Some("127.0.0.1:7879"), None, 7879, &allowed()), "no Origin = not a browser");
    }

    #[test]
    fn foreign_web_pages_and_rebinding_hosts_are_refused() {
        assert!(!handshake_allowed(Some("127.0.0.1:7879"), Some("https://evil.example"), 7879, &allowed()));
        assert!(!handshake_allowed(Some("127.0.0.1:7879"), Some("http://localhost:3000"), 7879, &allowed()));
        assert!(!handshake_allowed(Some("evil.example:7879"), None, 7879, &allowed()), "DNS rebinding");
        assert!(!handshake_allowed(None, None, 7879, &allowed()));
    }

    #[test]
    fn subscribe_commands_are_recognised() {
        assert_eq!(parse_cmd(r#"{"cmd":"subscribe","hz":10}"#).unwrap().0, "subscribe");
        assert!(parse_cmd(r#"{"cmd":"set"}"#).is_none());
        assert!(parse_cmd("garbage").is_none());
    }
}
