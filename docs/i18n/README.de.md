<div align="center">

<img src="../../assets/icon-1024.png" width="112" alt="Neat NAS">

# Neat NAS

**Dein NAS einmal merken. Für immer offen.**

Der leichteste NAS-Dateibrowser für Mac und Windows. Adresse, Benutzername und Passwort ein einziges Mal speichern. Danach öffnet jeder Start direkt deine Dateien.

[**Für macOS laden**](https://github.com/jason-jm/neat-nas/releases/latest) · [**Für Windows laden**](https://github.com/jason-jm/neat-nas/releases/latest) · [Website](https://jason-jm.github.io/neat-nas/)

[English](../../README.md) · [简体中文](README.zh-CN.md) · [繁體中文](README.zh-TW.md) · [日本語](README.ja.md) · [한국어](README.ko.md) · [Deutsch](README.de.md) · [Español](README.es.md) · [Français](README.fr.md) · [Português](README.pt-BR.md) · [Русский](README.ru.md)

</div>

---

## Das Problem

Wer ein NAS hat, kennt das: Du willst eine Datei, öffnest Finder oder Explorer, suchst den Server, überlegst, ob es `192.168.1.126` oder `.128` war, tippst den Benutzernamen, vertippst dich beim Passwort, wartest auf den Mount und darfst endlich stöbern. Jedes. Einzelne. Mal.

## Die Lösung

Neat NAS ist eine kleine Desktop-App, die eine Sache richtig macht: Sie öffnet sich auf deinem NAS. Einmal hinzufügen, und ab dann startet die App direkt in deine Dateien, verbunden, mit dem Passwort aus dem Schlüsselbund des Systems. Kein Mounten, keine Laufwerksbuchstaben, nie wieder `smb://` tippen.

## Was du bekommst

- **Öffnet direkt deine Dateien.** Das zuletzt genutzte NAS ist verbunden, bevor das Fenster fertig eingeblendet ist.
- **Findet dein NAS von selbst.** Synology, QNAP, TrueNAS, ein Raspberry Pi mit Samba: Alles, was SMB im Netzwerk ankündigt, erscheint im Dialog.
- **Passwörter bleiben im Schlüsselbund.** macOS-Schlüsselbund und Windows-Anmeldeinformationsverwaltung, nie eine Textdatei.
- **Übersicht für alles.** Fotos, Video, Audio, PDF und Text streamen direkt vom NAS. Leertaste drücken, wie im Finder.
- **Dateien in beide Richtungen.** Dateien oder ganze Ordner laden und hochladen, in den Finder ziehen, zum Hochladen hineinziehen.
- **Übertragungen, die durchhalten.** Pausierte oder unterbrochene Übertragungen machen weiter, wo sie aufgehört haben, auch nach dem Beenden.
- **Fotoraster ohne Wartezeit.** Miniaturen nutzen die Vorschau, die Kameras in jedes Foto einbetten; RAW- und HEIC-Ordner brauchen nur die Dateiköpfe.
- **Hell und Dunkel, zehn Sprachen.**
- **Wirklich leicht.** Nativer Rust-Kern und die Webview des Systems: etwa 10 MB Download, sofort gestartet, kein Electron.
- **Aktualisiert sich selbst.** Neue Versionen erscheinen in den Einstellungen und werden nach Signaturprüfung mit einem Klick installiert.

## Installation

**macOS 11+** (Apple Silicon und Intel, ein universelles Build): `.dmg` öffnen und Neat NAS in „Programme“ ziehen. Die App ist noch nicht von Apple notarisiert, deshalb meldet macOS beim ersten Start, dass es sie nicht überprüfen kann: Öffne Systemeinstellungen → Datenschutz & Sicherheit, scrolle nach unten und klicke auf **„Dennoch öffnen“**. Unter macOS 14 und älter geht auch Rechtsklick auf die App → Öffnen.

**Windows 10/11**: `-setup.exe` ausführen. Installiert für den aktuellen Benutzer, keine Adminrechte nötig. Solange der Installer nicht signiert ist, zeigt SmartScreen eine Warnung → Weitere Informationen → Trotzdem ausführen.

Beide Installer liegen auf der [Releases-Seite](https://github.com/jason-jm/neat-nas/releases/latest).

## So funktioniert es

Neat NAS spricht SMB 2 und 3 selbst, über einen reinen Rust-Client, also wird nichts gemountet und nichts braucht Adminrechte. Ein Ordner mit Hunderten Einträgen ist im Heimnetz in wenigen Dutzend Millisekunden gelistet. Vorschauen laufen über ein internes `nasfile://`-Protokoll mit Bereichsanfragen, deshalb startet ein Video ohne vorherigen Download. Die Oberfläche ist React in der Webview des Betriebssystems (WebKit auf macOS, WebView2 auf Windows) via [Tauri 2](https://tauri.app), eine Größenordnung kleiner als eine Electron-App.

## Mitmachen

Fehlerberichte, Übersetzungen und Pull Requests sind willkommen. [docs/DEVELOPING.md](../DEVELOPING.md) beschreibt den lokalen Samba-Server für Tests, die Dev-Hooks und den Code-Aufbau.

## License

[MIT](../../LICENSE)
