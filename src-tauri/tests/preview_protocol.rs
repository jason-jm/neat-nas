//! Exercises the `nasfile://` handler with Tauri's mock runtime against the
//! local Samba server. Skipped unless NEATNAS_TEST_SMB_* are set.
//!
//! Not built on Windows: a test binary that links Tauri's runtime dies at
//! start-up with STATUS_ENTRYPOINT_NOT_FOUND (0xc0000139) because the
//! WebView2 loader is only staged next to the real app, and the test would
//! self-skip on CI anyway for lack of an SMB server.
#![cfg(not(windows))]

use std::sync::Arc;

use neatnas_lib::config::{ConfigStore, SavedServer};
use neatnas_lib::{preview, smb, thumbs, transfer, AppState};
use tauri::Manager;

fn env(key: &str) -> Option<String> {
    std::env::var(key).ok()
}

fn build_state(dir: &std::path::Path) -> Option<AppState> {
    let addr = env("NEATNAS_TEST_SMB_ADDR")?;
    let (host, port) = addr.rsplit_once(':')?;
    let config = ConfigStore::load(dir.join("config.json"));
    config
        .update(|c| {
            c.servers.push(SavedServer {
                id: "test".into(),
                name: "Test".into(),
                host: host.to_string(),
                port: port.parse().ok().unwrap_or(445),
                username: env("NEATNAS_TEST_SMB_USER").unwrap_or_default(),
                domain: String::new(),
                last_share: None,
                added_at: 0,
            });
            c.insecure_passwords
                .insert("test".into(), env("NEATNAS_TEST_SMB_PASS").unwrap_or_default());
        })
        .ok()?;
    Some(AppState {
        config: Arc::new(config),
        pool: smb::SmbPool::default(),
        transfers: transfer::TransferManager::default(),
        store: transfer::TransferStore::load(dir.join("transfers.json")),
        preview: preview::ReaderCache::default(),
        thumbs: thumbs::ThumbLimiter::default(),
    })
}

fn request(uri: &str, range: Option<&str>) -> http::Request<Vec<u8>> {
    let mut b = http::Request::builder().method("GET").uri(uri);
    if let Some(r) = range {
        b = b.header("range", r);
    }
    b.body(Vec::new()).unwrap()
}

#[tokio::test]
async fn serves_whole_files_and_byte_ranges() {
    if env("NEATNAS_TEST_SMB_ADDR").is_none() {
        eprintln!("skipped: NEATNAS_TEST_SMB_* not set");
        return;
    }
    // The file credential store keeps the mock app away from the keychain.
    std::env::set_var("NEATNAS_CRED_STORE", "file");
    let dir = std::env::temp_dir().join(format!("neatnas-preview-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    let app = tauri::test::mock_app();
    app.manage(build_state(&dir).expect("state"));
    let handle = app.handle().clone();

    // Whole file.
    let resp = preview::serve(&handle, &request("nasfile://localhost/test/Public/readme.txt", None))
        .await
        .expect("serve");
    assert_eq!(resp.status(), 200);
    assert_eq!(resp.headers()["content-type"], "text/plain");
    assert_eq!(resp.headers()["accept-ranges"], "bytes");
    assert_eq!(resp.headers()["content-length"], "15");
    assert_eq!(resp.body(), b"hello from NAS\n");

    // Byte range, as a video element or a text preview would ask.
    let resp = preview::serve(&handle, &request("nasfile://localhost/test/Public/readme.txt", Some("bytes=6-9")))
        .await
        .expect("serve range");
    assert_eq!(resp.status(), 206);
    assert_eq!(resp.headers()["content-range"], "bytes 6-9/15");
    assert_eq!(resp.body(), b"from");

    // Open-ended range on a big file is capped, so the player streams in
    // chunks instead of pulling the whole thing.
    let resp = preview::serve(&handle, &request("nasfile://localhost/test/Media/Movies/sample-40mb.mkv", Some("bytes=1000-")))
        .await
        .expect("serve capped range");
    assert_eq!(resp.status(), 206);
    assert_eq!(resp.body().len(), 2 * 1024 * 1024);
    assert_eq!(resp.headers()["content-range"], format!("bytes 1000-{}/40000000", 1000 + 2 * 1024 * 1024 - 1));
    assert_eq!(resp.headers()["content-type"], "video/x-matroska");

    // Percent-encoded non-ASCII path segments.
    let resp = preview::serve(&handle, &request("nasfile://localhost/test/Public/%E6%96%87%E6%A1%A3/%E8%AF%B4%E6%98%8E.txt", None))
        .await
        .expect("serve unicode");
    assert_eq!(resp.status(), 200);
    assert_eq!(resp.body(), "中文内容测试\n".as_bytes());

    // Errors map to HTTP statuses.
    let err = preview::serve(&handle, &request("nasfile://localhost/test/Public/missing.txt", None))
        .await
        .unwrap_err();
    assert_eq!(err.code, "not_found");
    let err = preview::serve(&handle, &request("nasfile://localhost/test", None)).await.unwrap_err();
    assert_eq!(err.code, "bad_request");

    // The handle cache reuses the open file and closes idle ones later.
    let state = handle.state::<AppState>();
    state.preview.close_idle().await;
    let _ = std::fs::remove_dir_all(&dir);
}
