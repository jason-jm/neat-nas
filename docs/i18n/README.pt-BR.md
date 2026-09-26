<div align="center">

<img src="../../assets/icon-1024.png" width="112" alt="Neat NAS">

# Neat NAS

**Lembre do seu NAS uma vez. Abra para sempre.**

O navegador de arquivos NAS mais leve para Mac e Windows. Salve endereço, usuário e senha uma única vez. Daí em diante, cada abertura vai direto aos seus arquivos.

[**Baixar para macOS**](https://github.com/jason-jm/neat-nas/releases/latest) · [**Baixar para Windows**](https://github.com/jason-jm/neat-nas/releases/latest) · [Site](https://jason-jm.github.io/neat-nas/)

[English](../../README.md) · [简体中文](README.zh-CN.md) · [繁體中文](README.zh-TW.md) · [日本語](README.ja.md) · [한국어](README.ko.md) · [Deutsch](README.de.md) · [Español](README.es.md) · [Français](README.fr.md) · [Português](README.pt-BR.md) · [Русский](README.ru.md)

</div>

---

## O problema

Quem tem NAS conhece o ritual. Você quer um arquivo, então abre o Finder ou o Explorador, procura o servidor, tenta lembrar se era `192.168.1.126` ou `.128`, digita o usuário, erra a senha, espera a montagem e, enfim, começa a navegar. Toda. Santa. Vez.

## A solução

O Neat NAS é um app pequeno que faz uma coisa bem feita: abre no seu NAS. Adicione uma vez e, a partir daí, o app inicia direto nos seus arquivos, conectado, com a senha buscada nas chaves do sistema. Sem montagem, sem letra de unidade, sem nunca mais digitar `smb://`.

## O que você ganha

- **Abre nos seus arquivos.** O último NAS usado já está conectado antes de a janela terminar de aparecer.
- **Encontra o NAS sozinho.** Synology, QNAP, TrueNAS, um Raspberry Pi com Samba: tudo que anuncia SMB na rede aparece no diálogo de adicionar.
- **Senhas ficam nas chaves do sistema.** Chaves do macOS e Gerenciador de Credenciais do Windows, nunca um arquivo de texto.
- **Visualização Rápida de tudo.** Fotos, vídeo, áudio, PDF e texto vêm direto do NAS. Aperte espaço, como no Finder.
- **Arquivos nos dois sentidos.** Baixe ou envie arquivos e pastas inteiras, arraste para o Finder, solte para enviar.
- **Transferências que resistem.** Transferências pausadas ou interrompidas continuam de onde pararam, mesmo depois de fechar o app.
- **Grades de fotos instantâneas.** As miniaturas usam a prévia que a câmera embute em cada foto; pastas RAW e HEIC só precisam dos cabeçalhos.
- **Claro e escuro, dez idiomas.**
- **Leve de verdade.** Núcleo nativo em Rust e a web view do sistema: cerca de 10 MB de download, abertura instantânea, sem Electron.
- **Atualiza sozinho.** Versões novas aparecem nas Configurações e instalam com um clique depois de verificar a assinatura.

## Instalação

**macOS 11+** (Apple Silicon e Intel, uma só versão universal): abra o `.dmg` e arraste o Neat NAS para Aplicativos. Ele é assinado com um Developer ID e notarizado pela Apple, então abre como qualquer outro app baixado.

**Windows 10/11**: rode o `-setup.exe`. Instala para o usuário atual, sem direitos de administrador. Enquanto o instalador não for assinado, o SmartScreen mostra um aviso → Mais informações → Executar assim mesmo.

Os dois instaladores estão na [página de releases](https://github.com/jason-jm/neat-nas/releases/latest).

## Como funciona

O Neat NAS fala SMB 2 e 3 por conta própria, com um cliente em Rust puro, então nada é montado e nada precisa de administrador. Listar uma pasta com centenas de itens leva dezenas de milissegundos numa rede doméstica. As prévias chegam ao visualizador por um protocolo interno `nasfile://` com requisições por faixa, por isso um vídeo começa a tocar sem baixar antes. A interface é React dentro da web view do próprio sistema (WebKit no macOS, WebView2 no Windows) via [Tauri 2](https://tauri.app), uma ordem de grandeza menor que um app Electron.

## Contribuindo

Relatos de bugs, traduções e pull requests são bem-vindos. Comece por [docs/DEVELOPING.md](../DEVELOPING.md): servidor Samba local para testes, ganchos de desenvolvimento e estrutura do código.

## License

[MIT](../../LICENSE)
