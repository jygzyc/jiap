//! Networking: a minimal HTTP/1.1 client for the local DECX server plus
//! curl-based HTTPS downloads.
//!
//! The DECX server is always on 127.0.0.1, so the hot path uses a
//! hand-rolled HTTP/1.1 client over `TcpStream` — no TLS stack, no C
//! compiler, no external crates (same zero-dependency philosophy as
//! `decx-native`). Internet downloads (self install, URL targets) shell out
//! to the system `curl` (shipped with Windows 10+, ubiquitous elsewhere).

use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::Path;
use std::time::Duration;

use serde_json::Value;

use crate::error::{DecxError, DecxResult};

#[derive(Debug)]
pub struct HttpResponse {
    pub status: u16,
    pub body: Vec<u8>,
}

impl HttpResponse {
    pub fn body_json(&self) -> DecxResult<Value> {
        serde_json::from_slice(&self.body)
            .map_err(|e| DecxError::server("BAD_RESPONSE", format!("Malformed JSON response: {e}")))
    }

    pub fn body_string(&self) -> String {
        String::from_utf8_lossy(&self.body).to_string()
    }
}

/// Perform one request against the local DECX server.
pub fn request(
    port: u16,
    method: &str,
    path: &str,
    body: Option<&Value>,
    timeout: Duration,
) -> DecxResult<HttpResponse> {
    let payload = body.map(|b| b.to_string().into_bytes()).unwrap_or_default();
    let mut raw = String::with_capacity(256 + payload.len());
    raw.push_str(&format!("{method} {path} HTTP/1.1\r\n"));
    raw.push_str(&format!("Host: 127.0.0.1:{port}\r\n"));
    raw.push_str("Accept: application/json\r\n");
    raw.push_str("Connection: close\r\n");
    if body.is_some() {
        raw.push_str("Content-Type: application/json\r\n");
        raw.push_str(&format!("Content-Length: {}\r\n", payload.len()));
    }
    raw.push_str("\r\n");

    let addr = std::net::SocketAddr::from(([127, 0, 0, 1], port));
    let mut stream = TcpStream::connect_timeout(&addr, timeout.min(Duration::from_secs(10)))
        .map_err(map_io("Connection failed"))?;
    stream
        .set_read_timeout(Some(timeout))
        .and_then(|()| stream.set_write_timeout(Some(timeout)))
        .map_err(map_io("Connection failed"))?;

    // Headers and body go out in one write: a server that reads the request
    // with a single recv() and closes would otherwise RST mid-exchange.
    let mut bytes = raw.into_bytes();
    bytes.extend_from_slice(&payload);
    stream.write_all(&bytes).map_err(map_io("Connection failed"))?;

    let mut buf = Vec::with_capacity(16 * 1024);
    let mut reader = std::io::Read::by_ref(&mut stream).take(256 * 1024 * 1024);
    if let Err(e) = reader.read_to_end(&mut buf) {
        // Windows peers that close with unread receive data send RST instead
        // of FIN; tolerate that (and read timeouts) once what we already read
        // parses as a complete response.
        if parse_response(&buf).is_none() {
            let timed_out = matches!(e.kind(), std::io::ErrorKind::TimedOut | std::io::ErrorKind::WouldBlock);
            return Err(if timed_out {
                DecxError::timeout("Request timed out")
            } else {
                map_io("Connection failed")(e)
            });
        }
    }
    parse_response(&buf).ok_or_else(|| DecxError::server("BAD_RESPONSE", "Malformed HTTP response"))
}

fn map_io(prefix: &'static str) -> impl Fn(std::io::Error) -> DecxError {
    move |e: std::io::Error| {
        if matches!(e.kind(), std::io::ErrorKind::TimedOut | std::io::ErrorKind::WouldBlock) {
            DecxError::timeout(prefix)
        } else {
            DecxError::connection(format!("{prefix}: {e}"))
        }
    }
}

/// Parse an HTTP/1.1 response (Content-Length, chunked, or read-to-end).
pub fn parse_response(raw: &[u8]) -> Option<HttpResponse> {
    let header_end = raw.windows(4).position(|w| w == b"\r\n\r\n")?;
    let head = String::from_utf8_lossy(&raw[..header_end]);
    let mut lines = head.lines();
    let status_line = lines.next()?;
    let status: u16 = status_line.split_whitespace().nth(1)?.parse().ok()?;

    let mut chunked = false;
    let mut content_length: Option<usize> = None;
    for line in lines {
        let Some((name, value)) = line.split_once(':') else { continue };
        let name = name.trim().to_ascii_lowercase();
        let value = value.trim();
        if name == "transfer-encoding" && value.eq_ignore_ascii_case("chunked") {
            chunked = true;
        } else if name == "content-length" {
            content_length = value.parse().ok();
        }
    }

    let body_raw = &raw[header_end + 4..];
    let body = if chunked {
        decode_chunked(body_raw)?
    } else if let Some(len) = content_length {
        body_raw.get(..len.min(body_raw.len())).unwrap_or(body_raw).to_vec()
    } else {
        body_raw.to_vec()
    };
    Some(HttpResponse { status, body })
}

fn decode_chunked(mut data: &[u8]) -> Option<Vec<u8>> {
    let mut out = Vec::new();
    loop {
        let line_end = data.windows(2).position(|w| w == b"\r\n")?;
        let size_str = String::from_utf8_lossy(&data[..line_end]);
        let size = usize::from_str_radix(size_str.trim().split(';').next()?.trim(), 16).ok()?;
        data = &data[line_end + 2..];
        if size == 0 {
            break;
        }
        if data.len() < size {
            return None;
        }
        out.extend_from_slice(&data[..size]);
        data = &data[size..];
        // trailing CRLF after each chunk
        if data.starts_with(b"\r\n") {
            data = &data[2..];
        }
    }
    Some(out)
}

// ── curl-backed internet access ─────────────────────────────────────────────

fn curl_program() -> &'static str {
    // Prefer the OS-shipped curl on Windows; the mingw curl in PATH would also
    // work but the System32 one is guaranteed present on Win10+.
    #[cfg(windows)]
    {
        let system = Path::new(r"C:\Windows\System32\curl.exe");
        if system.exists() {
            return "C:\\Windows\\System32\\curl.exe";
        }
    }
    "curl"
}

fn curl_base() -> std::process::Command {
    let mut cmd = std::process::Command::new(curl_program());
    cmd.arg("-sSL").arg("--fail").arg("--max-time").arg("600");
    cmd
}

/// GET a URL as text (follows redirects).
pub fn http_get_text(url: &str, accept: &str) -> DecxResult<String> {
    let output = curl_base()
        .arg("-H")
        .arg(format!("Accept: {accept}"))
        .arg(url)
        .output()
        .map_err(|e| DecxError::connection(format!("Download failed (cannot run curl): {e}")))?;
    if !output.status.success() {
        return Err(DecxError::connection(format!(
            "Download failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

/// GET a URL and parse the body as JSON.
pub fn http_get_json(url: &str) -> DecxResult<Value> {
    let text = http_get_text(url, "application/json")?;
    serde_json::from_str(&text).map_err(|e| DecxError::connection(format!("Malformed JSON from {url}: {e}")))
}

/// Download a URL to a file (follows redirects), returning the byte count.
pub fn download_to_file(url: &str, dest: &Path) -> DecxResult<u64> {
    let output = curl_base().arg("-o").arg(dest).arg(url).output().map_err(|e| {
        DecxError::connection(format!("Download failed (cannot run curl): {e}"))
    })?;
    if !output.status.success() {
        let _ = std::fs::remove_file(dest);
        return Err(DecxError::connection(format!(
            "Download failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    Ok(std::fs::metadata(dest)
        .map(|m| m.len())
        .unwrap_or(0))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_content_length_response() {
        let raw = b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 8\r\n\r\n{\"ok\":1}\r\n";
        let resp = parse_response(raw).unwrap();
        assert_eq!(resp.status, 200);
        assert_eq!(resp.body, b"{\"ok\":1}".to_vec());
    }

    #[test]
    fn parses_chunked_response() {
        let raw = b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n4\r\n{\"ok\r\n4\r\n\":1}\r\n0\r\n\r\n";
        let resp = parse_response(raw).unwrap();
        assert_eq!(resp.body, b"{\"ok\":1}".to_vec());
    }

    #[test]
    fn parses_read_to_end_response() {
        let raw = b"HTTP/1.0 404 Not Found\r\nContent-Type: text/plain\r\n\r\nnope";
        let resp = parse_response(raw).unwrap();
        assert_eq!(resp.status, 404);
        assert_eq!(resp.body, b"nope".to_vec());
    }

    #[test]
    fn rejects_garbage() {
        assert!(parse_response(b"garbage").is_none());
    }

    #[test]
    fn request_roundtrip_against_local_server() {
        let listener = std::net::TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let port = listener.local_addr().unwrap().port();
        let server = std::thread::spawn(move || {
            let (mut sock, _) = listener.accept().unwrap();
            let mut buf = [0u8; 4096];
            let n = std::io::Read::read(&mut sock, &mut buf).unwrap_or(0);
            let req = String::from_utf8_lossy(&buf[..n]).to_string();
            assert!(req.starts_with("POST /api/decx/get_classes HTTP/1.1\r\n"));
            assert!(req.contains("Connection: close"));
            let body = b"{\"code\":\"OK\",\"data\":[\"com.a.B\"]}";
            let resp = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            );
            use std::io::Write;
            sock.write_all(resp.as_bytes()).unwrap();
            sock.write_all(body).unwrap();
        });

        let resp = request(
            port,
            "POST",
            "/api/decx/get_classes",
            Some(&serde_json::json!({ "filter": { "includes": [], "excludes": [] }, "page": 1 })),
            Duration::from_secs(5),
        )
        .unwrap();
        assert_eq!(resp.status, 200);
        assert_eq!(resp.body_json().unwrap()["data"][0], "com.a.B");
        server.join().unwrap();
    }
}
