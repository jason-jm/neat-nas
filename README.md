# Neat NAS

A small, modern desktop client for browsing an SMB NAS and pulling files down
to your Mac or Windows PC. Add the NAS once; every launch shows its files
immediately, no address to remember, no mounting.

- Tauri 2 shell (Rust backend, React + TypeScript UI, system WebView)
- Pure-Rust SMB2/3 client (`smb2` crate): no mounting, no libsmbclient
- Passwords live in the macOS Keychain / Windows Credential Manager (`keyring`)
- LAN discovery over mDNS (`_smb._tcp`), which Synology and most NAS boxes advertise
- Downloads and uploads (files and folders), with progress, cancellation and no-clobber naming
- Resumable transfers: interrupted or cancelled transfers continue from where they stopped, across app restarts; connection drops are retried automatically
- Quick Look style preview for images, video, audio, PDF and text, streamed straight from the share over the `nasfile://` protocol (no download first)
- Grid view with thumbnails (EXIF-embedded thumbnails when available, so photos and HEIC previews need only the file header)
- Drag files out of the app into Finder (macOS, via file promises) and drop files from Finder/Explorer into the app to upload
- English and Simplified Chinese UI, light and dark themes

## Install

Grab the installer for your platform from the GitHub releases page:

- **macOS** (Apple Silicon and Intel): open the `.dmg`, drag Neat NAS to Applications. Until the app is notarized, macOS asks you to confirm the first launch: right-click the app → Open.
- **Windows 10/11**: run the `-setup.exe`. Until the installer is signed, SmartScreen shows a warning: More info → Run anyway.

The app checks for updates on launch and installs them from Settings. See [RELEASE.md](RELEASE.md) for how releases are produced and signed.

## Develop

Prerequisites: Node 20+, Rust stable (via rustup), Xcode Command Line Tools on
macOS or the MSVC build tools + WebView2 on Windows.

```bash
npm install
npm run tauri dev        # desktop app with hot reload
npm run dev              # UI only, in a browser, backed by an in-memory mock
npm run tauri build      # installers under src-tauri/target/release/bundle
```

### Local SMB server for testing

You don't need a real NAS to work on this. `scripts/dev-smb.sh` starts a
throw-away Samba on `127.0.0.1:1445` as your own user (`brew install samba`
first). Credentials are your username and `nas-test-pass`; it serves a
`Public` and a `Media` share with sample files, including non-ASCII names.

The Rust integration tests run against it:

```bash
scripts/dev-smb.sh &   # or in another terminal
cd src-tauri
NEATNAS_TEST_SMB_ADDR=127.0.0.1:1445 NEATNAS_TEST_SMB_USER=$USER \
NEATNAS_TEST_SMB_PASS=nas-test-pass cargo test --test local_smb
```

### Testing against your own NAS

`cargo run --example probe -- 192.168.1.10:445 USER PASS` negotiates, lists
shares and reads the first share's root. The integration tests accept any
server through the `NEATNAS_TEST_SMB_*` variables as long as it has a
writable `Public` share with the sample files (see `scripts/dev-smb.sh`).

### Dev hooks (debug builds only)

- `NEATNAS_DEVTOOLS=1` opens the web inspector on launch.
- `NEATNAS_DEV_AUTOPILOT="open=Photos/2024,grid,preview=IMG_0003.jpg"`
  replays UI steps after the listing loads and logs a DOM summary for each,
  so tooling can drive the real app without a mouse. Frontend errors are
  forwarded to the Rust log either way.
- `npm run dev` + `http://localhost:1420/?autopilot=...` does the same
  against the in-browser mock.

### Credential store override

`NEATNAS_CRED_STORE=file` makes the app keep passwords in its JSON config
instead of the OS keychain. It exists so automated runs don't trigger keychain
prompts. Never ship or use it for real credentials.

## Layout

```
src/                      React UI
  api.ts                  typed invoke() wrappers; falls back to mock.ts outside Tauri
  components/             Sidebar, FileBrowser, AddServerDialog, SettingsDialog, TransferPanel
  i18n.ts                 en / zh-CN strings
src-tauri/src/
  commands.rs             Tauri command surface
  smb.rs                  connection params, per-server session + share cache, DTOs
  transfer.rs             transfer engine: downloads/uploads, resume via .part files, retries, persisted list
  preview.rs              nasfile:// protocol handler (range requests, cached file handles)
  thumbs.rs               thumbnail generation (EXIF fast path, bounded full decode) with disk cache
  drag.rs                 macOS drag-out using NSFilePromiseProvider
  discovery.rs            mDNS browse for _smb._tcp
  creds.rs                keychain access
  config.rs               servers + settings persisted as JSON in the app config dir
src-tauri/tests/          integration tests against a real SMB server
scripts/dev-smb.sh        local Samba for development
```

Config lives at `~/Library/Application Support/com.neatnas.app/config.json`
on macOS and `%APPDATA%\com.neatnas.app\config.json` on Windows. The
transfer list is `transfers.json` in the app data directory and thumbnails
are cached under the app cache directory.

## Platform notes

- Drag-out to Finder is macOS only for now. On Windows the gesture is a
  no-op and the UI says so; use Download instead.
- Video preview relies on the WebView's codecs: WebKit on macOS plays H.264
  and HEVC in MP4/MOV; MKV containers may not play even if the codec is
  supported.
- HEIC thumbnails come from the EXIF preview embedded by the camera; HEIC
  files without one show a generic icon.
