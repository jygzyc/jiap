//! Minimal blocking HTTP client for talking to decx-native-server on localhost.
//! Kept dependency-free on purpose: the CLI only needs POST JSON + GET with
//! `Connection: close` semantics, which std::net handles fine.

use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::Duration;

pub struct HttpResponse {
    pub status: u16,
    pub body: String,
}

fn request(port: u16, method: &str, path: &str, body: Option<&str>) -> std::io::Result<HttpResponse> {
    let mut stream = TcpStream::connect(("127.0.0.1", port))?;
    stream.set_read_timeout(Some(Duration::from_secs(3600)))?;
    stream.set_write_timeout(Some(Duration::from_secs(30)))?;
    let body_bytes = body.unwrap_or("");
    let req = format!(
        "{method} {path} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nContent-Type: application/json\r\nAccept: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body_bytes}",
        body_bytes.len()
    );
    stream.write_all(req.as_bytes())?;
    let mut raw = Vec::new();
    stream.read_to_end(&mut raw)?;
    let text = String::from_utf8_lossy(&raw).to_string();
    let (head, resp_body) = match text.split_once("\r\n\r\n") {
        Some((h, b)) => (h.to_string(), b.to_string()),
        None => (text.clone(), String::new()),
    };
    let status = head
        .lines()
        .next()
        .and_then(|l| l.split_whitespace().nth(1))
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    let chunked = head.to_lowercase().contains("transfer-encoding: chunked");
    let final_body = if chunked { decode_chunked(&resp_body) } else { resp_body };
    Ok(HttpResponse { status, body: final_body })
}

fn decode_chunked(data: &str) -> String {
    let mut out = String::new();
    let mut rest = data;
    loop {
        let Some((size_line, after)) = rest.split_once("\r\n") else { break };
        let size = usize::from_str_radix(size_line.trim().split(';').next().unwrap_or("0"), 16).unwrap_or(0);
        if size == 0 {
            break;
        }
        let end = after.len().min(size);
        out.push_str(&after[..end]);
        rest = &after[end..];
        rest = rest.strip_prefix("\r\n").unwrap_or(rest);
    }
    out
}

pub fn get(port: u16, path: &str) -> std::io::Result<HttpResponse> {
    request(port, "GET", path, None)
}

pub fn post_json(port: u16, path: &str, body: &serde_json::Value) -> std::io::Result<HttpResponse> {
    request(port, "POST", path, Some(&body.to_string()))
}

/// GET /health with a short timeout — used for probes; Ok(false) when unreachable.
pub fn health_ok(port: u16) -> bool {
    let mut stream = match TcpStream::connect(("127.0.0.1", port)) {
        Ok(s) => s,
        Err(_) => return false,
    };
    stream.set_read_timeout(Some(Duration::from_secs(3))).ok();
    let _ = stream.write_all(
        format!("GET /health HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nConnection: close\r\n\r\n").as_bytes(),
    );
    let mut raw = Vec::new();
    let _ = stream.read_to_end(&mut raw);
    String::from_utf8_lossy(&raw).contains("200")
}

#[cfg(test)]
mod tests {
    use super::decode_chunked;

    #[test]
    fn decodes_chunks() {
        assert_eq!(decode_chunked("4\r\nabcd\r\n3\r\n123\r\n0\r\n\r\n"), "abcd123");
    }
}
