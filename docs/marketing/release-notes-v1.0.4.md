## Neat NAS 1.0.4

Folders now remember where you were.

### What's new

- **Back where you left off.** Back, Forward, Up, Backspace and the folders in the path bar return the list to the scroll position it had, with the folder you came out of selected, as in Finder and Explorer.
- **New folders open at the top.** A folder no longer opens part-way down because the previous list was scrolled.
- **Arrow keys stay in view.** Moving the selection up with the arrow keys no longer tucks the row under the column headers.

Copies of 1.0.0 to 1.0.3 offer this update in Settings. Coming from 1.0.0 or 1.0.1, macOS asks once more for keychain access after the update: choose **Always Allow**.

### Downloads

| Platform | File | Notes |
| --- | --- | --- |
| macOS 11+ (Apple Silicon and Intel) | `Neat NAS_1.0.4_universal.dmg` | Open it and drag Neat NAS to Applications. Signed with a Developer ID and notarized by Apple. |
| macOS 11+ (Apple Silicon and Intel) | `Neat NAS_1.0.4_universal-mac.zip` | The same app without the disk image: unzip, move to Applications. |
| Windows 10/11 (x64) | `Neat NAS_1.0.4_x64-setup.exe` | Installs per user and adds WebView2 when it is missing. |
| Windows 10/11 (x64) | `Neat NAS_1.0.4_x64-win.zip` | The same app without an installer: unzip and run `Neat NAS.exe`. Needs WebView2, which Windows 11 and current Windows 10 include. |

**First launch on Windows:** the app is not code-signed on Windows yet. If Windows says it protected your PC, click **More info**, then **Run anyway**.

`SHA256SUMS.txt` lists the checksums of the four files. `latest.json` and the `.sig` files are what the in-app updater uses; you can ignore them.
