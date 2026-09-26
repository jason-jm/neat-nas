<div align="center">

<img src="../../assets/icon-1024.png" width="112" alt="Neat NAS">

# Neat NAS

**NAS の登録は一度だけ。あとは開けばファイルがそこに。**

Mac と Windows で使える、いちばん軽い NAS ファイルブラウザ。アドレス・ユーザー名・パスワードを一度保存すれば、起動するたびにすぐファイルが開きます。

[**macOS 版をダウンロード**](https://github.com/jason-jm/neat-nas/releases/latest) · [**Windows 版をダウンロード**](https://github.com/jason-jm/neat-nas/releases/latest) · [公式サイト](https://jason-jm.github.io/neat-nas/)

[English](../../README.md) · [简体中文](README.zh-CN.md) · [繁體中文](README.zh-TW.md) · [日本語](README.ja.md) · [한국어](README.ko.md) · [Deutsch](README.de.md) · [Español](README.es.md) · [Français](README.fr.md) · [Português](README.pt-BR.md) · [Русский](README.ru.md)

</div>

---

## 問題

NAS を使っている人ならお馴染みの流れ。ファイルが 1 つ欲しいだけなのに、Finder やエクスプローラーを開き、サーバーを探し、`192.168.1.126` だったか `.128` だったか思い出し、ユーザー名を打ち、パスワードを間違え、マウントを待ち、ようやく閲覧開始。毎回、毎回。

## 解決

Neat NAS は「NAS を開く」ことだけに集中した小さなアプリです。一度追加すれば、以後は起動と同時にファイル一覧が開き、接続済みで、パスワードはシステムのキーチェーンから自動で取り出されます。マウントもドライブ文字もなく、`smb://` を打つこともありません。

## できること

- **開いた瞬間にファイル。** ウィンドウが表示し終わる前に、最後に使った NAS へ接続済み。
- **NAS を自動で発見。** Synology、QNAP、TrueNAS、Samba を動かした Raspberry Pi。ネットワークで SMB を告知している機器は追加ダイアログに現れます。
- **パスワードはキーチェーンに。** macOS のキーチェーンと Windows の資格情報マネージャー。テキストファイルには書きません。
- **何でもクイックルック。** 写真・動画・音声・PDF・テキストを NAS から直接ストリーミング。Finder と同じくスペースキーで。
- **双方向の転送。** ファイルやフォルダのダウンロードとアップロード。Finder へドラッグ、ドロップでアップロード。
- **途切れても続く転送。** 一時停止や中断した転送は止まった場所から再開。アプリを終了しても引き継ぎます。
- **一瞬で並ぶ写真グリッド。** サムネイルはカメラが埋め込んだプレビューを利用。RAW や HEIC もヘッダーを読むだけ。
- **ライトとダーク、10 言語。**
- **本当に軽い。** 純 Rust のコアとシステムの WebView。ダウンロードは約 10 MB、起動は一瞬、Electron なし。
- **自動アップデート。** 新バージョンは設定に現れ、ワンクリックで署名検証のうえインストール。

## インストール

**macOS 11 以降**（Apple シリコンと Intel のユニバーサル）：`.dmg` を開き、Neat NAS をアプリケーションへドラッグ。まだ Apple の公証を受けていないため、初回起動時に確認できないという警告が出ます。「システム設定」→「プライバシーとセキュリティ」を開き、下の方の **「このまま開く」** をクリックしてください。macOS 14 以前では、アプリを右クリックして「開く」を選ぶ方法も使えます。

**Windows 10/11**：`-setup.exe` を実行。現在のユーザーにインストールされ、管理者権限は不要です。インストーラーは未署名のため、SmartScreen の警告が出たら「詳細情報」→「実行」を選んでください。

インストーラーは [Releases ページ](https://github.com/jason-jm/neat-nas/releases/latest) にあります。

## しくみ

Neat NAS は純 Rust 製クライアントで SMB 2/3 を直接話すため、何もマウントせず管理者権限も要りません。家庭内 LAN なら数百件のフォルダ一覧も数十ミリ秒。プレビューは内部の `nasfile://` プロトコルで範囲リクエストとして配信されるので、動画はダウンロードせずに再生が始まります。UI は OS 標準の WebView（macOS は WebKit、Windows は WebView2）で動く React で、[Tauri 2](https://tauri.app) が支えています。Electron アプリより一桁小さいサイズです。

## 参加する

バグ報告、翻訳、プルリクエストを歓迎します。[docs/DEVELOPING.md](../DEVELOPING.md) にテスト用のローカル Samba、開発フック、コード構成をまとめています。

## License

[MIT](../../LICENSE)
