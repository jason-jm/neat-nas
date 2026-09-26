## Neat NAS 1.0.3

A maintenance release: the app itself is unchanged from 1.0.2, rebuilt and published by the current release pipeline.

### Downloads

| Platform | File | Notes |
| --- | --- | --- |
| macOS 11+ (Apple Silicon and Intel) | `Neat NAS_1.0.3_universal.dmg` | Open it and drag Neat NAS to Applications. Signed with a Developer ID and notarized by Apple. |
| macOS 11+ (Apple Silicon and Intel) | `Neat NAS_1.0.3_universal-mac.zip` | The same app without the disk image: unzip, move to Applications. |
| Windows 10/11 (x64) | `Neat NAS_1.0.3_x64-setup.exe` | Installs per user and adds WebView2 when it is missing. |
| Windows 10/11 (x64) | `Neat NAS_1.0.3_x64-win.zip` | The same app without an installer: unzip and run `Neat NAS.exe`. Needs WebView2, which Windows 11 and current Windows 10 include. |

**First launch on Windows:** the app is not code-signed on Windows yet. If Windows says it protected your PC, click **More info**, then **Run anyway**.

`SHA256SUMS.txt` lists the checksums of the four files. `latest.json` and the `.sig` files are what the in-app updater uses; you can ignore them.
