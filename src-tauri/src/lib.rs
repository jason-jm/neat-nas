pub mod commands;
pub mod config;
pub mod creds;
pub mod discovery;
#[cfg(target_os = "macos")]
pub mod drag;
pub mod error;
pub mod preview;
pub mod smb;
pub mod thumbs;
pub mod transfer;

use std::sync::Arc;

use tauri::Manager;

/// The app used to ship as `com.nasdrive.app`; its config and transfer list
/// live in a sibling directory. Copy them over the first time the renamed
/// app starts, without touching the originals.
fn migrate_legacy_file(dir: &std::path::Path, name: &str) {
    let target = dir.join(name);
    if target.exists() {
        return;
    }
    let Some(parent) = dir.parent() else { return };
    let legacy = parent.join("com.nasdrive.app").join(name);
    if legacy.is_file() {
        match std::fs::copy(&legacy, &target) {
            Ok(_) => log::info!("migrated {} from {}", name, legacy.display()),
            Err(e) => log::warn!("could not migrate {}: {e}", legacy.display()),
        }
    }
}

pub struct AppState {
    pub config: Arc<config::ConfigStore>,
    pub pool: smb::SmbPool,
    pub transfers: transfer::TransferManager,
    pub store: transfer::TransferStore,
    pub preview: preview::ReaderCache,
    pub thumbs: thumbs::ThumbLimiter,
}

/// Give the macOS window a Finder-style tall title bar: an empty unified
/// toolbar makes AppKit grow the title bar to 52px and centre the traffic
/// lights in it, so they line up with the toolbar row drawn by the web view.
#[cfg(target_os = "macos")]
fn install_unified_titlebar(window: &tauri::WebviewWindow) {
    use objc2::MainThreadMarker;
    use objc2_app_kit::{NSTitlebarSeparatorStyle, NSToolbar, NSToolbarDisplayMode, NSWindow, NSWindowToolbarStyle};

    let Some(mtm) = MainThreadMarker::new() else { return };
    let Ok(ptr) = window.ns_window() else { return };
    if ptr.is_null() {
        return;
    }
    // SAFETY: `ns_window` hands back the live NSWindow that Tauri owns; we
    // only use it on the main thread during setup and never retain it.
    let ns_window: &NSWindow = unsafe { &*(ptr as *const NSWindow) };
    let toolbar = NSToolbar::new(mtm);
    #[allow(deprecated)]
    toolbar.setShowsBaselineSeparator(false);
    // Icon-only keeps the reserved title bar band at 52px instead of the
    // 66px that the default icon-and-label mode asks for.
    toolbar.setDisplayMode(NSToolbarDisplayMode::IconOnly);
    ns_window.setToolbar(Some(&toolbar));
    ns_window.setToolbarStyle(NSWindowToolbarStyle::Unified);
    ns_window.setTitlebarSeparatorStyle(NSTitlebarSeparatorStyle::None);
}


#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let _ = env_logger::Builder::from_env(
        env_logger::Env::default().default_filter_or("info,smb2=warn,mdns_sd=warn"),
    )
    .try_init();

    let builder = tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_process::init());
    #[cfg(feature = "updater")]
    let builder = builder.plugin(tauri_plugin_updater::Builder::new().build());
    builder
        .register_asynchronous_uri_scheme_protocol(preview::SCHEME, |ctx, request, responder| {
            preview::handle(ctx.app_handle().clone(), request, responder)
        })
        .setup(|app| {
            let config_dir = app.path().app_config_dir()?;
            let data_dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&config_dir)?;
            std::fs::create_dir_all(&data_dir)?;
            migrate_legacy_file(&config_dir, "config.json");
            migrate_legacy_file(&data_dir, "transfers.json");
            app.manage(AppState {
                config: Arc::new(config::ConfigStore::load(config_dir.join("config.json"))),
                pool: smb::SmbPool::default(),
                transfers: transfer::TransferManager::default(),
                store: transfer::TransferStore::load(data_dir.join("transfers.json")),
                preview: preview::ReaderCache::default(),
                thumbs: thumbs::ThumbLimiter::default(),
            });
            preview::spawn_janitor(app.handle().clone());
            #[cfg(target_os = "macos")]
            if let Some(w) = app.get_webview_window("main") {
                install_unified_titlebar(&w);
            }
            // Dev aids: NEATNAS_DEVTOOLS=1 opens the web inspector on launch;
            // NEATNAS_DEV_AUTOPILOT brings the window to the front so tooling
            // can screenshot it.
            #[cfg(debug_assertions)]
            if let Some(w) = app.get_webview_window("main") {
                if std::env::var("NEATNAS_DEVTOOLS").is_ok() {
                    w.open_devtools();
                }
                if std::env::var("NEATNAS_DEV_AUTOPILOT").is_ok() {
                    let _ = w.set_focus();
                }
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::list_servers,
            commands::test_connection,
            commands::add_server,
            commands::update_server,
            commands::remove_server,
            commands::disconnect_server,
            commands::remember_share,
            commands::list_shares,
            commands::list_dir,
            commands::start_download,
            commands::start_upload,
            commands::cancel_transfer,
            commands::resume_transfer,
            commands::remove_transfer,
            commands::list_transfers,
            commands::clear_finished_transfers,
            commands::thumbnail,
            commands::start_drag_out,
            commands::discover_servers,
            commands::get_settings,
            commands::set_download_dir,
            commands::frontend_log,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
