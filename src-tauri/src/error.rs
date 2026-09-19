//! A single serialisable error type that crosses the Tauri IPC boundary.
//!
//! Every backend failure is reduced to a stable `code` (which the UI switches
//! on for translated messages) plus the underlying human-readable `message`.

use serde::Serialize;
use std::fmt;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppError {
    pub code: String,
    pub message: String,
}

impl AppError {
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
        }
    }

    pub fn cancelled() -> Self {
        Self::new("cancelled", "Transfer cancelled")
    }

    /// True for failures where dropping the cached session and dialling
    /// again is the right recovery.
    pub fn is_connection_error(&self) -> bool {
        matches!(
            self.code.as_str(),
            "connection_lost" | "session_expired" | "timed_out"
        )
    }
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for AppError {}

impl From<smb2::Error> for AppError {
    fn from(e: smb2::Error) -> Self {
        use smb2::ErrorKind as K;
        let code = match e.kind() {
            // LOGON_FAILURE is folded into AuthRequired by the crate, so this
            // is the "wrong username or password" case for our purposes.
            K::AuthRequired => "auth_failed",
            K::SigningRequired => "signing_required",
            K::AccessDenied => "access_denied",
            K::NotFound => "not_found",
            K::AlreadyExists => "already_exists",
            K::SharingViolation => "sharing_violation",
            K::IsADirectory => "is_a_directory",
            K::NotADirectory => "not_a_directory",
            K::DiskFull => "disk_full",
            K::ConnectionLost => "connection_lost",
            K::TimedOut => "timed_out",
            K::Cancelled => "cancelled",
            K::SessionExpired => "session_expired",
            _ => "smb_error",
        };
        Self::new(code, e.to_string())
    }
}

impl From<std::io::Error> for AppError {
    fn from(e: std::io::Error) -> Self {
        Self::new("io_error", e.to_string())
    }
}

impl From<keyring::Error> for AppError {
    fn from(e: keyring::Error) -> Self {
        Self::new("keyring_error", e.to_string())
    }
}

impl From<serde_json::Error> for AppError {
    fn from(e: serde_json::Error) -> Self {
        Self::new("config_error", e.to_string())
    }
}

impl From<tauri::Error> for AppError {
    fn from(e: tauri::Error) -> Self {
        Self::new("tauri_error", e.to_string())
    }
}

impl From<mdns_sd::Error> for AppError {
    fn from(e: mdns_sd::Error) -> Self {
        Self::new("discovery_error", e.to_string())
    }
}

impl From<tokio::task::JoinError> for AppError {
    fn from(e: tokio::task::JoinError) -> Self {
        Self::new("internal_error", e.to_string())
    }
}
