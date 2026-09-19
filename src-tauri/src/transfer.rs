//! Transfers between the NAS and the local disk: downloads and uploads,
//! resumable, retried across connection loss, and persisted so an
//! interrupted transfer can be continued after the app restarts.
//!
//! Every transfer opens its own SMB connection so it never blocks browsing.
//! The engine (`run_transfer`) is UI-agnostic and reports through a callback;
//! the command layer forwards that to the webview as events and to the
//! on-disk transfer list.
//!
//! Resume model: a file in flight is written as `<name>.part` (locally for
//! downloads, remotely for uploads) and renamed when complete. On resume the
//! `.part` length is the offset to continue from, provided the source still
//! has the same size.

use std::collections::HashMap;
use std::io::SeekFrom;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use futures_util::future::join_all;
use serde::{Deserialize, Serialize};
use smb2::{SmbClient, Tree};
use tokio::io::{AsyncReadExt, AsyncSeekExt, AsyncWriteExt};

use crate::error::AppError;
use crate::smb::{join_path, ConnectionParams};

pub const PROGRESS_EVENT: &str = "transfer:progress";
const EMIT_INTERVAL: Duration = Duration::from_millis(80);
const CHUNK: u64 = 1024 * 1024;
const READ_WINDOW: usize = 4;
const MAX_ATTEMPTS: u32 = 6;
const PART_SUFFIX: &str = ".part";

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadItem {
    /// Share-relative path with `/` separators.
    pub path: String,
    pub name: String,
    pub is_dir: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TransferKind {
    Download,
    Upload,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TransferStatus {
    Queued,
    Scanning,
    Running,
    Done,
    Cancelled,
    Error,
    /// Gave up after repeated connection failures; can be resumed.
    Interrupted,
}

impl TransferStatus {
    pub fn is_active(self) -> bool {
        matches!(self, Self::Queued | Self::Scanning | Self::Running)
    }
    pub fn is_resumable(self) -> bool {
        matches!(self, Self::Cancelled | Self::Error | Self::Interrupted)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TransferProgress {
    pub task_id: String,
    #[serde(default = "default_kind")]
    pub kind: TransferKind,
    pub server_id: String,
    pub share: String,
    pub name: String,
    pub is_dir: bool,
    /// Download: source on the share. Upload: final destination on the share.
    pub remote_path: String,
    /// Download: final local destination. Upload: local source.
    pub local_path: String,
    /// Where the data ends up, for display (local path for downloads,
    /// share path for uploads).
    pub dest_path: String,
    pub status: TransferStatus,
    pub bytes_done: u64,
    pub bytes_total: u64,
    pub files_done: u32,
    pub files_total: u32,
    pub current_file: String,
    pub error: Option<String>,
    pub error_code: Option<String>,
    pub started_at: u64,
    #[serde(default)]
    pub updated_at: u64,
    pub speed_bps: u64,
    #[serde(default)]
    pub attempt: u32,
}

fn default_kind() -> TransferKind {
    TransferKind::Download
}

impl TransferProgress {
    fn base(task_id: String, kind: TransferKind, server_id: String, share: String, name: String, is_dir: bool) -> Self {
        Self {
            task_id,
            kind,
            server_id,
            share,
            name,
            is_dir,
            remote_path: String::new(),
            local_path: String::new(),
            dest_path: String::new(),
            status: TransferStatus::Queued,
            bytes_done: 0,
            bytes_total: 0,
            files_done: 0,
            files_total: 0,
            current_file: String::new(),
            error: None,
            error_code: None,
            started_at: now_ms(),
            updated_at: now_ms(),
            speed_bps: 0,
            attempt: 0,
        }
    }

    /// A download of `item` into `dest_dir`. The exact local name is decided
    /// when the transfer starts (see [`unique_destination`]).
    pub fn queued(task_id: String, server_id: String, share: String, item: &DownloadItem, dest_dir: &Path) -> Self {
        let mut p = Self::base(task_id, TransferKind::Download, server_id, share, item.name.clone(), item.is_dir);
        p.remote_path = item.path.clone();
        p.local_path = String::new();
        p.dest_path = dest_dir.to_string_lossy().into_owned();
        p
    }

    /// A download that must land exactly at `dest` (used by drag-out, where
    /// Finder has already chosen the file name).
    pub fn queued_exact(task_id: String, server_id: String, share: String, item: &DownloadItem, dest: &Path) -> Self {
        let mut p = Self::queued(task_id, server_id, share, item, dest.parent().unwrap_or(dest));
        p.local_path = dest.to_string_lossy().into_owned();
        p.dest_path = p.local_path.clone();
        p
    }

    /// An upload of the local file or folder at `local` into `remote_dir`.
    pub fn queued_upload(task_id: String, server_id: String, share: String, local: &Path, remote_dir: &str) -> Self {
        let name = local
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "upload".to_string());
        let is_dir = local.is_dir();
        let mut p = Self::base(task_id, TransferKind::Upload, server_id, share, name, is_dir);
        p.local_path = local.to_string_lossy().into_owned();
        p.remote_path = String::new();
        p.dest_path = remote_dir.trim_matches('/').to_string();
        p
    }

    /// Directory the download lands in (`dest_path` until the exact name is
    /// decided, then the parent of `local_path`).
    fn download_dir(&self) -> PathBuf {
        if self.local_path.is_empty() {
            PathBuf::from(&self.dest_path)
        } else {
            Path::new(&self.local_path)
                .parent()
                .map(Path::to_path_buf)
                .unwrap_or_else(|| PathBuf::from(&self.dest_path))
        }
    }
}

pub fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

// ── Cancellation flags ──────────────────────────────────────────────────

#[derive(Default)]
pub struct TransferManager {
    cancels: Mutex<HashMap<String, Arc<AtomicBool>>>,
}

impl TransferManager {
    fn lock(&self) -> std::sync::MutexGuard<'_, HashMap<String, Arc<AtomicBool>>> {
        self.cancels.lock().unwrap_or_else(|p| p.into_inner())
    }
    pub fn register(&self, task_id: &str) -> Arc<AtomicBool> {
        let flag = Arc::new(AtomicBool::new(false));
        self.lock().insert(task_id.to_string(), flag.clone());
        flag
    }
    pub fn cancel(&self, task_id: &str) -> bool {
        match self.lock().get(task_id) {
            Some(flag) => {
                flag.store(true, Ordering::Relaxed);
                true
            }
            None => false,
        }
    }
    pub fn is_running(&self, task_id: &str) -> bool {
        self.lock().contains_key(task_id)
    }
    pub fn finish(&self, task_id: &str) {
        self.lock().remove(task_id);
    }
}

// ── Persisted transfer list ─────────────────────────────────────────────

/// The transfer list on disk (`transfers.json`), newest first.
pub struct TransferStore {
    path: PathBuf,
    items: Mutex<Vec<TransferProgress>>,
    last_save: Mutex<Instant>,
}

impl TransferStore {
    pub fn load(path: PathBuf) -> Self {
        let mut items: Vec<TransferProgress> = std::fs::read(&path)
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default();
        // Anything still "running" when we were last alive was cut off by a
        // crash or quit; it is resumable, not running.
        for t in items.iter_mut() {
            if t.status.is_active() {
                t.status = TransferStatus::Interrupted;
                t.speed_bps = 0;
            }
        }
        Self {
            path,
            items: Mutex::new(items),
            last_save: Mutex::new(Instant::now() - Duration::from_secs(60)),
        }
    }

    pub fn all(&self) -> Vec<TransferProgress> {
        self.items.lock().unwrap_or_else(|p| p.into_inner()).clone()
    }

    pub fn get(&self, task_id: &str) -> Option<TransferProgress> {
        self.items
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .iter()
            .find(|t| t.task_id == task_id)
            .cloned()
    }

    /// Insert or replace; writes to disk immediately on status changes and at
    /// most every two seconds otherwise.
    pub fn upsert(&self, p: &TransferProgress) {
        let status_changed = {
            let mut items = self.items.lock().unwrap_or_else(|g| g.into_inner());
            match items.iter_mut().find(|t| t.task_id == p.task_id) {
                Some(existing) => {
                    let changed = existing.status != p.status;
                    *existing = p.clone();
                    changed
                }
                None => {
                    items.insert(0, p.clone());
                    true
                }
            }
        };
        let due = {
            let mut last = self.last_save.lock().unwrap_or_else(|g| g.into_inner());
            if status_changed || last.elapsed() >= Duration::from_secs(2) {
                *last = Instant::now();
                true
            } else {
                false
            }
        };
        if due {
            self.save();
        }
    }

    pub fn remove(&self, task_id: &str) -> Option<TransferProgress> {
        let removed = {
            let mut items = self.items.lock().unwrap_or_else(|g| g.into_inner());
            let idx = items.iter().position(|t| t.task_id == task_id)?;
            Some(items.remove(idx))
        };
        self.save();
        removed
    }

    pub fn retain(&self, keep: impl Fn(&TransferProgress) -> bool) -> Vec<TransferProgress> {
        let dropped = {
            let mut items = self.items.lock().unwrap_or_else(|g| g.into_inner());
            let (kept, dropped): (Vec<_>, Vec<_>) = items.drain(..).partition(|t| keep(t));
            *items = kept;
            dropped
        };
        self.save();
        dropped
    }

    fn save(&self) {
        let snapshot = self.all();
        if let Ok(json) = serde_json::to_vec_pretty(&snapshot) {
            if let Some(parent) = self.path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            let tmp = self.path.with_extension("json.tmp");
            if std::fs::write(&tmp, json).is_ok() {
                let _ = std::fs::rename(&tmp, &self.path);
            }
        }
    }
}

/// Delete the partial files a transfer left behind (local `.part` for
/// downloads; the remote `.part` is cleaned up lazily by the next run).
pub fn discard_partials(p: &TransferProgress) {
    if p.kind == TransferKind::Download && !p.local_path.is_empty() {
        let local = PathBuf::from(&p.local_path);
        if p.is_dir {
            remove_parts_in(&local);
        } else {
            let _ = std::fs::remove_file(part_path(&local));
        }
    }
}

fn remove_parts_in(dir: &Path) {
    if let Ok(rd) = std::fs::read_dir(dir) {
        for entry in rd.flatten() {
            let path = entry.path();
            if path.is_dir() {
                remove_parts_in(&path);
            } else if path.to_string_lossy().ends_with(PART_SUFFIX) {
                let _ = std::fs::remove_file(&path);
            }
        }
    }
}

// ── Reporting ───────────────────────────────────────────────────────────

struct Reporter<F: FnMut(&TransferProgress)> {
    progress: TransferProgress,
    on_progress: F,
    started: Instant,
    start_bytes: u64,
    last_emit: Option<Instant>,
}

impl<F: FnMut(&TransferProgress)> Reporter<F> {
    fn begin_attempt(&mut self) {
        self.started = Instant::now();
        self.start_bytes = self.progress.bytes_done;
        self.progress.speed_bps = 0;
    }

    fn emit(&mut self, force: bool) {
        let now = Instant::now();
        if !force {
            if let Some(last) = self.last_emit {
                if now.duration_since(last) < EMIT_INTERVAL {
                    return;
                }
            }
        }
        let elapsed = now.duration_since(self.started).as_secs_f64();
        if elapsed > 0.3 && self.progress.status == TransferStatus::Running {
            let moved = self.progress.bytes_done.saturating_sub(self.start_bytes);
            self.progress.speed_bps = (moved as f64 / elapsed) as u64;
        }
        self.progress.updated_at = now_ms();
        self.last_emit = Some(now);
        (self.on_progress)(&self.progress);
    }
}

fn check_cancel(cancel: &AtomicBool) -> Result<(), AppError> {
    if cancel.load(Ordering::Relaxed) {
        Err(AppError::cancelled())
    } else {
        Ok(())
    }
}

fn part_path(final_path: &Path) -> PathBuf {
    let mut s = final_path.as_os_str().to_owned();
    s.push(PART_SUFFIX);
    PathBuf::from(s)
}

/// Pick a destination that does not clobber an existing file or folder:
/// `photo.jpg`, `photo (1).jpg`, `photo (2).jpg`, ...
pub fn unique_destination(dir: &Path, name: &str) -> PathBuf {
    let candidate = dir.join(name);
    if !candidate.exists() && !part_path(&candidate).exists() {
        return candidate;
    }
    let (stem, ext) = split_name(name);
    for i in 1..10_000 {
        let candidate = dir.join(format!("{stem} ({i}){ext}"));
        if !candidate.exists() && !part_path(&candidate).exists() {
            return candidate;
        }
    }
    dir.join(format!("{stem} ({}){ext}", now_ms()))
}

fn split_name(name: &str) -> (String, String) {
    match name.rsplit_once('.') {
        Some((s, e)) if !s.is_empty() => (s.to_string(), format!(".{e}")),
        _ => (name.to_string(), String::new()),
    }
}

// ── Engine ──────────────────────────────────────────────────────────────

/// Run a transfer to completion, retrying across connection loss and
/// reporting through `on_progress`. The final callback always carries a
/// terminal status.
pub async fn run_transfer<F>(
    params: ConnectionParams,
    mut progress: TransferProgress,
    cancel: Arc<AtomicBool>,
    on_progress: F,
) -> TransferProgress
where
    F: FnMut(&TransferProgress) + Send,
{
    progress.status = TransferStatus::Scanning;
    progress.error = None;
    progress.error_code = None;
    progress.speed_bps = 0;
    let mut rep = Reporter {
        progress,
        on_progress,
        started: Instant::now(),
        start_bytes: 0,
        last_emit: None,
    };
    rep.emit(true);

    let mut attempt = 0u32;
    let outcome = loop {
        attempt += 1;
        rep.progress.attempt = attempt;
        rep.begin_attempt();
        let result = match rep.progress.kind {
            TransferKind::Download => download_once(&mut rep, &params, &cancel).await,
            TransferKind::Upload => upload_once(&mut rep, &params, &cancel).await,
        };
        match result {
            Ok(()) => break Ok(()),
            Err(e) if e.code == "cancelled" => break Err(e),
            Err(e) if e.is_connection_error() && attempt < MAX_ATTEMPTS => {
                log::warn!("transfer {}: attempt {attempt} failed ({e}); retrying", rep.progress.task_id);
                rep.progress.error = Some(e.message.clone());
                rep.progress.error_code = Some(e.code.clone());
                rep.progress.status = TransferStatus::Scanning;
                rep.emit(true);
                let backoff = Duration::from_secs(1 << attempt.min(4));
                let deadline = Instant::now() + backoff;
                while Instant::now() < deadline {
                    if cancel.load(Ordering::Relaxed) {
                        break;
                    }
                    tokio::time::sleep(Duration::from_millis(200)).await;
                }
                if cancel.load(Ordering::Relaxed) {
                    break Err(AppError::cancelled());
                }
            }
            Err(e) => break Err(e),
        }
    };

    match outcome {
        Ok(()) => {
            rep.progress.status = TransferStatus::Done;
            rep.progress.error = None;
            rep.progress.error_code = None;
        }
        Err(e) if e.code == "cancelled" => rep.progress.status = TransferStatus::Cancelled,
        Err(e) => {
            rep.progress.status = if e.is_connection_error() {
                TransferStatus::Interrupted
            } else {
                TransferStatus::Error
            };
            rep.progress.error = Some(e.message);
            rep.progress.error_code = Some(e.code);
        }
    }
    rep.progress.speed_bps = 0;
    rep.emit(true);
    rep.progress
}

// ── Download ────────────────────────────────────────────────────────────

struct FilePlan {
    remote: String,
    local: PathBuf,
    size: u64,
}

async fn download_once<F>(
    rep: &mut Reporter<F>,
    params: &ConnectionParams,
    cancel: &AtomicBool,
) -> Result<(), AppError>
where
    F: FnMut(&TransferProgress),
{
    let share = rep.progress.share.clone();
    let mut client = SmbClient::connect(params.client_config()).await?;
    let mut tree = client.connect_share(&share).await?;

    rep.progress.status = TransferStatus::Scanning;
    rep.emit(true);

    // Decide the local destination once; it is persisted for resumes.
    let dest_dir = rep.progress.download_dir();
    tokio::fs::create_dir_all(&dest_dir).await?;
    if rep.progress.local_path.is_empty() {
        let dest = unique_destination(&dest_dir, &rep.progress.name);
        rep.progress.local_path = dest.to_string_lossy().into_owned();
        rep.progress.dest_path = rep.progress.local_path.clone();
    }
    let dest_root = PathBuf::from(&rep.progress.local_path);

    // Plan every file with its size so skipping and resuming are exact.
    let mut plan: Vec<FilePlan> = Vec::new();
    if rep.progress.is_dir {
        let mut queue = vec![(rep.progress.remote_path.clone(), dest_root.clone())];
        while let Some((remote_dir, local_dir)) = queue.pop() {
            check_cancel(cancel)?;
            tokio::fs::create_dir_all(&local_dir).await?;
            let entries = client.list_directory(&mut tree, &remote_dir).await?;
            for e in entries {
                if e.name == "." || e.name == ".." {
                    continue;
                }
                let remote = join_path(&remote_dir, &e.name);
                let local = local_dir.join(&e.name);
                if e.is_directory {
                    queue.push((remote, local));
                } else {
                    plan.push(FilePlan { remote, local, size: e.size });
                }
            }
        }
    } else {
        let info = client.stat(&mut tree, &rep.progress.remote_path).await?;
        plan.push(FilePlan {
            remote: rep.progress.remote_path.clone(),
            local: dest_root.clone(),
            size: info.size,
        });
    }
    plan.sort_by(|a, b| a.remote.cmp(&b.remote));

    rep.progress.files_total = plan.len() as u32;
    rep.progress.bytes_total = plan.iter().map(|f| f.size).sum();
    rep.progress.bytes_done = 0;
    rep.progress.files_done = 0;
    rep.progress.status = TransferStatus::Running;
    rep.begin_attempt();
    rep.emit(true);

    for file in &plan {
        check_cancel(cancel)?;
        rep.progress.current_file = file.remote.clone();

        // Already complete from an earlier run?
        if let Ok(meta) = tokio::fs::metadata(&file.local).await {
            if meta.is_file() && meta.len() == file.size {
                rep.progress.bytes_done += file.size;
                rep.progress.files_done += 1;
                rep.emit(false);
                continue;
            }
        }

        let part = part_path(&file.local);
        let mut offset = match tokio::fs::metadata(&part).await {
            Ok(meta) if meta.is_file() && meta.len() <= file.size => meta.len(),
            _ => 0,
        };

        let reader = client.open_file_reader(&tree, &file.remote).await?;
        if reader.size() != file.size {
            // The source changed since we planned; start this file over.
            offset = 0;
        }
        let mut out = tokio::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(offset == 0)
            .open(&part)
            .await?;
        out.set_len(offset).await?;
        out.seek(SeekFrom::Start(offset)).await?;
        rep.progress.bytes_done += offset;
        rep.emit(false);

        let total = reader.size();
        let mut pos = offset;
        let result: Result<(), AppError> = async {
            while pos < total {
                check_cancel(cancel)?;
                let mut reads = Vec::with_capacity(READ_WINDOW);
                let mut cursor = pos;
                while cursor < total && reads.len() < READ_WINDOW {
                    let len = CHUNK.min(total - cursor);
                    reads.push(reader.read_at(cursor, len));
                    cursor += len;
                }
                for chunk in join_all(reads).await {
                    let bytes = chunk?;
                    if bytes.is_empty() {
                        return Err(AppError::new("smb_error", "server returned no data before end of file"));
                    }
                    out.write_all(&bytes).await?;
                    pos += bytes.len() as u64;
                    rep.progress.bytes_done += bytes.len() as u64;
                    rep.emit(false);
                }
            }
            Ok(())
        }
        .await;
        out.flush().await?;
        drop(out);
        let _ = reader.close().await;
        result?; // partial `.part` stays on disk for a later resume

        tokio::fs::rename(&part, &file.local).await?;
        rep.progress.files_done += 1;
        rep.emit(false);
    }
    Ok(())
}

// ── Upload ──────────────────────────────────────────────────────────────

struct UploadPlan {
    local: PathBuf,
    remote: String,
    size: u64,
}

fn walk_local(root: &Path, remote_root: &str) -> Result<(Vec<UploadPlan>, Vec<String>), AppError> {
    let mut files = Vec::new();
    let mut dirs = Vec::new();
    let mut stack = vec![(root.to_path_buf(), remote_root.to_string())];
    while let Some((dir, remote_dir)) = stack.pop() {
        dirs.push(remote_dir.clone());
        let mut entries: Vec<_> = std::fs::read_dir(&dir)?.flatten().collect();
        entries.sort_by_key(|e| e.file_name());
        for entry in entries {
            let name = entry.file_name().to_string_lossy().into_owned();
            if name == ".DS_Store" {
                continue;
            }
            let path = entry.path();
            let meta = entry.metadata()?;
            let remote = join_path(&remote_dir, &name);
            if meta.is_dir() {
                stack.push((path, remote));
            } else if meta.is_file() {
                files.push(UploadPlan { local: path, remote, size: meta.len() });
            }
        }
    }
    Ok((files, dirs))
}

async fn remote_exists(client: &mut SmbClient, tree: &mut Tree, path: &str) -> Result<Option<smb2::FileInfo>, AppError> {
    match client.stat(tree, path).await {
        Ok(info) => Ok(Some(info)),
        Err(e) if e.kind() == smb2::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e.into()),
    }
}

async fn unique_remote(client: &mut SmbClient, tree: &mut Tree, dir: &str, name: &str) -> Result<String, AppError> {
    let candidate = join_path(dir, name);
    if remote_exists(client, tree, &candidate).await?.is_none() {
        return Ok(candidate);
    }
    let (stem, ext) = split_name(name);
    for i in 1..10_000 {
        let candidate = join_path(dir, &format!("{stem} ({i}){ext}"));
        if remote_exists(client, tree, &candidate).await?.is_none() {
            return Ok(candidate);
        }
    }
    Ok(join_path(dir, &format!("{stem} ({}){ext}", now_ms())))
}

async fn ensure_remote_dir(client: &mut SmbClient, tree: &mut Tree, path: &str) -> Result<(), AppError> {
    if path.is_empty() {
        return Ok(());
    }
    match client.create_directory(tree, path).await {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == smb2::ErrorKind::AlreadyExists => Ok(()),
        Err(e) => Err(e.into()),
    }
}

async fn upload_once<F>(
    rep: &mut Reporter<F>,
    params: &ConnectionParams,
    cancel: &AtomicBool,
) -> Result<(), AppError>
where
    F: FnMut(&TransferProgress),
{
    let share = rep.progress.share.clone();
    let local_root = PathBuf::from(&rep.progress.local_path);
    if !local_root.exists() {
        return Err(AppError::new("not_found", format!("{} no longer exists", local_root.display())));
    }
    let mut client = SmbClient::connect(params.client_config()).await?;
    let mut tree = client.connect_share(&share).await?;

    rep.progress.status = TransferStatus::Scanning;
    rep.emit(true);

    // Decide the remote destination once; persisted for resumes.
    let remote_dir = rep.progress.dest_path.trim_matches('/').to_string();
    if rep.progress.remote_path.is_empty() {
        rep.progress.remote_path = unique_remote(&mut client, &mut tree, &remote_dir, &rep.progress.name).await?;
    }
    let remote_root = rep.progress.remote_path.clone();

    let (plan, dirs) = if rep.progress.is_dir {
        walk_local(&local_root, &remote_root)?
    } else {
        let size = std::fs::metadata(&local_root)?.len();
        (vec![UploadPlan { local: local_root.clone(), remote: remote_root.clone(), size }], Vec::new())
    };
    for dir in &dirs {
        check_cancel(cancel)?;
        ensure_remote_dir(&mut client, &mut tree, dir).await?;
    }

    rep.progress.files_total = plan.len() as u32;
    rep.progress.bytes_total = plan.iter().map(|f| f.size).sum();
    rep.progress.bytes_done = 0;
    rep.progress.files_done = 0;
    rep.progress.status = TransferStatus::Running;
    rep.begin_attempt();
    rep.emit(true);

    for file in &plan {
        check_cancel(cancel)?;
        rep.progress.current_file = file.remote.clone();

        if let Some(info) = remote_exists(&mut client, &mut tree, &file.remote).await? {
            if !info.is_directory && info.size == file.size {
                rep.progress.bytes_done += file.size;
                rep.progress.files_done += 1;
                rep.emit(false);
                continue;
            }
        }

        let remote_part = format!("{}{}", file.remote, PART_SUFFIX);
        let offset = match remote_exists(&mut client, &mut tree, &remote_part).await? {
            Some(info) if !info.is_directory && info.size <= file.size => info.size,
            _ => 0,
        };

        let mut src = tokio::fs::File::open(&file.local).await?;
        src.seek(SeekFrom::Start(offset)).await?;
        let mut writer = if offset > 0 {
            client.create_file_writer_at(&tree, &remote_part, offset).await?
        } else {
            client.create_file_writer(&tree, &remote_part).await?
        };
        rep.progress.bytes_done += offset;
        rep.emit(false);

        let mut buf = vec![0u8; CHUNK as usize];
        let mut pos = offset;
        let result: Result<(), AppError> = async {
            while pos < file.size {
                check_cancel(cancel)?;
                let want = (file.size - pos).min(CHUNK) as usize;
                let n = src.read(&mut buf[..want]).await?;
                if n == 0 {
                    return Err(AppError::new("io_error", "local file shrank while uploading"));
                }
                writer.write_chunk(&buf[..n]).await?;
                pos += n as u64;
                rep.progress.bytes_done += n as u64;
                rep.emit(false);
            }
            Ok(())
        }
        .await;
        // Finishing flushes what was sent so a cancelled or failed upload can
        // resume from the confirmed length.
        let finished = writer.finish().await;
        result?;
        finished?;

        client.rename(&mut tree, &remote_part, &file.remote).await?;
        rep.progress.files_done += 1;
        rep.emit(false);
    }
    Ok(())
}
