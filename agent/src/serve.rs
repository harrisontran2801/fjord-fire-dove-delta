use crate::config::load_config;
use crate::doctor::{doctor_json, run_doctor};
use crate::inspect::{inspect_path, inspect_to_json};
use crate::pipeline::{load_run, optimize};
use crate::security::{
    bind_is_loopback, cors_header_lines, default_sample_rel, host_allowed, mutating_origin_ok,
    resolve_config_path, resolve_inspect_path, restrict_unix_socket, sample_config_path,
    workspace_root, ERR_HOST,
};
use crate::util::{new_run_id, MAX_INSPECT_BYTES};
use serde_json::json;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::os::unix::net::UnixListener;
use std::path::PathBuf;
use std::thread;

pub const DEFAULT_BIND: &str = "127.0.0.1:4783";
pub const DEFAULT_SOCKET: &str = "/tmp/quench-agent.sock";

struct HttpReq {
    method: String,
    path: String,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
}

fn header<'a>(req: &'a HttpReq, name: &str) -> Option<&'a str> {
    req.headers
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case(name))
        .map(|(_, v)| v.as_str())
}

fn origin_of(req: &HttpReq) -> Option<&str> {
    header(req, "origin")
}

fn read_request(stream: &mut impl Read) -> Result<HttpReq, String> {
    let mut reader = BufReader::new(stream);
    let mut start = String::new();
    reader.read_line(&mut start).map_err(|e| e.to_string())?;
    let mut parts = start.split_whitespace();
    let method = parts.next().unwrap_or("").to_string();
    let path = parts.next().unwrap_or("/").to_string();
    if method.is_empty() {
        return Err("empty request".into());
    }
    let mut headers = Vec::new();
    loop {
        let mut line = String::new();
        reader.read_line(&mut line).map_err(|e| e.to_string())?;
        let t = line.trim_end();
        if t.is_empty() {
            break;
        }
        if let Some((k, v)) = t.split_once(':') {
            headers.push((k.trim().to_string(), v.trim().to_string()));
        }
    }
    let mut content_len = 0usize;
    if let Some((_, v)) = headers
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case("content-length"))
    {
        content_len = v.parse().unwrap_or(0);
    }
    if content_len as u64 > MAX_INSPECT_BYTES + 4096 {
        return Err("payload too large".into());
    }
    let mut body = vec![0u8; content_len];
    if content_len > 0 {
        reader.read_exact(&mut body).map_err(|e| e.to_string())?;
    }
    Ok(HttpReq {
        method,
        path,
        headers,
        body,
    })
}

fn respond(
    stream: &mut impl Write,
    status: &str,
    body: &[u8],
    content_type: &str,
    origin: Option<&str>,
) {
    let cors = cors_header_lines(origin);
    let _ = write!(
        stream,
        "HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\n{cors}Connection: close\r\n\r\n",
        body.len()
    );
    let _ = stream.write_all(body);
    let _ = stream.flush();
}

fn json_res(stream: &mut impl Write, status: &str, value: serde_json::Value, origin: Option<&str>) {
    let body = serde_json::to_vec(&value).unwrap_or_else(|_| b"{}".to_vec());
    respond(stream, status, &body, "application/json", origin);
}

fn status_body(bind: &str, socket: &str) -> serde_json::Value {
    let sample = sample_config_path();
    json!({
        "ok": true,
        "name": "quench-agent",
        "version": env!("CARGO_PKG_VERSION"),
        "listen": bind,
        "socket": socket,
        "platform": format!("{}/{}", std::env::consts::OS, std::env::consts::ARCH),
        "workspace": workspace_root().display().to_string(),
        "sampleConfig": default_sample_rel(),
        "sampleConfigPath": sample.as_ref().map(|p| p.display().to_string()),
    })
}

fn handle(req: HttpReq, mut stream: impl Write, bind: &str, socket: &str) {
    let origin = origin_of(&req).map(|s| s.to_string());
    let origin_ref = origin.as_deref();

    if !host_allowed(header(&req, "host")) {
        json_res(
            &mut stream,
            "403 Forbidden",
            json!({"ok": false, "error": ERR_HOST}),
            origin_ref,
        );
        return;
    }

    if req.method == "OPTIONS" {
        if mutating_origin_ok(origin_ref).is_err() {
            json_res(
                &mut stream,
                "403 Forbidden",
                json!({"ok": false, "error": "origin not allowed"}),
                None,
            );
            return;
        }
        respond(&mut stream, "204 No Content", b"", "text/plain", origin_ref);
        return;
    }

    let path = req.path.split('?').next().unwrap_or(&req.path).to_string();
    let mutating = req.method == "POST"
        || req.method == "PUT"
        || req.method == "PATCH"
        || req.method == "DELETE";
    if mutating {
        if let Err(e) = mutating_origin_ok(origin_ref) {
            json_res(
                &mut stream,
                "403 Forbidden",
                json!({"ok": false, "error": e}),
                None,
            );
            return;
        }
    }

    match (req.method.as_str(), path.as_str()) {
        ("GET", "/health") | ("GET", "/v1/status") | ("GET", "/v1/health") => {
            json_res(&mut stream, "200 OK", status_body(bind, socket), origin_ref);
        }
        ("GET", "/v1/doctor") | ("GET", "/doctor") => {
            let report = run_doctor();
            json_res(&mut stream, "200 OK", doctor_json(&report), origin_ref);
        }
        ("POST", "/v1/inspect") | ("POST", "/inspect") => {
            if let Some(p) = header(&req, "X-Quench-Path") {
                match resolve_inspect_path(p) {
                    Ok(resolved) => {
                        let outcome = inspect_path(&resolved);
                        json_res(&mut stream, "200 OK", inspect_to_json(&outcome), origin_ref);
                    }
                    Err(e) => json_res(
                        &mut stream,
                        "403 Forbidden",
                        json!({"ok": false, "error": e}),
                        origin_ref,
                    ),
                }
                return;
            }
            if header(&req, "content-type")
                .unwrap_or("")
                .starts_with("application/json")
            {
                if let Ok(v) = serde_json::from_slice::<serde_json::Value>(&req.body) {
                    if let Some(p) = v.get("path").and_then(|x| x.as_str()) {
                        match resolve_inspect_path(p) {
                            Ok(resolved) => {
                                let outcome = inspect_path(&resolved);
                                json_res(
                                    &mut stream,
                                    "200 OK",
                                    inspect_to_json(&outcome),
                                    origin_ref,
                                );
                            }
                            Err(e) => json_res(
                                &mut stream,
                                "403 Forbidden",
                                json!({"ok": false, "error": e}),
                                origin_ref,
                            ),
                        }
                        return;
                    }
                }
            }
            if req.body.len() as u64 > MAX_INSPECT_BYTES {
                json_res(
                    &mut stream,
                    "413 Payload Too Large",
                    json!({"ok": false, "error": "File is too large to inspect. Maximum size is 96 MB."}),
                    origin_ref,
                );
                return;
            }
            let name = header(&req, "X-Filename").unwrap_or("upload.bin");
            let safe_name = PathBuf::from(name)
                .file_name()
                .and_then(|s| s.to_str())
                .filter(|s| !s.is_empty())
                .unwrap_or("upload.bin")
                .to_string();
            let tmp_dir = crate::doctor::work_dir().join("uploads");
            let _ = std::fs::create_dir_all(&tmp_dir);
            let tmp = tmp_dir.join(format!("{}-{safe_name}", new_run_id()));
            if let Err(e) = std::fs::write(&tmp, &req.body) {
                json_res(
                    &mut stream,
                    "500 Internal Server Error",
                    json!({"ok": false, "error": e.to_string()}),
                    origin_ref,
                );
                return;
            }
            let outcome = inspect_path(&tmp);
            let _ = std::fs::remove_file(&tmp);
            json_res(&mut stream, "200 OK", inspect_to_json(&outcome), origin_ref);
        }
        ("POST", "/v1/optimize") | ("POST", "/optimize") => {
            let parsed: serde_json::Value = serde_json::from_slice(&req.body).unwrap_or(json!({}));
            let config_path = parsed
                .get("configPath")
                .or_else(|| parsed.get("config"))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            if config_path.is_empty() {
                json_res(
                    &mut stream,
                    "400 Bad Request",
                    json!({"ok": false, "error": "configPath is required"}),
                    origin_ref,
                );
                return;
            }
            let resolved = match resolve_config_path(config_path) {
                Ok(p) => p,
                Err(e) => {
                    json_res(
                        &mut stream,
                        "403 Forbidden",
                        json!({"ok": false, "error": e}),
                        origin_ref,
                    );
                    return;
                }
            };
            match load_config(&resolved).and_then(|cfg| optimize(&cfg)) {
                Ok(run) => json_res(
                    &mut stream,
                    "200 OK",
                    serde_json::to_value(&run).unwrap_or(json!({})),
                    origin_ref,
                ),
                Err(e) => json_res(
                    &mut stream,
                    "400 Bad Request",
                    json!({"ok": false, "error": e}),
                    origin_ref,
                ),
            }
        }
        (m, p) if m == "GET" && (p.starts_with("/v1/runs/") || p.starts_with("/runs/")) => {
            let rest = p
                .trim_start_matches("/v1/runs/")
                .trim_start_matches("/runs/");
            let (id, report) = if let Some(id) = rest.strip_suffix("/report") {
                (id, true)
            } else {
                (rest, false)
            };
            match load_run(id) {
                Ok(run) => {
                    if report {
                        json_res(&mut stream, "200 OK", run.report, origin_ref);
                    } else {
                        json_res(
                            &mut stream,
                            "200 OK",
                            serde_json::to_value(&run).unwrap_or(json!({})),
                            origin_ref,
                        );
                    }
                }
                Err(e) => json_res(
                    &mut stream,
                    "404 Not Found",
                    json!({"ok": false, "error": e}),
                    origin_ref,
                ),
            }
        }
        (m, p) if m == "GET" && (p.starts_with("/v1/report/") || p.starts_with("/report/")) => {
            let id = p
                .trim_start_matches("/v1/report/")
                .trim_start_matches("/report/");
            match load_run(id) {
                Ok(run) => json_res(&mut stream, "200 OK", run.report, origin_ref),
                Err(e) => json_res(
                    &mut stream,
                    "404 Not Found",
                    json!({"ok": false, "error": e}),
                    origin_ref,
                ),
            }
        }
        _ => json_res(
            &mut stream,
            "404 Not Found",
            json!({"ok": false, "error": "not found"}),
            origin_ref,
        ),
    }
}

fn serve_tcp_conn(mut stream: TcpStream, bind: String, socket: String) {
    match read_request(&mut stream) {
        Ok(req) => handle(req, stream, &bind, &socket),
        Err(e) => {
            if e.contains("too large") {
                json_res(
                    &mut stream,
                    "413 Payload Too Large",
                    json!({"ok": false, "error": "File is too large to inspect. Maximum size is 96 MB."}),
                    None,
                );
            }
        }
    }
}

fn serve_unix_conn(mut stream: std::os::unix::net::UnixStream, bind: String, socket: String) {
    match read_request(&mut stream) {
        Ok(req) => handle(req, stream, &bind, &socket),
        Err(_) => {}
    }
}

pub fn serve(bind: &str, socket: &str) -> Result<(), String> {
    if !bind_is_loopback(bind) {
        return Err(format!(
            "quench-agent only binds loopback (127.0.0.1). Refusing {bind}"
        ));
    }
    let _ = std::fs::remove_file(socket);
    let tcp = TcpListener::bind(bind).map_err(|e| format!("bind {bind}: {e}"))?;
    tcp.set_nonblocking(false).ok();
    let unix = UnixListener::bind(socket).map_err(|e| format!("bind unix {socket}: {e}"))?;
    restrict_unix_socket(std::path::Path::new(socket))?;
    println!("quench-agent serve {bind} unix:{socket}");
    let unix_bind = bind.to_string();
    let unix_socket = socket.to_string();
    thread::spawn(move || {
        for conn in unix.incoming() {
            if let Ok(s) = conn {
                let bind = unix_bind.clone();
                let socket = unix_socket.clone();
                thread::spawn(move || serve_unix_conn(s, bind, socket));
            }
        }
    });
    let tcp_bind = bind.to_string();
    let tcp_socket = socket.to_string();
    for conn in tcp.incoming() {
        if let Ok(s) = conn {
            let bind = tcp_bind.clone();
            let socket = tcp_socket.clone();
            thread::spawn(move || serve_tcp_conn(s, bind, socket));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    fn dispatch(raw: &str) -> String {
        let mut input = Cursor::new(raw.as_bytes().to_vec());
        let req = read_request(&mut input).expect("parse request");
        let mut out = Vec::new();
        handle(req, &mut out, DEFAULT_BIND, DEFAULT_SOCKET);
        String::from_utf8_lossy(&out).into_owned()
    }

    #[test]
    fn http_unknown_origin_is_rejected_on_optimize() {
        let raw = "POST /v1/optimize HTTP/1.1\r\nHost: 127.0.0.1:4783\r\nOrigin: https://evil.example\r\nContent-Type: application/json\r\nContent-Length: 28\r\n\r\n{\"configPath\":\"/etc/passwd\"}";
        let res = dispatch(raw);
        assert!(res.starts_with("HTTP/1.1 403"), "{res}");
        assert!(res.contains("origin not allowed"), "{res}");
        assert!(
            !res.to_ascii_lowercase()
                .contains("access-control-allow-origin: *"),
            "{res}"
        );
    }

    #[test]
    fn http_out_of_root_config_path_is_rejected() {
        let raw = "POST /v1/optimize HTTP/1.1\r\nHost: 127.0.0.1:4783\r\nOrigin: http://127.0.0.1:8080\r\nContent-Type: application/json\r\nContent-Length: 28\r\n\r\n{\"configPath\":\"/etc/passwd\"}";
        let res = dispatch(raw);
        assert!(res.starts_with("HTTP/1.1 403"), "{res}");
        assert!(
            res.contains("workspace") || res.contains("configPath"),
            "{res}"
        );
        assert!(!res.contains("Access-Control-Allow-Origin: *"), "{res}");
    }

    #[test]
    fn http_cors_echoes_allowlisted_origin_only() {
        let raw = "GET /v1/status HTTP/1.1\r\nHost: 127.0.0.1:4783\r\nOrigin: http://127.0.0.1:8080\r\n\r\n";
        let res = dispatch(raw);
        assert!(
            res.contains("Access-Control-Allow-Origin: http://127.0.0.1:8080"),
            "{res}"
        );
        assert!(!res.contains("Access-Control-Allow-Origin: *"), "{res}");
        assert!(res.contains("\"sampleConfig\""), "{res}");
    }
}
