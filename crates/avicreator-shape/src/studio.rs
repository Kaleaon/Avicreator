//! `avishape studio`: a small local HTTP server behind the Shape Studio page.
//!
//! Routes: `GET /` (the page), `GET /api/source` (program text and mtime, so
//! the page picks up edits made on disk, e.g. by an AI agent),
//! `POST /api/compile`, `POST /api/glb` and `POST /api/save`.
//! Only loopback hosts are accepted, and POSTs must be same-origin JSON.

use std::io::{self, BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::UNIX_EPOCH;

use serde_json::{json, Value};

use crate::{build_glb, glsl, preview, preview_in_pose, forms, shapeshift, mesh, MeshOptions, GlbOptions, Preview, ShapeError};

const MAX_BODY: usize = 8 * 1024 * 1024;

fn request_preview(text: &str, body: &Value) -> Result<Preview, ShapeError> {
    let pose = body.get("pose").and_then(Value::as_str).filter(|p| !p.is_empty());
    if let Some(target) = body.get("target").and_then(Value::as_str).filter(|t| !t.is_empty()) {
        let target = forms::get(target).ok_or_else(|| ShapeError::Program("unknown target form".into()))?;
        let progress = body.get("progress").and_then(Value::as_f64)
            .ok_or_else(|| ShapeError::Program("shape shifting requires numeric progress within 0..1".into()))? as f32;
        shapeshift::preview_shift(text, target, progress, pose)
    } else {
        preview_in_pose(text, &[], pose)
    }
}

pub fn serve(program: PathBuf, listener: TcpListener) -> io::Result<()> {
    let program = Arc::new(program);
    for stream in listener.incoming() {
        let Ok(stream) = stream else { continue };
        let program = Arc::clone(&program);
        std::thread::spawn(move || {
            let _ = handle(stream, &program);
        });
    }
    Ok(())
}

struct Request {
    method: String,
    path: String,
    host: String,
    origin: Option<String>,
    content_type: String,
    body: Vec<u8>,
}

fn read_request(stream: &TcpStream) -> io::Result<Request> {
    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    reader.read_line(&mut line)?;
    let mut words = line.split_whitespace();
    let method = words.next().unwrap_or_default().to_string();
    let path = words.next().unwrap_or_default().split('?').next().unwrap_or_default().to_string();
    let (mut host, mut origin, mut content_type, mut length) = (String::new(), None, String::new(), 0usize);
    loop {
        line.clear();
        if reader.read_line(&mut line)? == 0 || line.trim().is_empty() {
            break;
        }
        if let Some((name, value)) = line.split_once(':') {
            let value = value.trim().to_string();
            match name.trim().to_ascii_lowercase().as_str() {
                "host" => host = value,
                "origin" => origin = Some(value),
                "content-type" => content_type = value,
                "content-length" => length = value.parse().unwrap_or(0),
                _ => {}
            }
        }
    }
    if length > MAX_BODY {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "body too large"));
    }
    let mut body = vec![0; length];
    reader.read_exact(&mut body)?;
    Ok(Request { method, path, host, origin, content_type, body })
}

fn respond(mut stream: &TcpStream, status: &str, content_type: &str, body: &[u8], extra: &[(&str, String)]) -> io::Result<()> {
    let mut head = format!(
        "HTTP/1.1 {}\r\nContent-Type: {}\r\nContent-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\n",
        status,
        content_type,
        body.len()
    );
    for (name, value) in extra {
        head.push_str(&format!("{}: {}\r\n", name, value));
    }
    head.push_str("\r\n");
    stream.write_all(head.as_bytes())?;
    stream.write_all(body)?;
    stream.flush()
}

fn respond_json(stream: &TcpStream, status: &str, value: &Value) -> io::Result<()> {
    respond(stream, status, "application/json", value.to_string().as_bytes(), &[])
}

fn is_loopback(host: &str) -> bool {
    let name = if let Some(rest) = host.strip_prefix('[') {
        rest.split(']').next().unwrap_or_default()
    } else {
        host.split(':').next().unwrap_or_default()
    };
    matches!(name, "127.0.0.1" | "localhost" | "::1")
}

fn mtime(path: &Path) -> f64 {
    std::fs::metadata(path)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map_or(0.0, |d| d.as_secs_f64() * 1000.0)
}

fn handle(stream: TcpStream, program: &Path) -> io::Result<()> {
    let req = read_request(&stream)?;
    if !is_loopback(&req.host) {
        return respond(&stream, "403 Forbidden", "text/plain", b"studio only answers loopback hosts", &[]);
    }
    if req.method == "POST" {
        let same_origin = req.origin.as_deref().is_none_or(|o| o == format!("http://{}", req.host));
        if !same_origin || !req.content_type.starts_with("application/json") {
            return respond(&stream, "403 Forbidden", "text/plain", b"cross-origin or non-JSON request", &[]);
        }
    }
    let body: Value = if req.body.is_empty() {
        Value::Null
    } else {
        match serde_json::from_slice(&req.body) {
            Ok(v) => v,
            Err(e) => return respond_json(&stream, "400 Bad Request", &json!({ "ok": false, "error": e.to_string() })),
        }
    };
    let text = body.get("text").and_then(Value::as_str).unwrap_or_default();

    match (req.method.as_str(), req.path.as_str()) {
        ("GET", "/") | ("GET", "/index.html") => {
            respond(&stream, "200 OK", "text/html; charset=utf-8", glsl::viewer_html(None).as_bytes(), &[])
        }
        ("GET", "/api/source") => {
            let source = std::fs::read_to_string(program).unwrap_or_default();
            respond_json(
                &stream,
                "200 OK",
                &json!({ "text": source, "mtime": mtime(program), "path": program.display().to_string() }),
            )
        }
        ("GET", "/api/forms") => respond_json(&stream, "200 OK", &json!({
            "forms": forms::FORMS.iter().map(|(name, text)| json!({"name": name, "text": text})).collect::<Vec<_>>()
        })),
        ("POST", "/api/compile") => match request_preview(text, &body) {
            Ok(p) => respond_json(&stream, "200 OK", &json!({ "ok": true, "glsl": p.glsl, "meta": p.meta })),
            Err(e) => respond_json(&stream, "200 OK", &json!({ "ok": false, "error": e.to_string() })),
        },
        ("POST", "/api/preview-mesh") => match request_preview(text, &body) {
            Ok(p) => {
                let mut opts = MeshOptions::new(64);
                opts.triangles = Some(30000);
                let result = mesh(&p.shape, opts);
                respond_json(&stream, "200 OK", &json!({"ok": true, "meta": p.meta,
                    "mesh": glsl::mesh_payload(&p.shape, &result.mesh)}))
            },
            Err(e) => respond_json(&stream, "200 OK", &json!({"ok": false, "error": e.to_string()})),
        },
        ("POST", "/api/glb") => match request_preview(text, &body) {
            Ok(p) => {
                let resolution = body.get("resolution").and_then(Value::as_u64).map(|r| r.clamp(8, 320) as u32);
                let animation = body.get("animation").and_then(Value::as_bool).unwrap_or(true);
                let (glb, stats) = build_glb(&p.shape, resolution, GlbOptions { animation, ..GlbOptions::default() });
                let summary = json!({
                    "vertices": stats.vertices, "triangles": stats.triangles,
                    "millis": (stats.millis * 10.0).round() / 10.0, "grid": stats.grid,
                    "samples_evaluated": stats.samples_evaluated, "samples_total": stats.samples_total,
                    "bytes": glb.len(),
                });
                respond(&stream, "200 OK", "model/gltf-binary", &glb, &[("X-Avishape-Stats", summary.to_string())])
            }
            Err(e) => respond_json(&stream, "400 Bad Request", &json!({ "ok": false, "error": e.to_string() })),
        },
        ("POST", "/api/save") => match preview(text, &[]) {
            Ok(_) => match std::fs::write(program, text) {
                Ok(()) => respond_json(&stream, "200 OK", &json!({ "ok": true, "mtime": mtime(program) })),
                Err(e) => respond_json(&stream, "500 Internal Server Error", &json!({ "ok": false, "error": e.to_string() })),
            },
            Err(e) => respond_json(&stream, "200 OK", &json!({ "ok": false, "error": format!("not saved: {}", e) })),
        },
        _ => respond(&stream, "404 Not Found", "text/plain", b"not found", &[]),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_loopback_hosts_are_served() {
        assert!(is_loopback("127.0.0.1:8765"));
        assert!(is_loopback("localhost:8765"));
        assert!(is_loopback("[::1]:8765"));
        assert!(!is_loopback("evil.example:8765"));
        assert!(!is_loopback("127.0.0.1.evil.example"));
    }
}
