# Code signing policy

Neat NAS releases are built only from tagged commits of this repository, by the scripts and workflows in it. Nothing is signed by hand.

## macOS

The app and the disk image are signed with the maintainer's Apple Developer ID and notarized by Apple (`scripts/build-mac.sh`, `scripts/notarize-mac.sh`).

## Windows

The installer and the zipped `Neat NAS.exe` are built on GitHub Actions (`.github/workflows/release.yml`).

**Status: not signed yet.** The project is preparing its application to the SignPath Foundation's free code signing program for open-source projects. Once accepted, Windows releases will be signed through that program:

> Free code signing provided by [SignPath.io](https://signpath.io), certificate by [SignPath Foundation](https://signpath.org)

## Team roles

- Committers and reviewers: [@jason-jm](https://github.com/jason-jm)
- Approvers of release signing: [@jason-jm](https://github.com/jason-jm)

## Privacy

Neat NAS collects no telemetry. It contacts other systems only in these cases:

- It connects over SMB to the NAS servers you add, and reconnects to the last one when it starts.
- When you open the Add NAS dialog, it looks for SMB servers on your local network with mDNS (Bonjour).
- About five seconds after it starts, it downloads `latest.json` from github.com to see whether a newer version exists. The request carries no personal data. Updates are downloaded from GitHub only when you choose to install one.
- Passwords stay in the macOS Keychain or the Windows Credential Manager and are sent only to the NAS they belong to.
