//! `nasfile://` custom protocol: serves bytes straight from the share with
//! HTTP range support, so images, video, audio and PDFs preview without a
//! download. URL shape: `nasfile://localhost/<serverId>/<share>/<path>`
//! (`http://nasfile.localhost/...` on Windows).

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use http::{header, Response, StatusCode};
use percent_encoding::percent_decode_str;
use smb2::FileReader;
use tauri::{AppHandle, Manager, Runtime, UriSchemeResponder};
use tokio::sync::Mutex;

use crate::commands::session_for;
use crate::error::AppError;
use crate::AppState;

pub const SCHEME: &str = "nasfile";
const MAX_RANGE_BYTES: u64 = 2 * 1024 * 1024;
const MAX_FULL_BODY: u64 = 256 * 1024 * 1024;
const IDLE_CLOSE: Duration = Duration::from_secs(90);

struct Cached {
    reader: Arc<FileReader>,
    last_used: Instant,
}

/// Open file handles keyed by `server/share/path`, closed when idle. A
/// video preview issues many small range requests; reopening the file for
/// each would cost a round-trip per request.
#[derive(Default)]
pub struct ReaderCache {
    readers: Mutex<HashMap<String, Cached>>,
}

impl ReaderCache {
    pub async fn get_or_open(&self, state: &AppState, server_id: &str, share: &str, path: &str) -> Result<Arc<FileReader>, AppError> {
        let key = format!("{server_id}/{share}/{path}");
        {
            let mut map = self.readers.lock().await;
            if let Some(c) = map.get_mut(&key) {
                c.last_used = Instant::now();
                return Ok(c.reader.clone());
            }
        }
        let session = session_for(state, server_id).await?;
        let reader = Arc::new(session.lock().await.open_reader(share, path).await?);
        let mut map = self.readers.lock().await;
        map.insert(key, Cached { reader: reader.clone(), last_used: Instant::now() });
        Ok(reader)
    }

    pub async fn close_idle(&self) {
        let stale: Vec<(String, Arc<FileReader>)> = {
            let mut map = self.readers.lock().await;
            let keys: Vec<String> = map
                .iter()
                .filter(|(_, c)| c.last_used.elapsed() > IDLE_CLOSE)
                .map(|(k, _)| k.clone())
                .collect();
            keys.into_iter()
                .filter_map(|k| map.remove(&k).map(|c| (k, c.reader)))
                .collect()
        };
        for (_, reader) in stale {
            if let Ok(reader) = Arc::try_unwrap(reader) {
                let _ = reader.close().await;
            }
        }
    }

    pub async fn drop_server(&self, server_id: &str) {
        let prefix = format!("{server_id}/");
        let mut map = self.readers.lock().await;
        map.retain(|k, _| !k.starts_with(&prefix));
    }
}

pub fn spawn_janitor(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_secs(30)).await;
            if let Some(state) = app.try_state::<AppState>() {
                state.preview.close_idle().await;
            }
        }
    });
}

pub fn handle<R: Runtime>(app: AppHandle<R>, request: http::Request<Vec<u8>>, responder: UriSchemeResponder) {
    tauri::async_runtime::spawn(async move {
        let response = match serve(&app, &request).await {
            Ok(r) => r,
            Err(e) => {
                let status = match e.code.as_str() {
                    "not_found" => StatusCode::NOT_FOUND,
                    "access_denied" | "auth_failed" => StatusCode::FORBIDDEN,
                    "bad_request" => StatusCode::BAD_REQUEST,
                    "range_not_satisfiable" => StatusCode::RANGE_NOT_SATISFIABLE,
                    "too_large" => StatusCode::PAYLOAD_TOO_LARGE,
                    _ => StatusCode::BAD_GATEWAY,
                };
                Response::builder()
                    .status(status)
                    .header(header::CONTENT_TYPE, "text/plain; charset=utf-8")
                    .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
                    .body(e.message.into_bytes())
                    .unwrap()
            }
        };
        responder.respond(response);
    });
}

fn parse_path(request: &http::Request<Vec<u8>>) -> Result<(String, String, String), AppError> {
    let path = request.uri().path();
    let mut segs = path.trim_start_matches('/').splitn(3, '/');
    let server = segs.next().filter(|s| !s.is_empty());
    let share = segs.next().filter(|s| !s.is_empty());
    let rest = segs.next().unwrap_or("");
    let (Some(server), Some(share)) = (server, share) else {
        return Err(AppError::new("bad_request", "expected /server/share/path"));
    };
    let decode = |s: &str| percent_decode_str(s).decode_utf8_lossy().into_owned();
    let file_path = rest
        .split('/')
        .filter(|s| !s.is_empty())
        .map(decode)
        .collect::<Vec<_>>()
        .join("/");
    if file_path.is_empty() {
        return Err(AppError::new("bad_request", "missing file path"));
    }
    Ok((decode(server), decode(share), file_path))
}

pub async fn serve<R: Runtime>(app: &AppHandle<R>, request: &http::Request<Vec<u8>>) -> Result<Response<Vec<u8>>, AppError> {
    if request.method() == http::Method::OPTIONS {
        return Ok(Response::builder()
            .status(StatusCode::NO_CONTENT)
            .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
            .header(header::ACCESS_CONTROL_ALLOW_HEADERS, "range")
            .header(header::ACCESS_CONTROL_ALLOW_METHODS, "GET, HEAD, OPTIONS")
            .body(Vec::new())
            .unwrap());
    }
    let (server_id, share, path) = parse_path(request)?;
    log::info!("preview: {} {share}/{path} range={:?}", request.method(), request.headers().get(header::RANGE).and_then(|v| v.to_str().ok()));
    let state = app.state::<AppState>();
    let reader = state.preview.get_or_open(&state, &server_id, &share, &path).await?;
    let total = reader.size();
    let mime = mime_guess::from_path(&path).first_or_octet_stream().to_string();

    let mut builder = Response::builder()
        .header(header::CONTENT_TYPE, mime)
        .header(header::ACCEPT_RANGES, "bytes")
        .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
        .header(header::ACCESS_CONTROL_EXPOSE_HEADERS, "content-range, content-length, accept-ranges")
        .header(header::CACHE_CONTROL, "no-store");

    let range_header = request
        .headers()
        .get(header::RANGE)
        .and_then(|v| v.to_str().ok())
        .map(str::to_string);

    let (start, len, partial) = match range_header {
        Some(h) => {
            let ranges = http_range::HttpRange::parse(&h, total)
                .map_err(|_| AppError::new("range_not_satisfiable", "bad range"))?;
            let first = ranges.first().ok_or_else(|| AppError::new("range_not_satisfiable", "empty range"))?;
            (first.start, first.length.min(MAX_RANGE_BYTES), true)
        }
        None => {
            if total > MAX_FULL_BODY {
                return Err(AppError::new("too_large", "file too large to load without ranges"));
            }
            (0, total, false)
        }
    };

    let body = if request.method() == http::Method::HEAD || len == 0 {
        Vec::new()
    } else {
        reader.read_at(start, len).await?
    };
    let sent = body.len() as u64;

    if partial {
        let end = if sent == 0 { start } else { start + sent - 1 };
        builder = builder
            .status(StatusCode::PARTIAL_CONTENT)
            .header(header::CONTENT_RANGE, format!("bytes {start}-{end}/{total}"));
    } else {
        builder = builder.status(StatusCode::OK);
    }
    let content_length = if request.method() == http::Method::HEAD { if partial { len } else { total } } else { sent };
    Ok(builder
        .header(header::CONTENT_LENGTH, content_length.to_string())
        .body(body)
        .unwrap())
}
