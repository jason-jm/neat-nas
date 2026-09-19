//! Thin, app-shaped layer over the `smb2` crate: connection parameters,
//! a per-server session (client + cached share trees) and the DTOs sent to
//! the UI.

use std::collections::HashMap;
use std::sync::{Arc, Mutex as StdMutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::Serialize;
use smb2::{ClientConfig, SmbClient, Tree};
use tokio::sync::Mutex;

use crate::error::AppError;

pub const DEFAULT_PORT: u16 = 445;
const CONNECT_TIMEOUT: Duration = Duration::from_secs(8);

#[derive(Debug, Clone)]
pub struct ConnectionParams {
    pub host: String,
    pub port: u16,
    pub username: String,
    pub password: String,
    pub domain: String,
}

impl ConnectionParams {
    pub fn addr(&self) -> String {
        let host = self.host.trim();
        if host.contains(':') && !host.starts_with('[') {
            format!("[{}]:{}", host, self.port)
        } else {
            format!("{}:{}", host, self.port)
        }
    }

    pub fn client_config(&self) -> ClientConfig {
        ClientConfig {
            addr: self.addr(),
            timeout: CONNECT_TIMEOUT,
            username: self.username.clone(),
            password: self.password.clone(),
            domain: self.domain.clone(),
            auto_reconnect: true,
            ..Default::default()
        }
    }
}

pub async fn connect(params: &ConnectionParams) -> Result<SmbClient, AppError> {
    Ok(SmbClient::connect(params.client_config()).await?)
}

// ── DTOs ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShareDto {
    pub name: String,
    pub comment: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EntryDto {
    pub name: String,
    /// Share-relative path using `/` separators, no leading slash.
    pub path: String,
    pub size: u64,
    pub is_dir: bool,
    /// Unix milliseconds.
    pub modified: Option<u64>,
    pub created: Option<u64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionInfo {
    pub dialect: String,
    pub shares: Vec<ShareDto>,
    /// Share enumeration is a separate RPC that some servers refuse even
    /// when file access works, so its failure is reported, not fatal.
    pub shares_error: Option<String>,
}

// MS-SRVS share types: low bits are the kind, high bits are flags.
const STYPE_KIND_MASK: u32 = 0x0FFF_FFFF;
const STYPE_DISKTREE: u32 = 0x0000_0000;
const STYPE_SPECIAL: u32 = 0x8000_0000;

/// Keep only user-facing disk shares (drop IPC$, printers, admin shares).
pub fn filter_shares(shares: Vec<smb2::ShareInfo>) -> Vec<ShareDto> {
    let mut out: Vec<ShareDto> = shares
        .into_iter()
        .filter(|s| {
            s.share_type & STYPE_KIND_MASK == STYPE_DISKTREE
                && s.share_type & STYPE_SPECIAL == 0
                && !s.name.ends_with('$')
        })
        .map(|s| ShareDto {
            name: s.name,
            comment: s.comment,
        })
        .collect();
    out.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    out
}

pub fn join_path(dir: &str, name: &str) -> String {
    let dir = dir.trim_matches('/');
    if dir.is_empty() {
        name.to_string()
    } else {
        format!("{dir}/{name}")
    }
}

fn to_unix_ms(t: Option<SystemTime>) -> Option<u64> {
    t.and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_millis() as u64)
}

// ── Session ─────────────────────────────────────────────────────────────

/// One authenticated connection to one server plus the share trees that
/// have been opened on it.
pub struct Session {
    pub client: SmbClient,
    trees: HashMap<String, Tree>,
}

impl Session {
    pub fn new(client: SmbClient) -> Self {
        Self {
            client,
            trees: HashMap::new(),
        }
    }

    pub fn dialect(&self) -> String {
        self.client
            .params()
            .map(|p| p.dialect.to_string())
            .unwrap_or_default()
    }

    async fn ensure_tree(&mut self, share: &str) -> Result<(), AppError> {
        if !self.trees.contains_key(share) {
            let tree = self.client.connect_share(share).await?;
            self.trees.insert(share.to_string(), tree);
        }
        Ok(())
    }

    /// A clone of the connected tree for `share`, opening it if needed.
    pub async fn tree(&mut self, share: &str) -> Result<Tree, AppError> {
        self.ensure_tree(share).await?;
        Ok(self.trees.get(share).expect("tree was just inserted").clone())
    }

    /// Open a random-access reader that owns its own connection clone, so
    /// callers can read without holding the session lock.
    pub async fn open_reader(&mut self, share: &str, path: &str) -> Result<smb2::FileReader, AppError> {
        self.ensure_tree(share).await?;
        let tree = self.trees.get(share).expect("tree was just inserted");
        match self.client.open_file_reader(tree, path.trim_matches('/')).await {
            Ok(reader) => Ok(reader),
            Err(e) => {
                if matches!(e.kind(), smb2::ErrorKind::ConnectionLost | smb2::ErrorKind::SessionExpired) {
                    self.trees.remove(share);
                }
                Err(e.into())
            }
        }
    }

    pub async fn list_shares(&mut self) -> Result<Vec<ShareDto>, AppError> {
        Ok(filter_shares(self.client.list_shares().await?))
    }

    pub async fn list_dir(&mut self, share: &str, path: &str) -> Result<Vec<EntryDto>, AppError> {
        self.ensure_tree(share).await?;
        let path = path.trim_matches('/');
        let tree = self.trees.get_mut(share).expect("tree was just inserted");
        let entries = match self.client.list_directory(tree, path).await {
            Ok(entries) => entries,
            Err(e) => {
                // A stale tree id after a server-side reconnect is the usual
                // cause; forget it so the next call opens a fresh one.
                self.trees.remove(share);
                return Err(e.into());
            }
        };
        let mut out: Vec<EntryDto> = entries
            .into_iter()
            .filter(|e| e.name != "." && e.name != "..")
            .map(|e| EntryDto {
                path: join_path(path, &e.name),
                size: e.size,
                is_dir: e.is_directory,
                modified: to_unix_ms(e.modified.to_system_time()),
                created: to_unix_ms(e.created.to_system_time()),
                name: e.name,
            })
            .collect();
        out.sort_by(|a, b| {
            b.is_dir
                .cmp(&a.is_dir)
                .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
        });
        Ok(out)
    }
}

/// All live sessions, keyed by saved-server id.
#[derive(Default)]
pub struct SmbPool {
    sessions: StdMutex<HashMap<String, Arc<Mutex<Session>>>>,
}

impl SmbPool {
    fn lock(&self) -> std::sync::MutexGuard<'_, HashMap<String, Arc<Mutex<Session>>>> {
        self.sessions
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    pub fn get(&self, server_id: &str) -> Option<Arc<Mutex<Session>>> {
        self.lock().get(server_id).cloned()
    }

    pub fn insert(&self, server_id: &str, session: Session) -> Arc<Mutex<Session>> {
        let session = Arc::new(Mutex::new(session));
        self.lock().insert(server_id.to_string(), session.clone());
        session
    }

    pub fn remove(&self, server_id: &str) {
        self.lock().remove(server_id);
    }
}
