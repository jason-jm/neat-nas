# Developing Neat NAS

Prerequisites: Node 20+, Rust stable (via rustup), Xcode Command Line Tools on
macOS or the MSVC build tools + WebView2 on Windows.

```bash
npm install
npm run tauri dev        # desktop app with hot reload
npm run dev              # UI only, in a browser, backed by an in-memory mock
npm run tauri build      # installers under src-tauri/target/release/bundle
```

## Local SMB server for testing

You don't need a real NAS to work on this. `scripts/dev-smb.sh` starts a
throw-away Samba on `127.0.0.1:1445` as your own user (`brew install samba`
first). Credentials are your username and `nas-test-pass`; it serves a
`Public` and a `Media` share with sample files, including non-ASCII names.

The Rust integration tests run against it:

```bash
scripts/dev-smb.sh &   # or in another terminal
cd src-tauri
NEATNAS_TEST_SMB_ADDR=127.0.0.1:1445 NEATNAS_TEST_SMB_USER=$USER \
NEATNAS_TEST_SMB_PASS=nas-test-pass cargo test --tests
```

## Testing against your own NAS

`cargo run --example probe -- 192.168.1.10:445 USER PASS` negotiates, lists
shares and reads the first share's root. The integration tests accept any
server through the `NEATNAS_TEST_SMB_*` variables as long as it has a
writable `Public` share with the sample files (see `scripts/dev-smb.sh`).

## Dev hooks (debug builds only)

- `NEATNAS_DEVTOOLS=1` opens the web inspector on launch.
- `NEATNAS_DEV_AUTOPILOT="open=Photos/2024,grid,preview=IMG_0003.jpg"`
  replays UI steps after the listing loads and logs a DOM summary for each,
  so tooling can drive the real app without a mouse. Frontend errors are
  forwarded to the Rust log either way.
- `npm run dev` + `http://localhost:1420/?autopilot=...` does the same
  against the in-browser mock.

## Credential store override

`NEATNAS_CRED_STORE=file` makes the app keep passwords in its JSON config
instead of the OS keychain. It exists so automated runs don't trigger keychain
prompts. Never ship or use it for real credentials.

## Layout

```
src/                      React UI
  api.ts                  typed invoke() wrappers; falls back to mock.ts outside Tauri
  components/             Sidebar, FileBrowser, PreviewModal, Transfers, AddServerDialog, SettingsDialog
  i18n.ts + locales/      UI strings, one JSON per language (scripts/check-i18n.mjs keeps them in sync)
  prefs.ts                theme and double-click preferences
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
scripts/                  dev Samba, Windows cross-build, release publishing, i18n check
docs/                     website (GitHub Pages), localized READMEs, marketing copy
ci/                       GitHub Actions workflows, parked until enabled (see ci/README.md)
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
- Adding a UI language: copy `src/locales/en.json`, translate, register the
  file in `src/locales/index.ts`. `npm run build` refuses to build if any
  locale is missing a key or a placeholder.
