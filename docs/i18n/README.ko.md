<div align="center">

<img src="../../assets/icon-1024.png" width="112" alt="Neat NAS">

# Neat NAS

**NAS는 한 번만 기억하세요. 그다음부터는 열면 바로 파일입니다.**

Mac과 Windows를 위한 가장 가벼운 NAS 파일 브라우저. 주소, 사용자 이름, 비밀번호를 한 번만 저장하면 실행할 때마다 곧바로 파일이 열립니다.

[**macOS용 다운로드**](https://github.com/jason-jm/neat-nas/releases/latest) · [**Windows용 다운로드**](https://github.com/jason-jm/neat-nas/releases/latest) · [웹사이트](https://jason-jm.github.io/neat-nas/)

[English](../../README.md) · [简体中文](README.zh-CN.md) · [繁體中文](README.zh-TW.md) · [日本語](README.ja.md) · [한국어](README.ko.md) · [Deutsch](README.de.md) · [Español](README.es.md) · [Français](README.fr.md) · [Português](README.pt-BR.md) · [Русский](README.ru.md)

</div>

---

## 문제

NAS를 쓰는 사람이라면 다 아는 과정입니다. 파일 하나가 필요할 뿐인데 Finder나 탐색기를 열고, 서버를 찾고, `192.168.1.126`이었는지 `.128`이었는지 떠올리고, 사용자 이름을 치고, 비밀번호를 틀리고, 마운트를 기다린 다음에야 탐색이 시작됩니다. 매번요.

## 해결

Neat NAS는 한 가지를 잘하는 작은 앱입니다. 열면 바로 NAS입니다. 한 번 추가하면 이후에는 실행과 동시에 파일 목록이 열리고, 이미 연결되어 있으며, 비밀번호는 시스템 키체인에서 자동으로 가져옵니다. 마운트도, 드라이브 문자도, `smb://` 입력도 없습니다.

## 제공하는 것

- **열면 바로 파일.** 창이 다 나타나기 전에 마지막으로 쓴 NAS에 연결됩니다.
- **NAS를 스스로 찾음.** Synology, QNAP, TrueNAS, Samba를 돌리는 라즈베리 파이. 네트워크에서 SMB를 알리는 장치는 추가 대화상자에 나타납니다.
- **비밀번호는 키체인에만.** macOS 키체인과 Windows 자격 증명 관리자. 텍스트 파일에는 절대 남기지 않습니다.
- **모든 파일 훑어보기.** 사진, 동영상, 오디오, PDF, 텍스트를 NAS에서 바로 스트리밍. Finder처럼 스페이스 키로.
- **양방향 전송.** 파일과 폴더를 다운로드하거나 업로드하고, Finder로 끌어내고, 끌어다 놓아 업로드합니다.
- **끊겨도 이어지는 전송.** 일시 정지되거나 중단된 전송은 멈춘 곳부터 이어집니다. 앱을 종료해도 마찬가지입니다.
- **즉시 뜨는 사진 격자.** 카메라가 넣어 둔 미리보기를 썸네일로 쓰므로 RAW와 HEIC 폴더도 파일 헤더만 읽습니다.
- **라이트와 다크, 10개 언어.**
- **정말 가벼움.** 순수 Rust 코어와 시스템 웹뷰. 약 10 MB 다운로드, 즉시 실행, Electron 없음.
- **자동 업데이트.** 새 버전은 설정에 나타나고 서명 검증 후 클릭 한 번으로 설치됩니다.

## 설치

**macOS 11 이상**(Apple Silicon과 Intel 유니버설): `.dmg`를 열고 Neat NAS를 응용 프로그램으로 끌어다 놓으세요. 아직 공증되지 않아 처음 실행할 때는 오른쪽 클릭 → 열기가 필요합니다.

**Windows 10/11**: `-setup.exe`를 실행하세요. 현재 사용자에게 설치되며 관리자 권한이 필요 없습니다. 설치 파일이 아직 서명되지 않아 SmartScreen 경고가 뜨면 추가 정보 → 실행을 선택하세요.

설치 파일은 [Releases 페이지](https://github.com/jason-jm/neat-nas/releases/latest)에 있습니다.

## 작동 원리

Neat NAS는 순수 Rust 클라이언트로 SMB 2/3를 직접 사용하므로 아무것도 마운트하지 않고 관리자 권한도 필요 없습니다. 가정 내 네트워크에서 수백 개 항목의 폴더 목록도 수십 밀리초면 됩니다. 미리보기는 내부 `nasfile://` 프로토콜의 범위 요청으로 제공되어 동영상이 다운로드 없이 재생됩니다. UI는 OS 자체 웹뷰(macOS의 WebKit, Windows의 WebView2)에서 도는 React이며 [Tauri 2](https://tauri.app)가 이를 지탱합니다. Electron 앱보다 한 자릿수 작습니다.

## 참여하기

버그 신고, 번역, PR을 환영합니다. [docs/DEVELOPING.md](../DEVELOPING.md)에서 테스트용 로컬 Samba, 개발 훅, 코드 구조를 확인하세요.

## License

[MIT](../../LICENSE)
