//! A deliberately small HTTP/1.1 subset — enough for the six endpoints, and nothing else.
//!
//! Why hand-rolled rather than a web framework: the agent must start and keep serving on a
//! laptop with no internet and no working name resolution, and it is compiled into the same
//! workspace as the product. Adding an async runtime plus a framework for six routes would be
//! more code in the trusted path, not less.
//!
//! Deliberate omissions, each of which is answered with an explicit status rather than a guess:
//!   * no keep-alive — every response says `Connection: close`;
//!   * no chunked transfer-encoding — `411`/`501`, the controller always sends `Content-Length`;
//!   * no HTTP/2, no TLS (the LAN link is guarded by the allowlist + bearer token + firewall rule).
//!
//! Request bodies are never buffered whole: [`stream_body_to_file`] copies straight to disk, which
//! is what lets `/push` accept a full build without the agent's memory growing to match.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::path::Path;

use sha2::{Digest, Sha256};

/// Cap on the request line + all headers. Generous for our own client, small enough that a
/// malicious peer cannot make the agent allocate.
const MAX_HEAD_BYTES: usize = 16 * 1024;
/// Copy buffer for body↔disk transfer.
const COPY_CHUNK: usize = 64 * 1024;

#[derive(Debug)]
pub enum HttpError {
    Io(std::io::Error),
    HeadTooLarge,
    Malformed(&'static str),
    /// The peer used a transfer coding the agent does not implement.
    UnsupportedTransferEncoding,
    /// A body arrived without a `Content-Length`.
    LengthRequired,
    BodyTooLarge { limit: u64 },
}

impl std::fmt::Display for HttpError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(e) => write!(f, "i/o error: {e}"),
            Self::HeadTooLarge => write!(f, "request head exceeds {MAX_HEAD_BYTES} bytes"),
            Self::Malformed(what) => write!(f, "malformed request: {what}"),
            Self::UnsupportedTransferEncoding => {
                write!(f, "only Content-Length framing is supported")
            }
            Self::LengthRequired => write!(f, "Content-Length required"),
            Self::BodyTooLarge { limit } => write!(f, "body exceeds the {limit} byte limit"),
        }
    }
}

impl std::error::Error for HttpError {}

impl From<std::io::Error> for HttpError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}

impl HttpError {
    /// The status the peer should be told about. Keeping this next to the error stops the
    /// server from inventing a different status for the same condition in two places.
    pub fn status(&self) -> u16 {
        match self {
            Self::Io(_) => 400,
            Self::HeadTooLarge => 431,
            Self::Malformed(_) => 400,
            Self::UnsupportedTransferEncoding => 501,
            Self::LengthRequired => 411,
            Self::BodyTooLarge { .. } => 413,
        }
    }
}

/// A parsed request line + headers. The body is left on the wire for the handler to stream.
pub struct Head {
    pub method: String,
    /// Percent-decoded path, no query string.
    pub path: String,
    /// Percent-decoded query parameters, in arrival order.
    pub query: Vec<(String, String)>,
    /// Header names lowercased; values trimmed.
    pub headers: Vec<(String, String)>,
    pub content_length: u64,
}

impl Head {
    pub fn query_get(&self, key: &str) -> Option<&str> {
        self.query.iter().find(|(k, _)| k == key).map(|(_, v)| v.as_str())
    }
}

/// Decode `%XX` escapes. Note `+` is left alone: the controller sends Windows paths through
/// `[Uri]::EscapeDataString`, which encodes a space as `%20`, and treating `+` as a space would
/// corrupt any file name that legitimately contains one.
fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let hi = (bytes[i + 1] as char).to_digit(16);
            let lo = (bytes[i + 2] as char).to_digit(16);
            if let (Some(hi), Some(lo)) = (hi, lo) {
                out.push((hi * 16 + lo) as u8);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    // Lossy is correct here: an invalid-UTF-8 path is rejected downstream by the sandbox
    // validator, and failing at the decode step would lose the audit record of what was tried.
    String::from_utf8_lossy(&out).into_owned()
}

fn parse_query(raw: &str) -> Vec<(String, String)> {
    raw.split('&')
        .filter(|p| !p.is_empty())
        .map(|pair| match pair.split_once('=') {
            Some((k, v)) => (percent_decode(k), percent_decode(v)),
            None => (percent_decode(pair), String::new()),
        })
        .collect()
}

/// Read and parse the request line and headers, leaving the reader positioned at the body.
pub fn read_head(reader: &mut BufReader<TcpStream>) -> Result<Head, HttpError> {
    let mut raw = Vec::with_capacity(1024);
    // Read line by line rather than byte by byte, but keep a hard cap on the total.
    loop {
        let mut line = Vec::with_capacity(128);
        let n = read_line_limited(reader, &mut line, MAX_HEAD_BYTES - raw.len())?;
        if n == 0 {
            return Err(HttpError::Malformed("connection closed before end of headers"));
        }
        raw.extend_from_slice(&line);
        if line == b"\r\n" || line == b"\n" {
            break;
        }
        if raw.len() >= MAX_HEAD_BYTES {
            return Err(HttpError::HeadTooLarge);
        }
    }

    let text = String::from_utf8_lossy(&raw);
    let mut lines = text.split("\r\n").flat_map(|l| l.split('\n'));

    let request_line = lines.next().ok_or(HttpError::Malformed("no request line"))?;
    let mut parts = request_line.split(' ');
    let method = parts.next().ok_or(HttpError::Malformed("no method"))?.to_string();
    let target = parts.next().ok_or(HttpError::Malformed("no request target"))?;
    let version = parts.next().unwrap_or("HTTP/1.1");
    if !version.starts_with("HTTP/1.") {
        return Err(HttpError::Malformed("unsupported HTTP version"));
    }
    if method.is_empty() || target.is_empty() {
        return Err(HttpError::Malformed("empty method or target"));
    }

    let (path_raw, query_raw) = match target.split_once('?') {
        Some((p, q)) => (p, q),
        None => (target, ""),
    };

    let mut headers = Vec::new();
    for line in lines {
        if line.is_empty() {
            continue;
        }
        let (name, value) = line.split_once(':').ok_or(HttpError::Malformed("header without ':'"))?;
        headers.push((name.trim().to_ascii_lowercase(), value.trim().to_string()));
    }

    if headers.iter().any(|(n, v)| n == "transfer-encoding" && !v.eq_ignore_ascii_case("identity")) {
        return Err(HttpError::UnsupportedTransferEncoding);
    }

    let content_length = match headers.iter().find(|(n, _)| n == "content-length") {
        Some((_, v)) => v.parse::<u64>().map_err(|_| HttpError::Malformed("bad Content-Length"))?,
        None => 0,
    };

    Ok(Head {
        method,
        path: percent_decode(path_raw),
        query: parse_query(query_raw),
        headers,
        content_length,
    })
}

/// `read_until(b'\n')` with an explicit ceiling, so a peer that never sends a newline cannot
/// drive the agent out of memory.
fn read_line_limited(
    reader: &mut BufReader<TcpStream>,
    out: &mut Vec<u8>,
    limit: usize,
) -> Result<usize, HttpError> {
    let mut total = 0;
    loop {
        let available = reader.fill_buf()?;
        if available.is_empty() {
            return Ok(total);
        }
        match available.iter().position(|b| *b == b'\n') {
            Some(idx) => {
                out.extend_from_slice(&available[..=idx]);
                reader.consume(idx + 1);
                return Ok(total + idx + 1);
            }
            None => {
                let take = available.len();
                out.extend_from_slice(&available[..take]);
                reader.consume(take);
                total += take;
                if total > limit {
                    return Err(HttpError::HeadTooLarge);
                }
            }
        }
    }
}

/// Read a small body (a JSON job spec) fully into memory. `cap` is enforced before allocating.
pub fn read_body_to_vec(
    reader: &mut BufReader<TcpStream>,
    len: u64,
    cap: u64,
) -> Result<Vec<u8>, HttpError> {
    if len > cap {
        return Err(HttpError::BodyTooLarge { limit: cap });
    }
    let mut buf = vec![0u8; len as usize];
    reader.read_exact(&mut buf)?;
    Ok(buf)
}

/// Copy exactly `len` body bytes straight to `path`, hashing on the way past.
///
/// This is the "capture to disk first" rule applied to uploads: at no point does the agent hold
/// the whole file, so `/push` of a 200 MB build costs a 64 KiB buffer. The returned digest lets
/// the controller prove the bytes on the laptop are the bytes it sent, over a link that is about
/// to be deliberately broken.
pub fn stream_body_to_file(
    reader: &mut BufReader<TcpStream>,
    len: u64,
    limit: u64,
    path: &Path,
) -> Result<(u64, String), HttpError> {
    if len > limit {
        return Err(HttpError::BodyTooLarge { limit });
    }
    let mut file = std::fs::File::create(path)?;
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; COPY_CHUNK];
    let mut remaining = len;
    while remaining > 0 {
        let want = COPY_CHUNK.min(remaining as usize);
        let got = reader.read(&mut buf[..want])?;
        if got == 0 {
            return Err(HttpError::Malformed("connection closed mid-body"));
        }
        file.write_all(&buf[..got])?;
        hasher.update(&buf[..got]);
        remaining -= got as u64;
    }
    // Force the bytes out before the response claims success — the point of this agent is that
    // the network may die immediately after, and a file still sitting in the page cache with no
    // process alive to flush it is exactly the failure this design exists to survive.
    file.sync_all()?;
    Ok((len, super::util::hex(&hasher.finalize())))
}

/// Drain and discard a body the handler is not going to read. Without this, the response to a
/// rejected request races the peer's still-arriving body and the peer sees a connection reset
/// instead of the 403 that explains what happened.
pub fn discard_body(reader: &mut BufReader<TcpStream>, len: u64) {
    let mut remaining = len.min(64 * 1024 * 1024);
    let mut buf = vec![0u8; COPY_CHUNK];
    while remaining > 0 {
        let want = COPY_CHUNK.min(remaining as usize);
        match reader.read(&mut buf[..want]) {
            Ok(0) | Err(_) => return,
            Ok(n) => remaining -= n as u64,
        }
    }
}

fn reason(status: u16) -> &'static str {
    match status {
        200 => "OK",
        201 => "Created",
        202 => "Accepted",
        400 => "Bad Request",
        401 => "Unauthorized",
        403 => "Forbidden",
        404 => "Not Found",
        405 => "Method Not Allowed",
        411 => "Length Required",
        413 => "Payload Too Large",
        414 => "URI Too Long",
        429 => "Too Many Requests",
        431 => "Request Header Fields Too Large",
        500 => "Internal Server Error",
        501 => "Not Implemented",
        503 => "Service Unavailable",
        _ => "Unknown",
    }
}

/// Write a complete response with an in-memory body.
pub fn write_response(
    stream: &mut TcpStream,
    status: u16,
    content_type: &str,
    body: &[u8],
) -> std::io::Result<()> {
    let head = format!(
        "HTTP/1.1 {status} {}\r\n\
         Content-Type: {content_type}\r\n\
         Content-Length: {}\r\n\
         Cache-Control: no-store\r\n\
         X-Content-Type-Options: nosniff\r\n\
         Connection: close\r\n\r\n",
        reason(status),
        body.len()
    );
    stream.write_all(head.as_bytes())?;
    stream.write_all(body)?;
    stream.flush()
}

/// JSON convenience wrapper. `body` must already be valid JSON.
pub fn write_json(stream: &mut TcpStream, status: u16, body: &str) -> std::io::Result<()> {
    write_response(stream, status, "application/json; charset=utf-8", body.as_bytes())
}

/// A one-field JSON error object, with the message escaped.
pub fn write_error(stream: &mut TcpStream, status: u16, message: &str) -> std::io::Result<()> {
    let body = format!(
        "{{\"error\":\"{}\",\"status\":{status}}}",
        super::util::json_escape(message)
    );
    write_json(stream, status, &body)
}

/// Stream a file out as the response body, 64 KiB at a time — `/pull` of a large capture bundle
/// must not be bounded by the agent's memory any more than `/push` is.
pub fn write_file(stream: &mut TcpStream, path: &Path) -> std::io::Result<u64> {
    let mut file = std::fs::File::open(path)?;
    let len = file.metadata()?.len();
    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("download");
    let head = format!(
        "HTTP/1.1 200 OK\r\n\
         Content-Type: application/octet-stream\r\n\
         Content-Length: {len}\r\n\
         Content-Disposition: attachment; filename=\"{}\"\r\n\
         Cache-Control: no-store\r\n\
         X-Content-Type-Options: nosniff\r\n\
         Connection: close\r\n\r\n",
        super::util::sanitize_for_log(name, 120)
    );
    stream.write_all(head.as_bytes())?;
    let mut buf = vec![0u8; COPY_CHUNK];
    let mut sent = 0u64;
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        stream.write_all(&buf[..n])?;
        sent += n as u64;
    }
    stream.flush()?;
    Ok(sent)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn percent_decoding_handles_windows_paths() {
        assert_eq!(percent_decode("docs%2Fcaptures%2F01.txt"), "docs/captures/01.txt");
        assert_eq!(percent_decode("a%20b.txt"), "a b.txt");
        assert_eq!(percent_decode("docs%5Ccaptures"), r"docs\captures");
        // `+` must survive verbatim — it is a legal file-name character.
        assert_eq!(percent_decode("c++notes.txt"), "c++notes.txt");
        // A malformed escape is passed through rather than swallowed.
        assert_eq!(percent_decode("100%zz"), "100%zz");
        assert_eq!(percent_decode("trailing%"), "trailing%");
    }

    #[test]
    fn query_parsing_splits_and_decodes() {
        let q = parse_query("path=docs%2Fa.txt&tail=4096");
        assert_eq!(q.len(), 2);
        assert_eq!(q[0], ("path".to_string(), "docs/a.txt".to_string()));
        assert_eq!(q[1], ("tail".to_string(), "4096".to_string()));
        // A bare key yields an empty value rather than being dropped.
        assert_eq!(parse_query("flag"), vec![("flag".to_string(), String::new())]);
        assert!(parse_query("").is_empty());
    }

    #[test]
    fn head_lookup_finds_query_parameters() {
        let head = Head {
            method: "GET".into(),
            path: "/pull".into(),
            query: parse_query("path=a.txt"),
            headers: vec![],
            content_length: 0,
        };
        assert_eq!(head.query_get("path"), Some("a.txt"));
        assert_eq!(head.query_get("missing"), None);
    }

    #[test]
    fn every_error_maps_to_a_deliberate_status() {
        assert_eq!(HttpError::LengthRequired.status(), 411);
        assert_eq!(HttpError::UnsupportedTransferEncoding.status(), 501);
        assert_eq!(HttpError::BodyTooLarge { limit: 10 }.status(), 413);
        assert_eq!(HttpError::HeadTooLarge.status(), 431);
        assert_eq!(HttpError::Malformed("x").status(), 400);
    }

    #[test]
    fn reason_phrases_exist_for_the_statuses_actually_used() {
        for s in [200, 201, 202, 400, 401, 403, 404, 405, 411, 413, 429, 431, 500, 501, 503] {
            assert_ne!(reason(s), "Unknown", "status {s} needs a reason phrase");
        }
    }
}
