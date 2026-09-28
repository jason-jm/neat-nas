//! Windows only: drag files from the app into Explorer (or any drop target)
//! as virtual files. Nothing is downloaded before the drop; Explorer then
//! reads each file from the NAS through us and shows its own progress,
//! conflict and error dialogs. See the `win-drag` crate for the COM side.

#![cfg(windows)]

use std::collections::BTreeMap;
use std::io::Read;
use std::sync::Arc;

use futures_util::future::join_all;
use smb2::{SmbClient, Tree};
use tauri::{AppHandle, Emitter, Window};
use tokio::sync::{mpsc, Mutex, MutexGuard, OnceCell};
use win_drag::{Entry, Preview, Source, MAX_PATH_LEN};

use crate::error::AppError;
use crate::smb::{join_path, ConnectionParams};
use crate::transfer::DownloadItem;

/// Emitted with the number of items left out because their path is too long
/// for a Windows file descriptor.
pub const SKIPPED_EVENT: &str = "drag:skipped";

const CHUNK: u64 = 1024 * 1024;
/// Reads in flight per file, as in the download engine.
const READ_WINDOW: usize = 4;
/// Chunks buffered ahead of the drop target's reads.
const READ_AHEAD: usize = 8;

type Listing = Result<Vec<Entry>, String>;

struct SmbSource {
    app: AppHandle,
    params: ConnectionParams,
    share: String,
    items: Vec<DownloadItem>,
    conn: Mutex<Option<(SmbClient, Tree)>>,
    listing: OnceCell<Listing>,
}

fn too_long(path: &str) -> bool {
    path.encode_utf16().count() > MAX_PATH_LEN
}

fn parent_of(path: &str) -> &str {
    path.trim_matches('/').rsplit_once('/').map_or("", |(parent, _)| parent)
}

impl SmbSource {
    async fn connection(&self) -> Result<MutexGuard<'_, Option<(SmbClient, Tree)>>, String> {
        let mut guard = self.conn.lock().await;
        if guard.is_none() {
            let mut client = SmbClient::connect(self.params.client_config()).await.map_err(|e| e.to_string())?;
            let tree = client.connect_share(&self.share).await.map_err(|e| e.to_string())?;
            *guard = Some((client, tree));
        }
        Ok(guard)
    }

    async fn listing(&self) -> Listing {
        self.listing.get_or_init(|| self.list()).await.clone()
    }

    /// Every folder and file under the dragged items, parents first.
    async fn list(&self) -> Listing {
        let mut guard = self.connection().await?;
        let (client, tree) = guard.as_mut().expect("connected above");
        let mut entries = Vec::new();
        let mut skipped = 0usize;

        // Sizes and dates of the dragged files come from their folder's listing.
        let mut by_parent: BTreeMap<&str, Vec<&DownloadItem>> = BTreeMap::new();
        for item in &self.items {
            by_parent.entry(parent_of(&item.path)).or_default().push(item);
        }
        for (parent, items) in by_parent {
            let siblings = client.list_directory(tree, parent).await.map_err(|e| e.to_string())?;
            for item in items {
                if too_long(&item.name) {
                    skipped += 1;
                    continue;
                }
                if !item.is_dir {
                    let found = siblings.iter().find(|e| e.name == item.name);
                    entries.push(Entry {
                        path: item.name.clone(),
                        is_dir: false,
                        size: found.map_or(0, |e| e.size),
                        modified: found.and_then(|e| e.modified.to_system_time()),
                        key: item.path.clone(),
                    });
                    continue;
                }
                entries.push(Entry { path: item.name.clone(), is_dir: true, size: 0, modified: None, key: item.path.clone() });
                let mut pending = vec![(item.path.trim_matches('/').to_string(), item.name.clone())];
                while let Some((remote_dir, rel_dir)) = pending.pop() {
                    let children = client.list_directory(tree, &remote_dir).await.map_err(|e| e.to_string())?;
                    for child in children {
                        if child.name == "." || child.name == ".." {
                            continue;
                        }
                        let rel = format!("{rel_dir}\\{}", child.name);
                        if too_long(&rel) {
                            skipped += 1;
                            continue;
                        }
                        let remote = join_path(&remote_dir, &child.name);
                        if child.is_directory {
                            entries.push(Entry { path: rel.clone(), is_dir: true, size: 0, modified: None, key: remote.clone() });
                            pending.push((remote, rel));
                        } else {
                            entries.push(Entry {
                                path: rel,
                                is_dir: false,
                                size: child.size,
                                modified: child.modified.to_system_time(),
                                key: remote,
                            });
                        }
                    }
                }
            }
        }
        if skipped > 0 {
            log::warn!("drag-out: left out {skipped} item(s) with paths too long for Explorer");
            let _ = self.app.emit(SKIPPED_EVENT, skipped);
        }
        log::info!("drag-out: {} entries ready", entries.len());
        Ok(entries)
    }

    async fn open_reader(&self, path: &str) -> Result<smb2::FileReader, String> {
        let mut guard = self.connection().await?;
        let (client, tree) = guard.as_ref().expect("connected above");
        match client.open_file_reader(tree, path).await {
            Ok(reader) => Ok(reader),
            Err(e) => {
                // Start from a fresh connection if Explorer retries.
                *guard = None;
                Err(e.to_string())
            }
        }
    }
}

impl Source for SmbSource {
    fn entries(&self) -> Listing {
        tauri::async_runtime::block_on(self.listing())
    }

    fn open(&self, entry: &Entry, offset: u64) -> Result<Box<dyn Read + Send>, String> {
        let reader = tauri::async_runtime::block_on(self.open_reader(&entry.key)).map_err(|e| {
            log::warn!("drag-out: cannot open {}: {e}", entry.key);
            e
        })?;
        let (tx, rx) = mpsc::channel(READ_AHEAD);
        let name = entry.key.clone();
        tauri::async_runtime::spawn(async move {
            let total = reader.size();
            let mut pos = offset;
            'file: while pos < total {
                let mut reads = Vec::with_capacity(READ_WINDOW);
                let mut cursor = pos;
                while cursor < total && reads.len() < READ_WINDOW {
                    let len = CHUNK.min(total - cursor);
                    reads.push(reader.read_at(cursor, len));
                    cursor += len;
                }
                for chunk in join_all(reads).await {
                    let chunk = match chunk {
                        Ok(bytes) if !bytes.is_empty() => {
                            pos += bytes.len() as u64;
                            Ok(bytes)
                        }
                        Ok(_) => Err("the NAS returned no data before the end of the file".to_string()),
                        Err(e) => Err(e.to_string()),
                    };
                    if let Err(e) = &chunk {
                        log::warn!("drag-out: reading {name} failed: {e}");
                    }
                    let failed = chunk.is_err();
                    // A closed channel means the drop target stopped reading.
                    if tx.send(chunk).await.is_err() || failed {
                        break 'file;
                    }
                }
            }
            let _ = reader.close().await;
        });
        Ok(Box::new(ChannelReader { rx, chunk: Vec::new(), at: 0 }))
    }

    fn copy_started(&self) {
        log::info!("drag-out: Explorer started copying");
    }

    fn copy_ended(&self, ok: bool) {
        log::info!("drag-out: Explorer finished copying (ok: {ok})");
    }
}

/// The file's bytes as they arrive from the reading task. Only used on COM
/// worker threads, never inside the async runtime.
struct ChannelReader {
    rx: mpsc::Receiver<Result<Vec<u8>, String>>,
    chunk: Vec<u8>,
    at: usize,
}

impl Read for ChannelReader {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        while self.at == self.chunk.len() {
            match self.rx.blocking_recv() {
                Some(Ok(bytes)) => {
                    self.chunk = bytes;
                    self.at = 0;
                }
                Some(Err(e)) => return Err(std::io::Error::other(e)),
                None => return Ok(0),
            }
        }
        let n = buf.len().min(self.chunk.len() - self.at);
        buf[..n].copy_from_slice(&self.chunk[self.at..self.at + n]);
        self.at += n;
        Ok(n)
    }
}

/// Begin a drag of `items`. Must be called while the mouse button is down
/// (the webview reports the gesture; the drag takes over from there).
pub fn start(window: Window, app: AppHandle, params: ConnectionParams, share: String, items: Vec<DownloadItem>) -> Result<(), AppError> {
    if items.is_empty() {
        return Ok(());
    }
    let preview: Vec<Preview> = items.iter().map(|i| Preview { name: i.name.clone(), is_dir: i.is_dir }).collect();
    let source = Arc::new(SmbSource { app, params, share, items, conn: Mutex::new(None), listing: OnceCell::new() });
    // List folders while the pointer travels, so the drop starts at once.
    let early = source.clone();
    tauri::async_runtime::spawn(async move {
        let _ = early.listing().await;
    });
    window
        .run_on_main_thread(move || match win_drag::drag(source, &preview) {
            Ok(outcome) => log::info!("drag-out: {outcome:?}"),
            Err(e) => log::warn!("drag-out could not run: {e}"),
        })
        .map_err(|e| AppError::new("drag_error", e.to_string()))
}
