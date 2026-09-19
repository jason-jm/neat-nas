//! Credential storage. Production builds use the OS keychain (macOS Keychain,
//! Windows Credential Manager) through the `keyring` crate. Setting
//! `NEATNAS_CRED_STORE=file` switches to a plain-text entry in the config
//! file; that mode exists only for automated tests.

use crate::config::ConfigStore;
use crate::error::AppError;

const SERVICE: &str = "com.neatnas.app";
/// Service name used before the app was renamed; read-only fallback.
const LEGACY_SERVICE: &str = "com.nasdrive.app";

fn use_file_store() -> bool {
    std::env::var("NEATNAS_CRED_STORE")
        .map(|v| v == "file")
        .unwrap_or(false)
}

pub fn save_password(config: &ConfigStore, server_id: &str, password: &str) -> Result<(), AppError> {
    if use_file_store() {
        config.update(|c| {
            c.insecure_passwords
                .insert(server_id.to_string(), password.to_string());
        })?;
        return Ok(());
    }
    keyring::Entry::new(SERVICE, server_id)?.set_password(password)?;
    Ok(())
}

pub fn load_password(config: &ConfigStore, server_id: &str) -> Result<Option<String>, AppError> {
    if use_file_store() {
        return Ok(config.snapshot().insecure_passwords.get(server_id).cloned());
    }
    match keyring::Entry::new(SERVICE, server_id)?.get_password() {
        Ok(p) => Ok(Some(p)),
        Err(keyring::Error::NoEntry) => load_legacy_password(server_id),
        Err(e) => Err(e.into()),
    }
}

/// Passwords saved under the pre-rename service name are read once and
/// re-saved under the current one, so the old entry is only touched once.
fn load_legacy_password(server_id: &str) -> Result<Option<String>, AppError> {
    match keyring::Entry::new(LEGACY_SERVICE, server_id)?.get_password() {
        Ok(p) => {
            if let Err(e) = keyring::Entry::new(SERVICE, server_id).and_then(|entry| entry.set_password(&p)) {
                log::warn!("could not migrate keychain entry for {server_id}: {e}");
            } else {
                log::info!("migrated keychain entry for {server_id} to {SERVICE}");
            }
            Ok(Some(p))
        }
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(e) => Err(e.into()),
    }
}

pub fn delete_password(config: &ConfigStore, server_id: &str) -> Result<(), AppError> {
    if use_file_store() {
        config.update(|c| {
            c.insecure_passwords.remove(server_id);
        })?;
        return Ok(());
    }
    match keyring::Entry::new(SERVICE, server_id)?.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(e.into()),
    }
}
