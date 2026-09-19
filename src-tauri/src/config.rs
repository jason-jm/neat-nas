//! On-disk configuration: the list of saved NAS servers and user settings.
//!
//! Passwords never live here (see `creds.rs`), except in the explicit
//! dev-only file store used for automated tests.

use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;

use serde::{Deserialize, Serialize};

use crate::error::AppError;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SavedServer {
    pub id: String,
    pub name: String,
    pub host: String,
    pub port: u16,
    pub username: String,
    #[serde(default)]
    pub domain: String,
    #[serde(default)]
    pub last_share: Option<String>,
    #[serde(default)]
    pub added_at: u64,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppConfig {
    #[serde(default)]
    pub servers: Vec<SavedServer>,
    #[serde(default)]
    pub download_dir: Option<String>,
    /// Dev-only. Only populated when `NEATNAS_CRED_STORE=file` is set, so
    /// that automated tests can run without a keychain prompt.
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub insecure_passwords: HashMap<String, String>,
}

pub struct ConfigStore {
    path: PathBuf,
    data: Mutex<AppConfig>,
}

impl ConfigStore {
    pub fn load(path: PathBuf) -> Self {
        let data = fs::read(&path)
            .ok()
            .and_then(|bytes| serde_json::from_slice::<AppConfig>(&bytes).ok())
            .unwrap_or_default();
        Self {
            path,
            data: Mutex::new(data),
        }
    }

    pub fn path(&self) -> &PathBuf {
        &self.path
    }

    pub fn snapshot(&self) -> AppConfig {
        self.data
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone()
    }

    pub fn server(&self, id: &str) -> Option<SavedServer> {
        self.snapshot().servers.into_iter().find(|s| s.id == id)
    }

    /// Mutate the config and persist it atomically (write temp + rename).
    pub fn update<R>(&self, f: impl FnOnce(&mut AppConfig) -> R) -> Result<R, AppError> {
        let mut guard = self
            .data
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let result = f(&mut guard);
        let json = serde_json::to_vec_pretty(&*guard)?;
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
        }
        let tmp = self.path.with_extension("json.tmp");
        fs::write(&tmp, json)?;
        fs::rename(&tmp, &self.path)?;
        Ok(result)
    }
}
