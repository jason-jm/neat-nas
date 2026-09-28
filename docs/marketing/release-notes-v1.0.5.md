## Neat NAS 1.0.5

Drag and drop on Windows, and a toolbar that never cuts anything off.

### What's new

- **Drag files into Explorer on Windows.** Drag files or whole folders from Neat NAS onto an Explorer window or the desktop. Explorer copies them straight from the NAS with its usual progress window; nothing is downloaded first.
- **Nothing in the toolbar is cut off.** On Mac and Windows, in all ten languages, button labels and the search hint are shown in full or left out. In a narrow window the search becomes a magnifier button (or press ⌘F / Ctrl+F) that opens where the folder name was. On Windows, the close button is whole again.
- **The upload overlay only shows for files.** Dragging text or links over the window no longer offers to upload.

Copies of 1.0.0 to 1.0.4 offer this update in Settings. Coming from 1.0.0 or 1.0.1, macOS asks once more for keychain access after the update: choose **Always Allow**.

### Downloads

| Platform | File | Notes |
| --- | --- | --- |
| macOS 11+ (Apple Silicon and Intel) | `Neat NAS_1.0.5_universal.dmg` | Open it and drag Neat NAS to Applications. Signed with a Developer ID and notarized by Apple. |
| macOS 11+ (Apple Silicon and Intel) | `Neat NAS_1.0.5_universal-mac.zip` | The same app without the disk image: unzip, move to Applications. |
| Windows 10/11 (x64) | `Neat NAS_1.0.5_x64-setup.exe` | Installs per user and adds WebView2 when it is missing. |
| Windows 10/11 (x64) | `Neat NAS_1.0.5_x64-win.zip` | The same app without an installer: unzip and run `Neat NAS.exe`. Needs WebView2, which Windows 11 and current Windows 10 include. |

**First launch on Windows:** the app is not code-signed on Windows yet. If Windows says it protected your PC, click **More info**, then **Run anyway**.

`SHA256SUMS.txt` lists the checksums of the four files. `latest.json` and the `.sig` files are what the in-app updater uses; you can ignore them.
