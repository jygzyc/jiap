//! Hand-rolled HTTP/1.1 server (std::net only): thread-per-connection,
//! Content-Length bodies, Connection: close, keep-alive optional off.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};

pub struct Request {
    pub method: String,
    pub path: String,
    pub body: Vec<u8>,
}

pub fn serve<F>(addr: &str, handler: F) -> std::io::Result<()>
where
    F: Fn(Request) -> (u16, String) + Send + Sync + 'static,
{
    let listener = TcpListener::bind(addr)?;
    let handler = std::sync::Arc::new(handler);
    for stream in listener.incoming() {
        match stream {
            Ok(s) => {
                let h = handler.clone();
                std::thread::spawn(move || {
                    let _ = handle_conn(s, move |r| h(r));
                });
            }
            Err(_) => continue,
        }
    }
    Ok(())
}

fn handle_conn<F>(stream: TcpStream, handler: F) -> std::io::Result<()>
where
    F: Fn(Request) -> (u16, String),
{
    let mut reader = BufReader::new(stream.try_clone()?);
    let mut out = stream;
    loop {
        let mut line = String::new();
        if reader.read_line(&mut line)? == 0 {
            return Ok(());
        }
        let line = line.trim_end().to_string();
        if line.is_empty() {
            continue;
        }
        let mut parts = line.split_whitespace();
        let method = parts.next().unwrap_or("").to_string();
        let path = parts.next().unwrap_or("/").to_string();
        // headers
        let mut content_length = 0usize;
        let mut keep_alive = false;
        loop {
            let mut h = String::new();
            if reader.read_line(&mut h)? == 0 {
                return Ok(());
            }
            let h = h.trim_end();
            if h.is_empty() {
                break;
            }
            let lower = h.to_lowercase();
            if let Some(v) = lower.strip_prefix("content-length:") {
                content_length = v.trim().parse().unwrap_or(0);
            }
            if let Some(v) = lower.strip_prefix("connection:") {
                keep_alive = v.trim().contains("keep-alive");
            }
        }
        let mut body = vec![0u8; content_length.min(64 * 1024 * 1024)];
        if content_length > 0 {
            reader.read_exact(&mut body)?;
        }
        let (status, body_out) = handler(Request { method, path, body });
        let reason = match status {
            200 => "OK",
            400 => "Bad Request",
            404 => "Not Found",
            405 => "Method Not Allowed",
            503 => "Service Unavailable",
            504 => "Gateway Timeout",
            _ => "Internal Server Error",
        };
        let head = format!(
            "HTTP/1.1 {} {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: {}\r\n\r\n",
            status,
            reason,
            body_out.len(),
            if keep_alive { "keep-alive" } else { "close" }
        );
        out.write_all(head.as_bytes())?;
        out.write_all(body_out.as_bytes())?;
        out.flush()?;
        if !keep_alive {
            return Ok(());
        }
    }
}
