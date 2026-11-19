//! Server UI lokal WebTask (panel drag & drop).

use serde_json::{json, Value};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};
use tiny_http::{Header, Method, Response, Server};

use crate::{creds, keys, store};

pub type OpenTx = Arc<Mutex<Option<Sender<String>>>>;

const UI_INDEX: &str = include_str!("ui/shell.html");

pub fn start(port: u16, open_tx: OpenTx) -> std::io::Result<u16> {
    let server = Server::http(("127.0.0.1", port))
        .map_err(|e| std::io::Error::other(e.to_string()))?;
    let actual = server
        .server_addr()
        .to_ip()
        .map(|a| a.port())
        .unwrap_or(port);

    std::thread::spawn(move || {
        for request in server.incoming_requests() {
            let _ = handle(request, &open_tx);
        }
    });
    Ok(actual)
}

fn json_response(code: u16, body: Value) -> Response<std::io::Cursor<Vec<u8>>> {
    let text = body.to_string();
    let header = Header::from_bytes("Content-Type", "application/json").unwrap();
    Response::from_string(text)
        .with_status_code(code)
        .with_header(header)
}

fn public(cfg: &store::Config) -> Value {
    json!({
        "id": cfg.id,
        "filename": cfg.filename,
        "startURL": cfg.start_url,
        "host": cfg.host,
        "cmid": cfg.cmid,
        "configKey": cfg.config_key,
        "importedAt": cfg.imported_at,
    })
}

fn open(open_tx: &OpenTx, url: &str) {
    if let Some(tx) = open_tx.lock().unwrap().as_ref() {
        let _ = tx.send(url.to_string());
    }
}

fn read_body(request: &mut tiny_http::Request) -> Value {
    let mut buf = String::new();
    if let Err(e) = std::io::Read::read_to_string(request.as_reader(), &mut buf) {
        eprintln!("[WebTask] read_body error: {e}");
        return json!({});
    }
    match serde_json::from_str(&buf) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("[WebTask] body bukan JSON ({e}): {buf:.200}");
            json!({})
        }
    }
}

fn handle(
    mut request: tiny_http::Request,
    open_tx: &OpenTx,
) -> std::io::Result<()> {
    let path = request.url().split('?').next().unwrap_or("/").to_string();

    let response = match request.method() {
        Method::Get if path == "/" || path == "/index.html" => {
            let header = Header::from_bytes("Content-Type", "text/html; charset=utf-8").unwrap();
            Response::from_string(UI_INDEX).with_header(header)
        }
        Method::Get if path == "/api/state" => {
            let state = store::load();
            let active = state
                .active
                .as_ref()
                .and_then(|id| state.configs.iter().find(|c| &c.id == id))
                .map(public);
            let creds = creds::public();
            json_response(
                200,
                json!({
                    "ok": true,
                    "active": active,
                    "configs": state.configs.iter().map(public).collect::<Vec<_>>(),
                    "mode": creds.mode,
                    "login": {
                        "username": creds.username,
                        "hasPassword": creds.has_password,
                    },
                }),
            )
        }
        Method::Post if path == "/api/mode" => {
            let payload = read_body(&mut request);
            let raw = payload.get("mode").and_then(Value::as_str).unwrap_or("");
            match creds::Mode::parse(raw) {
                Some(mode) => match creds::set_mode(mode) {
                    Ok(()) => json_response(200, json!({"ok": true, "mode": mode.as_str()})),
                    Err(e) => json_response(500, json!({"ok": false, "error": e.to_string()})),
                },
                None => json_response(400, json!({"ok": false, "error": "mode harus 'seb' atau 'web'"})),
            }
        }
        Method::Post if path == "/api/login" => {
            let payload = read_body(&mut request);
            let username = payload.get("username").and_then(Value::as_str).unwrap_or("");
            let password = payload.get("password").and_then(Value::as_str).unwrap_or("");
            match creds::save(username, password) {
                Ok(()) => {
                    let creds = creds::public();
                    json_response(
                        200,
                        json!({
                            "ok": true,
                            "username": creds.username,
                            "hasPassword": creds.has_password,
                        }),
                    )
                }
                Err(e) => json_response(400, json!({"ok": false, "error": e})),
            }
        }
        Method::Post if path == "/api/logout" => match creds::clear_password() {
            Ok(()) => json_response(200, json!({"ok": true})),
            Err(e) => json_response(500, json!({"ok": false, "error": e.to_string()})),
        },
        Method::Post if path == "/api/open" => {
            let payload = read_body(&mut request);
            let url = payload.get("url").and_then(Value::as_str).unwrap_or("");
            if url.is_empty() {
                json_response(400, json!({"ok": false, "error": "url kosong"}))
            } else {
                open(open_tx, url);
                json_response(200, json!({"ok": true}))
            }
        }
        Method::Post if path == "/api/config" => {
            let payload = read_body(&mut request);
            let content = payload.get("content").and_then(Value::as_str).unwrap_or("");
            let filename = payload
                .get("filename")
                .and_then(Value::as_str)
                .unwrap_or("config.seb");
            if content.is_empty() {
                json_response(400, json!({"ok": false, "error": "content kosong"}))
            } else {
                match keys::load_seb(content.as_bytes(), filename) {
                    Ok(meta) => {
                        let entry = store::add(meta);
                        open(open_tx, &entry.start_url);
                        json_response(200, json!({"ok": true, "config": public(&entry)}))
                    }
                    Err(e) => json_response(400, json!({"ok": false, "error": e.to_string()})),
                }
            }
        }
        Method::Post if path == "/api/activate" => {
            let payload = read_body(&mut request);
            let id = payload.get("id").and_then(Value::as_str).unwrap_or("");
            match store::activate(id) {
                Some(cfg) => {
                    open(open_tx, &cfg.start_url);
                    json_response(200, json!({"ok": true, "config": public(&cfg)}))
                }
                None => json_response(404, json!({"ok": false, "error": "config tidak ditemukan"})),
            }
        }
        Method::Post if path == "/api/delete" => {
            let payload = read_body(&mut request);
            let id = payload.get("id").and_then(Value::as_str).unwrap_or("");
            store::delete(id);
            let active = store::active().map(|c| public(&c));
            json_response(200, json!({"ok": true, "active": active}))
        }
        _ => json_response(404, json!({"ok": false, "error": "not found"})),
    };

    let _ = request.respond(response);
    Ok(())
}