<div align="center">

<img src="../../assets/icon-1024.png" width="112" alt="Neat NAS">

# Neat NAS

**NAS 的位址、帳號、密碼，只記一次。**

Mac 與 Windows 上最輕量的 NAS 檔案瀏覽器。位址、使用者名稱、密碼儲存一次，之後每次開啟應用程式，直接就是你的檔案。

[**下載 macOS 版**](https://github.com/jason-jm/neat-nas/releases/latest) · [**下載 Windows 版**](https://github.com/jason-jm/neat-nas/releases/latest) · [官網](https://jason-jm.github.io/neat-nas/)

[English](../../README.md) · [简体中文](README.zh-CN.md) · [繁體中文](README.zh-TW.md) · [日本語](README.ja.md) · [한국어](README.ko.md) · [Deutsch](README.de.md) · [Español](README.es.md) · [Français](README.fr.md) · [Português](README.pt-BR.md) · [Русский](README.ru.md)

</div>

---

## 問題

有 NAS 的人都懂：想拿一個檔案，先開 Finder 或檔案總管，找到伺服器，回想到底是 `192.168.1.126` 還是 `.128`，輸入使用者名稱，密碼打錯一次，等掛載，最後才能開始翻檔案。每一次都是這樣。

## 解法

Neat NAS 是一個只把一件事做好的小應用程式：開啟就是你的 NAS。新增一次之後，每次啟動都直接進入檔案列表，已連線，密碼從系統鑰匙圈自動取出。不掛載、沒有磁碟代號，再也不必輸入 `smb://`。

## 你能得到什麼

- **開啟即檔案。** 上次用的 NAS 在視窗顯示完之前已經連上。
- **自動找到 NAS。** Synology、QNAP、TrueNAS、跑著 Samba 的樹莓派，只要在區域網路廣播 SMB，新增對話框裡就看得到。
- **密碼只放在鑰匙圈。** macOS 鑰匙圈與 Windows 認證管理員，絕不落到純文字檔。
- **任何檔案都能快速查看。** 照片、影片、音訊、PDF、文字直接從 NAS 串流，像 Finder 一樣按空白鍵。
- **雙向傳檔。** 下載、上傳檔案或整個資料夾；拖出到 Finder，拖進來即上傳。
- **傳輸不怕中斷。** 暫停或中斷的傳輸從斷點繼續，退出應用程式再開也一樣。
- **秒開的照片格狀檢視。** 縮圖使用相機內嵌在每張照片裡的預覽，RAW 和 HEIC 資料夾只需讀檔頭。
- **淺色、深色，十種語言。**
- **真正輕量。** 純 Rust 核心加系統內建 WebView，下載約 10 MB，秒開，沒有 Electron。
- **自動更新。** 新版本出現在設定裡，一鍵安裝，安裝前驗證簽章。

## 安裝

**macOS 11 以上**（Apple 晶片與 Intel 通用）：開啟 `.dmg`，把 Neat NAS 拖到「應用程式」。應用程式已用開發者憑證簽署並通過 Apple 公證，像其他下載的應用程式一樣直接開啟即可。

**Windows 10/11**：執行 `-setup.exe`，安裝到目前使用者，不需要管理員權限。安裝程式尚未簽章，SmartScreen 會提示 → 其他資訊 → 仍要執行。

安裝檔在 [Releases 頁面](https://github.com/jason-jm/neat-nas/releases/latest)。

## 運作原理

Neat NAS 以純 Rust 用戶端直接實作 SMB 2/3，不掛載任何東西，也不需要管理員權限。家用區域網路列出幾百個檔案只需數十毫秒。預覽透過內部的 `nasfile://` 協定按範圍讀取，所以影片不必先下載就能播放。介面是跑在系統內建 WebView（macOS 的 WebKit、Windows 的 WebView2）裡的 React，由 [Tauri 2](https://tauri.app) 驅動，體積比 Electron 應用程式小一個數量級。

## 參與

歡迎回報問題、翻譯與 PR。從 [docs/DEVELOPING.md](../DEVELOPING.md) 開始，裡面有測試用的本機 Samba、開發掛鉤與程式碼結構。

## License

[MIT](../../LICENSE)
