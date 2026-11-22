//! Just enough HTTP for a local tool: one request per connection, GET and
//! POST, a path and a query string.

use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;

pub struct Request {
    pub method: String,
    pub path: String,
    pub query: HashMap<String, String>,
    /// The Range header, if any (`bytes=a-b`).
    pub range: Option<String>,
    /// What was sent with it (up to 8 MiB).
    pub body: Vec<u8>,
}

impl Request {
    pub fn get(&self, k: &str) -> Option<&str> {
        self.query.get(k).map(String::as_str)
    }
    pub fn num<T: std::str::FromStr>(&self, k: &str) -> Option<T> {
        self.get(k).and_then(|v| v.parse().ok())
    }
    /// A hash given as hex (with or without 0x).
    pub fn hex(&self, k: &str) -> Option<u64> {
        self.get(k).and_then(|v| u64::from_str_radix(v.trim_start_matches("0x"), 16).ok())
    }
}

pub struct Reply {
    pub status: u16,
    pub ctype: &'static str,
    pub body: Vec<u8>,
    /// May the browser keep it (tiles, textures, static files).
    pub keep: bool,
    pub extra: Vec<(&'static str, String)>,
}

impl Reply {
    pub fn json(body: String) -> Reply {
        Reply { status: 200, ctype: "application/json", body: body.into_bytes(), keep: false, extra: Vec::new() }
    }
    pub fn bytes(ctype: &'static str, body: Vec<u8>, keep: bool) -> Reply {
        Reply { status: 200, ctype, body, keep, extra: Vec::new() }
    }
    pub fn missing(what: &str) -> Reply {
        Reply { status: 404, ctype: "application/json", body: format!("{{\"error\":{}}}", quote(what)).into_bytes(), keep: false, extra: Vec::new() }
    }
    pub fn bad(what: &str) -> Reply {
        Reply { status: 400, ctype: "application/json", body: format!("{{\"error\":{}}}", quote(what)).into_bytes(), keep: false, extra: Vec::new() }
    }
}

/// A JSON string literal.
pub fn quote(s: &str) -> String {
    let mut o = String::with_capacity(s.len() + 2);
    o.push('"');
    for c in s.chars() {
        match c {
            '"' => o.push_str("\\\""),
            '\\' => o.push_str("\\\\"),
            '\n' => o.push_str("\\n"),
            '\r' => o.push_str("\\r"),
            '\t' => o.push_str("\\t"),
            c if (c as u32) < 0x20 => o.push_str(&format!("\\u{:04x}", c as u32)),
            c => o.push(c),
        }
    }
    o.push('"');
    o
}

fn unescape(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'+' => out.push(b' '),
            b'%' if i + 2 < b.len() => {
                if let Ok(v) = u8::from_str_radix(std::str::from_utf8(&b[i + 1..i + 3]).unwrap_or("zz"), 16) {
                    out.push(v);
                    i += 2;
                } else {
                    out.push(b'%');
                }
            }
            c => out.push(c),
        }
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Read one request.
pub fn read(stream: &TcpStream) -> Option<Request> {
    let mut r = BufReader::new(stream);
    let mut line = String::new();
    r.read_line(&mut line).ok()?;
    let mut parts = line.split_whitespace();
    let (method, target) = (parts.next()?.to_string(), parts.next()?.to_string());
    let mut length = 0usize;
    let mut range = None;
    loop {
        let mut h = String::new();
        if r.read_line(&mut h).ok()? == 0 || h == "\r\n" || h == "\n" {
            break;
        }
        if let Some((k, v)) = h.split_once(':') {
            if k.eq_ignore_ascii_case("content-length") {
                length = v.trim().parse().unwrap_or(0);
            } else if k.eq_ignore_ascii_case("range") {
                range = Some(v.trim().to_string());
            }
        }
    }
    let mut body = vec![0; length.min(8 << 20)];
    r.read_exact(&mut body).ok()?;
    let (path, q) = target.split_once('?').unwrap_or((&target, ""));
    let query = q.split('&').filter(|p| !p.is_empty()).map(|p| p.split_once('=').unwrap_or((p, ""))).map(|(k, v)| (unescape(k), unescape(v))).collect();
    Some(Request { method, path: unescape(path), query, range, body })
}

pub fn write(mut stream: TcpStream, r: Reply) {
    let reason = match r.status {
        200 => "OK",
        206 => "Partial Content",
        400 => "Bad Request",
        404 => "Not Found",
        _ => "Error",
    };
    let cache = if r.keep { "max-age=86400" } else { "no-store" };
    let extra: String = r.extra.iter().map(|(k, v)| format!("{k}: {v}\r\n")).collect();
    let head = format!("HTTP/1.1 {} {reason}\r\nContent-Type: {}\r\nContent-Length: {}\r\nCache-Control: {cache}\r\n{extra}Connection: close\r\n\r\n", r.status, r.ctype, r.body.len());
    let _ = stream.write_all(head.as_bytes());
    let _ = stream.write_all(&r.body);
}
