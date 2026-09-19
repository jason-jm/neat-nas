//! Tauri command surface. Every function here is invoked from the webview.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, State};
use tokio::sync::Mutex;

use crate::config::SavedServer;
use crate::creds;
use crate::discovery::{self, DiscoveredServer};
use crate::error::AppError;
use crate::smb::{self, ConnectionInfo, ConnectionParams, EntryDto, Session, DEFAULT_PORT};
use crate::thumbs;
use crate::transfer::{self, DownloadItem, TransferProgress, TransferStatus, PROGRESS_EVENT};
use crate::AppState;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerInput {
    pub name: Option<String>,
    pub host: String,
    pub port: Option<u16>,
    pub username: String,
    pub password: String,
    pub domain: Option<String>,
}

impl ServerInput {
    fn params(&self) -> ConnectionParams {
        ConnectionParams {
            host: self.host.trim().to_string(),
            port: self.port.unwrap_or(DEFAULT_PORT),
            username: self.username.trim().to_string(),
            password: self.password.clone(),
            domain: self.domain.clone().unwrap_or_default().trim().to_string(),
        }
    }

    fn display_name(&self) -> String {
        self.name
            .as_deref()
            .map(str::trim)
            .filter(|n| !n.is_empty())
            .unwrap_or_else(|| self.host.trim())
            .to_string()
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    pub download_dir: String,
    pub download_dir_is_custom: bool,
    pub config_path: String,
    pub platform: String,
    /// Dev-only scripted UI steps from `NEATNAS_DEV_AUTOPILOT`, used to
    /// screenshot the real app without a human at the mouse.
    pub dev_autopilot: Option<String>,
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

fn not_found(what: &str) -> AppError {
    AppError::new("not_found", format!("{what} not found"))
}

pub(crate) async fn params_for_server(state: &AppState, server_id: &str) -> Result<(SavedServer, ConnectionParams), AppError> {
    let server = state.config.server(server_id).ok_or_else(|| not_found("Server"))?;
    let cfg = state.config.clone();
    let id = server_id.to_string();
    let password = tokio::task::spawn_blocking(move || creds::load_password(&cfg, &id))
        .await??
        .ok_or_else(|| AppError::new("no_credentials", "No saved password for this server"))?;
    let params = ConnectionParams {
        host: server.host.clone(),
        port: server.port,
        username: server.username.clone(),
        password,
        domain: server.domain.clone(),
    };
    Ok((server, params))
}

pub(crate) async fn session_for(state: &AppState, server_id: &str) -> Result<Arc<Mutex<Session>>, AppError> {
    if let Some(session) = state.pool.get(server_id) {
        let alive = !session.lock().await.client.is_disconnected();
        if alive {
            return Ok(session);
        }
        state.pool.remove(server_id);
        state.preview.drop_server(server_id).await;
    }
    let (_, params) = params_for_server(state, server_id).await?;
    let client = smb::connect(&params).await?;
    Ok(state.pool.insert(server_id, Session::new(client)))
}

// ── Servers ─────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn list_servers(state: State<'_, AppState>) -> Result<Vec<SavedServer>, AppError> {
    Ok(state.config.snapshot().servers)
}

#[tauri::command]
pub async fn test_connection(input: ServerInput) -> Result<ConnectionInfo, AppError> {
    let params = input.params();
    let client = smb::connect(&params).await?;
    let mut session = Session::new(client);
    let dialect = session.dialect();
    let (shares, shares_error) = match session.list_shares().await {
        Ok(shares) => (shares, None),
        Err(e) => (Vec::new(), Some(e.message)),
    };
    Ok(ConnectionInfo {
        dialect,
        shares,
        shares_error,
    })
}

#[tauri::command]
pub async fn add_server(state: State<'_, AppState>, input: ServerInput) -> Result<SavedServer, AppError> {
    let params = input.params();
    let client = smb::connect(&params).await?;
    let id = uuid::Uuid::new_v4().to_string();
    let server = SavedServer {
        id: id.clone(),
        name: input.display_name(),
        host: params.host.clone(),
        port: params.port,
        username: params.username.clone(),
        domain: params.domain.clone(),
        last_share: None,
        added_at: now_ms(),
    };
    let cfg = state.config.clone();
    let (id_for_task, password) = (id.clone(), params.password.clone());
    tokio::task::spawn_blocking(move || creds::save_password(&cfg, &id_for_task, &password)).await??;
    state.config.update(|c| c.servers.push(server.clone()))?;
    state.pool.insert(&id, Session::new(client));
    Ok(server)
}

#[tauri::command]
pub async fn update_server(
    state: State<'_, AppState>,
    id: String,
    input: ServerInput,
) -> Result<SavedServer, AppError> {
    let existing = state.config.server(&id).ok_or_else(|| not_found("Server"))?;
    let mut params = input.params();
    // An empty password in an edit means "keep the stored one".
    if params.password.is_empty() {
        let cfg = state.config.clone();
        let id2 = id.clone();
        params.password = tokio::task::spawn_blocking(move || creds::load_password(&cfg, &id2))
            .await??
            .unwrap_or_default();
    }
    let client = smb::connect(&params).await?;
    let updated = SavedServer {
        id: id.clone(),
        name: input.display_name(),
        host: params.host.clone(),
        port: params.port,
        username: params.username.clone(),
        domain: params.domain.clone(),
        last_share: existing.last_share,
        added_at: existing.added_at,
    };
    let cfg = state.config.clone();
    let (id_for_task, password) = (id.clone(), params.password.clone());
    tokio::task::spawn_blocking(move || creds::save_password(&cfg, &id_for_task, &password)).await??;
    state.config.update(|c| {
        if let Some(s) = c.servers.iter_mut().find(|s| s.id == id) {
            *s = updated.clone();
        }
    })?;
    state.preview.drop_server(&id).await;
    state.pool.insert(&id, Session::new(client));
    Ok(updated)
}

#[tauri::command]
pub async fn remove_server(state: State<'_, AppState>, id: String) -> Result<(), AppError> {
    state.pool.remove(&id);
    state.preview.drop_server(&id).await;
    state.config.update(|c| c.servers.retain(|s| s.id != id))?;
    let cfg = state.config.clone();
    tokio::task::spawn_blocking(move || creds::delete_password(&cfg, &id)).await??;
    Ok(())
}

#[tauri::command]
pub async fn disconnect_server(state: State<'_, AppState>, server_id: String) -> Result<(), AppError> {
    state.pool.remove(&server_id);
    state.preview.drop_server(&server_id).await;
    Ok(())
}

#[tauri::command]
pub async fn remember_share(
    state: State<'_, AppState>,
    server_id: String,
    share: Option<String>,
) -> Result<(), AppError> {
    state.config.update(|c| {
        if let Some(s) = c.servers.iter_mut().find(|s| s.id == server_id) {
            s.last_share = share;
        }
    })?;
    Ok(())
}

// ── Browsing ────────────────────────────────────────────────────────────

/// Connects (or reuses the session) and enumerates shares. Share
/// enumeration failing on an otherwise healthy session is reported in
/// `shares_error` rather than as a hard error, so the UI can still offer
/// "open share by name".
#[tauri::command]
pub async fn list_shares(state: State<'_, AppState>, server_id: String) -> Result<ConnectionInfo, AppError> {
    let mut retried = false;
    loop {
        let session = session_for(&state, &server_id).await?;
        let mut guard = session.lock().await;
        let dialect = guard.dialect();
        let result = guard.list_shares().await;
        drop(guard);
        match result {
            Ok(shares) => {
                return Ok(ConnectionInfo {
                    dialect,
                    shares,
                    shares_error: None,
                })
            }
            Err(e) if !retried && e.is_connection_error() => {
                state.pool.remove(&server_id);
                retried = true;
            }
            Err(e) if e.is_connection_error() => return Err(e),
            Err(e) => {
                return Ok(ConnectionInfo {
                    dialect,
                    shares: Vec::new(),
                    shares_error: Some(e.message),
                })
            }
        }
    }
}

#[tauri::command]
pub async fn list_dir(
    state: State<'_, AppState>,
    server_id: String,
    share: String,
    path: String,
) -> Result<Vec<EntryDto>, AppError> {
    let mut retried = false;
    let started = std::time::Instant::now();
    loop {
        let session = session_for(&state, &server_id).await?;
        let result = session.lock().await.list_dir(&share, &path).await;
        match result {
            Err(e) if !retried && e.is_connection_error() => {
                state.pool.remove(&server_id);
                retried = true;
            }
            Ok(entries) => {
                log::info!("list_dir: {share}/{path} -> {} entries in {} ms", entries.len(), started.elapsed().as_millis());
                return Ok(entries);
            }
            Err(e) => {
                log::warn!("list_dir: {share}/{path} failed after {} ms: {e}", started.elapsed().as_millis());
                return Err(e);
            }
        }
    }
}

// ── Transfers ───────────────────────────────────────────────────────────

fn effective_download_dir(app: &AppHandle, state: &AppState) -> Result<(PathBuf, bool), AppError> {
    if let Some(custom) = state.config.snapshot().download_dir.filter(|d| !d.trim().is_empty()) {
        return Ok((PathBuf::from(custom), true));
    }
    let default = app
        .path()
        .download_dir()
        .or_else(|_| app.path().home_dir())?;
    Ok((default, false))
}

/// Register, persist, announce and run a transfer in the background.
pub(crate) fn spawn_transfer(app: &AppHandle, params: ConnectionParams, progress: TransferProgress) {
    let state = app.state::<AppState>();
    let cancel = state.transfers.register(&progress.task_id);
    state.store.upsert(&progress);
    let _ = app.emit(PROGRESS_EVENT, progress.clone());

    let app_for_task = app.clone();
    tauri::async_runtime::spawn(async move {
        let emitter = app_for_task.clone();
        let final_state = transfer::run_transfer(params, progress, cancel, move |p| {
            let _ = emitter.emit(PROGRESS_EVENT, p.clone());
            if let Some(s) = emitter.try_state::<AppState>() {
                s.store.upsert(p);
            }
        })
        .await;
        if let Some(s) = app_for_task.try_state::<AppState>() {
            s.transfers.finish(&final_state.task_id);
            s.store.upsert(&final_state);
        }
    });
}

#[tauri::command]
pub async fn start_download(
    app: AppHandle,
    state: State<'_, AppState>,
    server_id: String,
    share: String,
    items: Vec<DownloadItem>,
    dest_dir: Option<String>,
) -> Result<Vec<TransferProgress>, AppError> {
    let dest_dir = match dest_dir.filter(|d| !d.trim().is_empty()) {
        Some(d) => PathBuf::from(d),
        None => effective_download_dir(&app, &state)?.0,
    };
    let (_, params) = params_for_server(&state, &server_id).await?;
    let mut queued = Vec::with_capacity(items.len());
    for item in items {
        let task_id = uuid::Uuid::new_v4().to_string();
        let progress = TransferProgress::queued(task_id, server_id.clone(), share.clone(), &item, &dest_dir);
        queued.push(progress.clone());
        spawn_transfer(&app, params.clone(), progress);
    }
    Ok(queued)
}

#[tauri::command]
pub async fn start_upload(
    app: AppHandle,
    state: State<'_, AppState>,
    server_id: String,
    share: String,
    dest_dir: String,
    paths: Vec<String>,
) -> Result<Vec<TransferProgress>, AppError> {
    let (_, params) = params_for_server(&state, &server_id).await?;
    let mut queued = Vec::with_capacity(paths.len());
    for path in paths {
        let local = PathBuf::from(&path);
        if !local.exists() {
            return Err(AppError::new("not_found", format!("{path} does not exist")));
        }
        let task_id = uuid::Uuid::new_v4().to_string();
        let progress = TransferProgress::queued_upload(task_id, server_id.clone(), share.clone(), &local, &dest_dir);
        queued.push(progress.clone());
        spawn_transfer(&app, params.clone(), progress);
    }
    Ok(queued)
}

#[tauri::command]
pub async fn cancel_transfer(state: State<'_, AppState>, task_id: String) -> Result<bool, AppError> {
    Ok(state.transfers.cancel(&task_id))
}

#[tauri::command]
pub async fn resume_transfer(app: AppHandle, state: State<'_, AppState>, task_id: String) -> Result<TransferProgress, AppError> {
    if state.transfers.is_running(&task_id) {
        return Err(AppError::new("already_running", "transfer is already running"));
    }
    let mut record = state.store.get(&task_id).ok_or_else(|| not_found("Transfer"))?;
    if !record.status.is_resumable() {
        return Err(AppError::new("not_resumable", "transfer cannot be resumed"));
    }
    let (_, params) = params_for_server(&state, &record.server_id).await?;
    record.status = TransferStatus::Queued;
    record.error = None;
    record.error_code = None;
    record.speed_bps = 0;
    spawn_transfer(&app, params, record.clone());
    Ok(record)
}

#[tauri::command]
pub async fn remove_transfer(state: State<'_, AppState>, task_id: String) -> Result<(), AppError> {
    if state.transfers.is_running(&task_id) {
        state.transfers.cancel(&task_id);
        return Err(AppError::new("still_running", "cancel the transfer first"));
    }
    if let Some(record) = state.store.remove(&task_id) {
        transfer::discard_partials(&record);
    }
    Ok(())
}

#[tauri::command]
pub async fn list_transfers(state: State<'_, AppState>) -> Result<Vec<TransferProgress>, AppError> {
    Ok(state.store.all())
}

#[tauri::command]
pub async fn clear_finished_transfers(state: State<'_, AppState>) -> Result<(), AppError> {
    state.store.retain(|t| t.status != TransferStatus::Done);
    Ok(())
}

// ── Thumbnails, drag-out ────────────────────────────────────────────────

#[tauri::command]
pub async fn thumbnail(
    app: AppHandle,
    state: State<'_, AppState>,
    server_id: String,
    share: String,
    path: String,
    size: u64,
    mtime: u64,
) -> Result<String, AppError> {
    let name = path.rsplit('/').next().unwrap_or(&path).to_string();
    if !thumbs::is_thumbnailable(&name) {
        return Err(AppError::new("no_thumbnail", "not an image"));
    }
    let cache_dir = app.path().app_cache_dir()?;
    let file = thumbs::cache_path(&cache_dir, &server_id, &share, &path, size, mtime);
    if let Ok(bytes) = tokio::fs::read(&file).await {
        return Ok(thumbs::to_data_url(&bytes));
    }
    let _permit = state
        .thumbs
        .sem
        .acquire()
        .await
        .map_err(|_| AppError::new("internal_error", "thumbnail limiter closed"))?;
    let session = session_for(&state, &server_id).await?;
    let reader = session.lock().await.open_reader(&share, &path).await?;
    let built = thumbs::build(&reader, &name).await;
    log::info!("thumbnail: {share}/{path} -> {}", match &built { Ok(b) => format!("{} bytes", b.len()), Err(e) => e.code.clone() });
    let _ = reader.close().await;
    let jpeg = built?;
    if let Some(parent) = file.parent() {
        let _ = tokio::fs::create_dir_all(parent).await;
    }
    let _ = tokio::fs::write(&file, &jpeg).await;
    Ok(thumbs::to_data_url(&jpeg))
}

#[tauri::command]
pub async fn start_drag_out(
    window: tauri::Window,
    state: State<'_, AppState>,
    server_id: String,
    share: String,
    items: Vec<DownloadItem>,
) -> Result<(), AppError> {
    let (_, params) = params_for_server(&state, &server_id).await?;
    #[cfg(target_os = "macos")]
    {
        let app = window.app_handle().clone();
        crate::drag::start(window, app, params, server_id, share, items)
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (window, params, server_id, share, items);
        Err(AppError::new("unsupported", "drag out is only implemented on macOS"))
    }
}

/// Frontend errors land in the Rust log so `tauri dev` shows them.
#[tauri::command]
pub async fn frontend_log(level: String, message: String) -> Result<(), AppError> {
    match level.as_str() {
        "error" => log::error!(target: "webview", "{message}"),
        _ => log::warn!(target: "webview", "{message}"),
    }
    Ok(())
}

// ── Discovery & settings ────────────────────────────────────────────────

#[tauri::command]
pub async fn discover_servers(timeout_ms: Option<u64>) -> Result<Vec<DiscoveredServer>, AppError> {
    let timeout = Duration::from_millis(timeout_ms.unwrap_or(2500).clamp(300, 15_000));
    discovery::discover(timeout).await
}

#[tauri::command]
pub async fn get_settings(app: AppHandle, state: State<'_, AppState>) -> Result<Settings, AppError> {
    let (dir, is_custom) = effective_download_dir(&app, &state)?;
    Ok(Settings {
        download_dir: dir.to_string_lossy().into_owned(),
        download_dir_is_custom: is_custom,
        config_path: state.config.path().to_string_lossy().into_owned(),
        platform: std::env::consts::OS.to_string(),
        dev_autopilot: std::env::var("NEATNAS_DEV_AUTOPILOT").ok().filter(|s| !s.is_empty()),
    })
}

#[tauri::command]
pub async fn set_download_dir(state: State<'_, AppState>, path: Option<String>) -> Result<(), AppError> {
    state.config.update(|c| c.download_dir = path.filter(|p| !p.trim().is_empty()))?;
    Ok(())
}
