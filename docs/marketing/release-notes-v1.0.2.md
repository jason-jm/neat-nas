## Neat NAS 1.0.2

The macOS app is now signed with a Developer ID and notarized by Apple.

### What's new

- **Opens like any other Mac app.** No more trip to System Settings on first launch: macOS only confirms once that you want to open an app downloaded from the internet.
- **No keychain prompt after future updates.** macOS now recognises every new version as the same app. It asks one last time after this update, so choose **Always Allow**.

Windows works exactly as before. Copies of 1.0.0 and 1.0.1 offer this update in Settings.

### Downloads

| Platform | File | Notes |
| --- | --- | --- |
| macOS 11+ (Apple Silicon and Intel) | `Neat NAS_1.0.2_universal.dmg` | Open it and drag Neat NAS to Applications. |
| macOS 11+ (Apple Silicon and Intel) | `Neat NAS_1.0.2_universal-mac.zip` | The same app without the disk image: unzip, move to Applications. |
| Windows 10/11 (x64) | `Neat NAS_1.0.2_x64-setup.exe` | Installs per user. Not yet code-signed: SmartScreen → More info → Run anyway. |
| Windows 10/11 (x64) | `Neat NAS_1.0.2_x64-win.zip` | The same app without an installer: unzip and run `Neat NAS.exe`. Needs WebView2, which Windows 11 and current Windows 10 include. |

`SHA256SUMS.txt` lists the checksums of the four files. `latest.json` and the `.sig` files are what the in-app updater uses; you can ignore them.
