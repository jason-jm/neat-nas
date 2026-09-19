<div align="center">

<img src="../../assets/icon-1024.png" width="112" alt="Neat NAS">

# Neat NAS

**Recuerda tu NAS una vez. Ábrelo para siempre.**

El explorador de archivos NAS más ligero para Mac y Windows. Guarda la dirección, el usuario y la contraseña una sola vez. Desde entonces, cada inicio abre directamente tus archivos.

[**Descargar para macOS**](https://github.com/jason-jm/neat-nas/releases/latest) · [**Descargar para Windows**](https://github.com/jason-jm/neat-nas/releases/latest) · [Sitio web](https://jason-jm.github.io/neat-nas/)

[English](../../README.md) · [简体中文](README.zh-CN.md) · [繁體中文](README.zh-TW.md) · [日本語](README.ja.md) · [한국어](README.ko.md) · [Deutsch](README.de.md) · [Español](README.es.md) · [Français](README.fr.md) · [Português](README.pt-BR.md) · [Русский](README.ru.md)

</div>

---

## El problema

Quien tiene un NAS conoce el ritual. Quieres un archivo, así que abres el Finder o el Explorador, buscas el servidor, intentas recordar si era `192.168.1.126` o `.128`, escribes el usuario, te equivocas con la contraseña, esperas el montaje y por fin empiezas a navegar. Todas. Las. Veces.

## La solución

Neat NAS es una pequeña app de escritorio que hace bien una sola cosa: se abre en tu NAS. Añádelo una vez y, desde entonces, la app arranca directamente en tus archivos, conectada, con la contraseña recuperada del llavero del sistema. Sin montajes, sin letras de unidad, sin volver a escribir `smb://`.

## Lo que obtienes

- **Se abre en tus archivos.** El último NAS que usaste ya está conectado antes de que termine de aparecer la ventana.
- **Encuentra tu NAS solo.** Synology, QNAP, TrueNAS, una Raspberry Pi con Samba: todo lo que anuncie SMB en la red aparece en el diálogo de añadir.
- **Las contraseñas se quedan en el llavero.** Llavero de macOS y Administrador de credenciales de Windows, nunca un archivo de texto.
- **Vista rápida para todo.** Fotos, vídeo, audio, PDF y texto se reproducen directamente desde el NAS. Pulsa espacio, como en el Finder.
- **Archivos en ambos sentidos.** Descarga o sube archivos y carpetas enteras, arrastra al Finder, suelta para subir.
- **Transferencias que sobreviven.** Las transferencias en pausa o interrumpidas continúan donde se quedaron, incluso después de cerrar la app.
- **Cuadrículas de fotos al instante.** Las miniaturas usan la vista previa que la cámara incrusta en cada foto; las carpetas RAW y HEIC solo necesitan las cabeceras.
- **Claro y oscuro, diez idiomas.**
- **Ligero de verdad.** Núcleo nativo en Rust y el web view del sistema: unos 10 MB de descarga, arranque instantáneo, sin Electron.
- **Se actualiza sola.** Las versiones nuevas aparecen en Ajustes y se instalan con un clic tras verificar la firma.

## Instalación

**macOS 11+** (Apple Silicon e Intel, una sola compilación universal): abre el `.dmg` y arrastra Neat NAS a Aplicaciones. La app aún no está notarizada, así que el primer inicio necesita clic derecho → Abrir.

**Windows 10/11**: ejecuta `-setup.exe`. Se instala para el usuario actual, sin permisos de administrador. Hasta que el instalador esté firmado, SmartScreen muestra un aviso → Más información → Ejecutar de todas formas.

Ambos instaladores están en la [página de versiones](https://github.com/jason-jm/neat-nas/releases/latest).

## Cómo funciona

Neat NAS habla SMB 2 y 3 por sí mismo, con un cliente escrito en Rust, así que no monta nada ni necesita permisos de administrador. Listar una carpeta con cientos de entradas tarda decenas de milisegundos en una red doméstica. Las vistas previas llegan al visor integrado por un protocolo interno `nasfile://` con peticiones por rangos, por eso un vídeo empieza a reproducirse sin descargarlo antes. La interfaz es React dentro del web view del propio sistema (WebKit en macOS, WebView2 en Windows) mediante [Tauri 2](https://tauri.app), un orden de magnitud más pequeña que una app Electron.

## Contribuir

Se agradecen informes de errores, traducciones y pull requests. Empieza por [docs/DEVELOPING.md](../DEVELOPING.md): describe el servidor Samba local para pruebas, los ganchos de desarrollo y la estructura del código.

## License

[MIT](../../LICENSE)
