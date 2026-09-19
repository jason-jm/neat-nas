# Neat NAS launch kit

Everything needed to announce Neat NAS 1.0, ready to paste. Adjust the voice, never the facts (all numbers below are real: ~10 MB download, SMB 2/3, ten languages, MIT).

## Positioning

**One line.** Remember your NAS once. Open it forever.

**Two lines.** Neat NAS is the lightest NAS file browser for Mac and Windows. Save the address, user name and password a single time; every launch opens straight into your files.

**Why it's different, in one breath.** It is a browser, not a mount: no drive letters, no Finder time-outs, no admin rights, ~10 MB instead of 200, and it speaks SMB itself in Rust.

**Who it is for.** Anyone with a Synology, QNAP, TrueNAS or Samba box at home who keeps re-typing an IP address. Households that move files between a Mac and a Windows PC through the NAS.

**Chinese one-liner.** 记不住 NAS 的地址、账号、密码？Neat NAS 只要记一次。Mac 和 Windows 上最轻量的 NAS 文件浏览器。

## Assets to prepare before posting

1. Hero screenshot (2×, dark and light): the app open on a photo folder in grid view, sidebar showing one NAS with the green dot.
2. A 12-second GIF or MP4: launch → files appear instantly → Space on a photo → arrow to a video → it plays.
3. The "three things you'll never type again" card from the website as a single image (address / user / password, struck through).
4. Screenshot of Settings showing ten languages and the appearance switch.

Record with Screen Studio, CleanShot or macOS ⌘⇧5; keep the window at 1120×720 so text stays legible when downscaled.

## Show HN

**Title:** Show HN: Neat NAS – a 10 MB SMB file browser for Mac and Windows that remembers your NAS

**Body:**

> I kept forgetting my NAS's address and re-entering the password every time Finder timed out, so I built a small desktop app that does one thing: it opens on the NAS.
>
> Neat NAS speaks SMB 2/3 itself (the pure-Rust `smb2` crate), so nothing is mounted and nothing needs admin rights. The UI is React in the system web view via Tauri 2, which keeps the download around 10 MB. Passwords go to the Keychain / Credential Manager. Previews stream over an internal `nasfile://` protocol with range requests, so a video starts playing without a download. Transfers resume across restarts. mDNS discovery finds the box.
>
> macOS universal + Windows x64, ten UI languages, MIT: https://github.com/jason-jm/neat-nas
>
> Things I'd love feedback on: the resume logic across flaky Wi-Fi, and what people with QNAP/TrueNAS see, since I only own a Synology.

Post between 8 and 10 am US Eastern on a weekday. Reply to every comment in the first two hours.

## Product Hunt

- **Name:** Neat NAS
- **Tagline (60 chars):** Remember your NAS once. Open it forever.
- **Description:** The lightest NAS file browser for Mac and Windows. Save the address, user name and password a single time; from then on every launch opens straight into your files. Quick Look for photos, video, PDF and text streamed from the NAS, resumable transfers both ways, drag-out to Finder, ten languages, dark mode, ~10 MB. Free and open source (MIT).
- **Topics:** Productivity, Mac, Windows, Open Source, Developer Tools
- **First comment (maker):** the Show HN body, shortened, plus the GIF.

## Reddit

Post the GIF with a short, honest text. Suggested subreddits and angles:

- **r/synology** — "Made a tiny Mac/Windows app that opens straight into my DS files without mounting or re-typing the password (open source)". Mention it works with any SMB share, not only DSM.
- **r/DataHoarder** — lead with resumable transfers and folder uploads; this crowd cares about robustness.
- **r/macapps** — lead with Quick Look, drag-out to Finder and the 10 MB size ("not Electron").
- **r/software** or **r/Windows10** — lead with "no drive letter, no admin rights, installs per user".
- **r/rust** — a technical write-up: pure-Rust SMB client, Tauri 2, range-request streaming, EXIF thumbnail fast path.

Rules of thumb: link to GitHub, not the website; disclose you are the author; answer questions with specifics.

## X / Twitter thread

1. I kept forgetting my NAS address and password, so I built an app that remembers it once and opens straight into my files. Neat NAS, for Mac and Windows, is out today and it's open source. 🧵
2. It's a browser, not a mount. No drive letters, no Finder time-outs, no admin rights. It speaks SMB 2/3 itself, in Rust.
3. Quick Look works on everything: photos, video, audio, PDF, text, streamed straight from the NAS. Press Space, like Finder. [GIF]
4. Transfers resume where they stopped, even after you quit. Drag files out to Finder, drop files in to upload.
5. ~10 MB download. Native core + the web view your OS already has (Tauri 2), not a 200 MB Electron shell.
6. Ten languages, light and dark, signed auto-updates. MIT licensed: https://github.com/jason-jm/neat-nas  Website: https://jason-jm.github.io/neat-nas/

## Chinese-speaking channels

- **V2EX › 分享创造**：标题「做了一个记住 NAS 一次就够的文件浏览器，Mac/Windows，约 10 MB，开源」。正文说明痛点（记不住地址、Finder 超时、每次输密码）、实现（Rust 直连 SMB、不挂载、系统 WebView）、下载链接，最后求反馈（群晖以外的机器表现）。
- **少数派**：投稿到「Matrix」栏目，写成一篇 1500 字左右的介绍：为什么不用 Finder 直接连、快速预览和断点续传的实现、隐私（密码只在钥匙串）。附浅色和深色截图。
- **什么值得买 / 张大妈**、**恩山论坛 NAS 板块**、**群晖官方社区中文区**：短文 + 截图 + GitHub 链接。
- **小红书 / B 站**：15 秒竖屏或横屏视频，"打开就是文件"的对比：左边 Finder 连接 NAS 的五步，右边 Neat NAS 一步。

## Synology / QNAP communities

Synology Community (English) and the QNAP forum both have "third-party apps" sections. Post once, with the GIF, and explicitly say the app is not affiliated with the vendor.

## Launch-week checklist

- [ ] Repository public, README and website live, release 1.0.0 published with both installers.
- [ ] GitHub topics set: `nas`, `smb`, `synology`, `file-browser`, `tauri`, `rust`, `macos`, `windows`.
- [ ] Social preview image uploaded in the repository settings (1280×640, the "three things" card).
- [ ] Day 1: Show HN + r/synology + X thread + V2EX.
- [ ] Day 2: Product Hunt (schedule 12:01 am PT) + r/macapps + 少数派 submission.
- [ ] Day 3–5: r/DataHoarder, r/rust write-up, vendor communities.
- [ ] Reply to every issue within 24 hours during launch week; label good ones `good first issue`.
- [ ] Ship 1.0.1 within two weeks with the most requested fix; nothing signals "maintained" like a fast follow-up release.

## What to measure

GitHub stars and release downloads per day (Insights → Traffic), issues opened per platform, and which channel the referrers come from. A healthy first week for a niche utility is a few hundred stars and a handful of real bug reports from non-Synology users.
