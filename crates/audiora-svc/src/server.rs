//! Loopback TCP server speaking the JSON-lines control protocol.
//! Binds 127.0.0.1 only; a named pipe with an ACL replaces this before release.

use std::io::{BufRead, BufReader, ErrorKind, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use crate::api::Api;

const MAX_LINE: usize = 64 * 1024;

pub fn serve(api: Arc<Api>, port: u16, stop: Arc<AtomicBool>) -> std::io::Result<()> {
    let listener = TcpListener::bind(("127.0.0.1", port))?;
    listener.set_nonblocking(true)?;
    while !stop.load(Ordering::Relaxed) {
        match listener.accept() {
            Ok((stream, _)) => {
                let (api, stop) = (api.clone(), stop.clone());
                std::thread::spawn(move || {
                    let _ = client(stream, &api, &stop);
                });
            }
            Err(e) if e.kind() == ErrorKind::WouldBlock => std::thread::sleep(Duration::from_millis(50)),
            Err(e) => return Err(e),
        }
    }
    Ok(())
}

fn client(stream: TcpStream, api: &Api, stop: &AtomicBool) -> std::io::Result<()> {
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
