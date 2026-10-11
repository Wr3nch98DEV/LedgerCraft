//! LedgerCraft local server: serves the UI and a JSON API over 127.0.0.1 only.
//! Usage: ledgercraft [--port N] [--data-dir PATH] [--open] [--app]

mod rpc;
mod render;
mod state;

use axum::{
    body::Body,
    extract::{Path, Query, State},
    http::{header, HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use rust_embed::Embed;
use serde_json::{json, Value};
use state::AppState;
use std::{net::SocketAddr, sync::Arc};

#[derive(Embed)]
#[folder = "../../ui/build"]
struct Ui;

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().collect();
    let arg = |k: &str| args.iter().position(|a| a == k).and_then(|i| args.get(i + 1)).cloned();
    let port: u16 = arg("--port").and_then(|p| p.parse().ok()).unwrap_or(4180);
    let data_dir = arg("--data-dir").map(std::path::PathBuf::from).unwrap_or_else(state::default_data_dir);
    std::fs::create_dir_all(data_dir.join("orgs")).expect("create data dir");
    let st = Arc::new(AppState::new(data_dir.clone()));
    st.auto_open();

    let app = Router::new()
        .route("/api/rpc/{method}", post(rpc_handler))
        .route("/api/session", get(session_handler))
        .route("/view/{kind}/{id}", get(view_handler))
        .route("/online/{token}", get(online_handler))
        .route("/download/{kind}", get(download_handler))
        .route("/api/v1/{*path}", get(rest_get).post(rest_post))
        .fallback(get(static_handler))
        .with_state(st.clone());

    // Bind to localhost only. If the port is busy, try the next few.
    let mut listener = None;
    let mut bound = port;
    for p in port..port + 20 {
        if let Ok(l) = tokio::net::TcpListener::bind(SocketAddr::from(([127, 0, 0, 1], p))).await { listener = Some(l); bound = p; break; }
    }
    let listener = listener.expect("no free port");
    let url = format!("http://127.0.0.1:{bound}/");
    println!("LedgerCraft running at {url}  (data: {})", data_dir.display());
    std::fs::write(data_dir.join("server.url"), &url).ok();
    if args.iter().any(|a| a == "--open" || a == "--app") { open_window(&url, args.iter().any(|a| a == "--app")); }
    axum::serve(listener, app).await.unwrap();
}

/// Open the UI: as a chromeless app window (Edge/Chrome --app) when available, else the default browser.
fn open_window(url: &str, app: bool) {
    #[cfg(windows)]
    {
        if app {
            let candidates = [
                r"C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe",
                r"C:\Program Files\Microsoft\Edge\Application\msedge.exe",
                r"C:\Program Files\Google\Chrome\Application\chrome.exe",
            ];
            for c in candidates {
                if std::path::Path::new(c).exists() {
                    let profile = state::default_data_dir().join("window-profile");
                    let _ = std::process::Command::new(c).arg(format!("--app={url}")).arg(format!("--user-data-dir={}", profile.display()))
                        .arg("--window-size=1400,900").arg("--force-dark-mode").spawn();
                    return;
                }
            }
        }
        let _ = std::process::Command::new("cmd").args(["/C", "start", "", url]).spawn();
    }
    #[cfg(target_os = "macos")]
    { let _ = std::process::Command::new("open").arg(url).spawn(); let _ = app; }
    #[cfg(all(unix, not(target_os = "macos")))]
    { let _ = std::process::Command::new("xdg-open").arg(url).spawn(); let _ = app; }
}

fn json_err(status: StatusCode, msg: String) -> Response {
    (status, Json(json!({"error": msg}))).into_response()
}

async fn rpc_handler(State(st): State<Arc<AppState>>, headers: HeaderMap, Path(method): Path<String>, body: Option<Json<Value>>) -> Response {
    let params = body.map(|b| b.0).unwrap_or(Value::Null);
    let token = headers.get("x-session").and_then(|v| v.to_str().ok()).map(String::from);
    let st2 = st.clone();
    let r = tokio::task::spawn_blocking(move || rpc::handle(&st2, token.as_deref(), &method, params)).await;
    match r {
        Ok(Ok(v)) => Json(v).into_response(),
        Ok(Err(e)) => {
            let status = match &e {
                rpc::ApiError::Unauthorized(_) => StatusCode::UNAUTHORIZED,
                rpc::ApiError::Forbidden(_) => StatusCode::FORBIDDEN,
                rpc::ApiError::NotFound(_) => StatusCode::NOT_FOUND,
                rpc::ApiError::Bad(_) => StatusCode::BAD_REQUEST,
            };
            json_err(status, e.to_string())
        }
        Err(e) => json_err(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
    }
}

async fn session_handler(State(st): State<Arc<AppState>>, headers: HeaderMap) -> Response {
    let token = headers.get("x-session").and_then(|v| v.to_str().ok()).map(String::from);
    let st2 = st.clone();
    match tokio::task::spawn_blocking(move || rpc::handle(&st2, token.as_deref(), "session.get", Value::Null)).await {
        Ok(Ok(v)) => Json(v).into_response(),
        Ok(Err(e)) => json_err(StatusCode::BAD_REQUEST, e.to_string()),
        Err(e) => json_err(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
    }
}

async fn view_handler(State(st): State<Arc<AppState>>, Path((kind, id)): Path<(String, i64)>, Query(q): Query<std::collections::HashMap<String, String>>) -> Response {
    let st2 = st.clone();
    let token = q.get("s").cloned();
    match tokio::task::spawn_blocking(move || render::view(&st2, token.as_deref(), &kind, id)).await {
        Ok(Ok(html)) => ([(header::CONTENT_TYPE, "text/html; charset=utf-8")], html).into_response(),
        Ok(Err(e)) => (StatusCode::NOT_FOUND, e).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

async fn online_handler(State(st): State<Arc<AppState>>, Path(token): Path<String>) -> Response {
    let st2 = st.clone();
    match tokio::task::spawn_blocking(move || render::online(&st2, &token)).await {
        Ok(Ok(html)) => ([(header::CONTENT_TYPE, "text/html; charset=utf-8")], html).into_response(),
        Ok(Err(e)) => (StatusCode::NOT_FOUND, e).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

async fn download_handler(State(st): State<Arc<AppState>>, Path(kind): Path<String>, Query(q): Query<std::collections::HashMap<String, String>>) -> Response {
    let st2 = st.clone();
    match tokio::task::spawn_blocking(move || render::download(&st2, &kind, &q)).await {
        Ok(Ok((mime, name, bytes))) => (
            [(header::CONTENT_TYPE, mime), (header::CONTENT_DISPOSITION, format!("attachment; filename=\"{name}\""))],
            Body::from(bytes),
        ).into_response(),
        Ok(Err(e)) => (StatusCode::BAD_REQUEST, e).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

async fn rest_get(State(st): State<Arc<AppState>>, headers: HeaderMap, Path(path): Path<String>, Query(q): Query<std::collections::HashMap<String, String>>) -> Response {
    rest(st, headers, "GET", path, serde_json::to_value(q).unwrap_or(Value::Null)).await
}
async fn rest_post(State(st): State<Arc<AppState>>, headers: HeaderMap, Path(path): Path<String>, body: Option<Json<Value>>) -> Response {
    rest(st, headers, "POST", path, body.map(|b| b.0).unwrap_or(Value::Null)).await
}
async fn rest(st: Arc<AppState>, headers: HeaderMap, verb: &'static str, path: String, body: Value) -> Response {
    let auth = headers.get(header::AUTHORIZATION).and_then(|v| v.to_str().ok()).map(|s| s.trim_start_matches("Bearer ").to_string());
    let st2 = st.clone();
    match tokio::task::spawn_blocking(move || rpc::rest(&st2, auth.as_deref(), verb, &path, body)).await {
        Ok(Ok(v)) => Json(v).into_response(),
        Ok(Err(e)) => json_err(match e { rpc::ApiError::Unauthorized(_) => StatusCode::UNAUTHORIZED, rpc::ApiError::NotFound(_) => StatusCode::NOT_FOUND, rpc::ApiError::Forbidden(_) => StatusCode::FORBIDDEN, _ => StatusCode::BAD_REQUEST }, e.to_string()),
        Err(e) => json_err(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
    }
}

async fn static_handler(uri: axum::http::Uri) -> Response {
    let path = uri.path().trim_start_matches('/');
    let candidates = if path.is_empty() { vec!["index.html".to_string()] } else { vec![path.to_string(), format!("{path}.html"), format!("{path}/index.html")] };
    for c in candidates {
        if let Some(f) = Ui::get(&c) {
            let mime = mime_guess::from_path(&c).first_or_octet_stream();
            let cache = if c.contains("/_app/immutable/") { "public, max-age=31536000, immutable" } else { "no-cache" };
            return ([(header::CONTENT_TYPE, mime.as_ref().to_string()), (header::CACHE_CONTROL, cache.to_string())], f.data.into_owned()).into_response();
        }
    }
    // SPA fallback
    match Ui::get("index.html") {
        Some(f) => ([(header::CONTENT_TYPE, "text/html".to_string())], f.data.into_owned()).into_response(),
        None => (StatusCode::NOT_FOUND, "UI not built. Run start.bat (it builds the UI) or `npm run build` in ui/.").into_response(),
    }
}
