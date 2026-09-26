## Neat NAS 1.0.1

A macOS fix, plus zip downloads for both platforms.

### What's new

- **macOS: the app is now code-signed.** 1.0.0 was unsigned, and Apple silicon Macs can report such apps as "damaged" when they come from the internet. 1.0.1 is ad-hoc signed, so macOS shows its usual first-launch check instead (see below).
- **Zip downloads.** Every release now also ships the app as a plain `.zip` for macOS and Windows, plus `SHA256SUMS.txt` with the checksums of all four files.

Windows works exactly as in 1.0.0. Copies of 1.0.0 offer this update in Settings; on a Mac, macOS then asks once whether Neat NAS may use its saved password in the keychain, so choose **Always Allow**.

### Downloads

| Platform | File | Notes |
| --- | --- | --- |
| macOS 11+ (Apple Silicon and Intel) | `Neat NAS_1.0.1_universal.dmg` | Open it and drag Neat NAS to Applications. |
| macOS 11+ (Apple Silicon and Intel) | `Neat NAS_1.0.1_universal-mac.zip` | The same app without the disk image: unzip, move to Applications. |
| Windows 10/11 (x64) | `Neat NAS_1.0.1_x64-setup.exe` | Installs per user. Not yet code-signed: SmartScreen → More info → Run anyway. |
| Windows 10/11 (x64) | `Neat NAS_1.0.1_x64-win.zip` | The same app without an installer: unzip and run `Neat NAS.exe`. Needs WebView2, which Windows 11 and current Windows 10 include. |

**First launch on a Mac:** the app is not notarized by Apple yet, so macOS says it cannot verify it. Open System Settings → Privacy & Security, scroll down and click **Open Anyway**. On macOS 14 and earlier you can right-click the app and choose Open instead.

`SHA256SUMS.txt` lists the checksums of the four files. `latest.json` and the `.sig` files are what the in-app updater uses; you can ignore them.
