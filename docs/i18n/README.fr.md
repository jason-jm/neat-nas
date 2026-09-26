<div align="center">

<img src="../../assets/icon-1024.png" width="112" alt="Neat NAS">

# Neat NAS

**Retenez votre NAS une fois. Ouvrez-le pour toujours.**

Le navigateur de fichiers NAS le plus léger pour Mac et Windows. Enregistrez l’adresse, le nom d’utilisateur et le mot de passe une seule fois. Ensuite, chaque lancement s’ouvre directement sur vos fichiers.

[**Télécharger pour macOS**](https://github.com/jason-jm/neat-nas/releases/latest) · [**Télécharger pour Windows**](https://github.com/jason-jm/neat-nas/releases/latest) · [Site web](https://jason-jm.github.io/neat-nas/)

[English](../../README.md) · [简体中文](README.zh-CN.md) · [繁體中文](README.zh-TW.md) · [日本語](README.ja.md) · [한국어](README.ko.md) · [Deutsch](README.de.md) · [Español](README.es.md) · [Français](README.fr.md) · [Português](README.pt-BR.md) · [Русский](README.ru.md)

</div>

---

## Le problème

Tous les possesseurs de NAS connaissent le rituel. Vous voulez un fichier, alors vous ouvrez le Finder ou l’Explorateur, cherchez le serveur, tentez de vous rappeler si c’était `192.168.1.126` ou `.128`, tapez le nom d’utilisateur, ratez le mot de passe, attendez le montage, et enfin vous pouvez naviguer. À. Chaque. Fois.

## La solution

Neat NAS est une petite app de bureau qui fait une seule chose, bien : elle s’ouvre sur votre NAS. Ajoutez-le une fois et, dès lors, l’app démarre directement dans vos fichiers, connectée, le mot de passe récupéré dans le trousseau du système. Pas de montage, pas de lettre de lecteur, plus jamais de `smb://` à taper.

## Ce que vous obtenez

- **S’ouvre sur vos fichiers.** Le dernier NAS utilisé est connecté avant que la fenêtre ait fini d’apparaître.
- **Trouve votre NAS tout seul.** Synology, QNAP, TrueNAS, un Raspberry Pi sous Samba : tout ce qui annonce SMB sur le réseau apparaît dans la fenêtre d’ajout.
- **Les mots de passe restent dans le trousseau.** Trousseau macOS et Gestionnaire d’identifiants Windows, jamais un fichier texte.
- **Coup d’œil sur tout.** Photos, vidéo, audio, PDF et texte sont lus directement depuis le NAS. Appuyez sur Espace, comme dans le Finder.
- **Des fichiers dans les deux sens.** Téléchargez ou envoyez fichiers et dossiers entiers, glissez vers le Finder, déposez pour envoyer.
- **Des transferts qui tiennent.** Les transferts suspendus ou interrompus reprennent là où ils s’étaient arrêtés, même après avoir quitté l’app.
- **Des grilles de photos instantanées.** Les vignettes utilisent l’aperçu que l’appareil intègre dans chaque photo ; les dossiers RAW et HEIC ne lisent que les en-têtes.
- **Clair et sombre, dix langues.**
- **Vraiment léger.** Cœur natif en Rust et web view du système : environ 10 Mo à télécharger, lancement instantané, pas d’Electron.
- **Se met à jour tout seul.** Les nouvelles versions apparaissent dans les Réglages et s’installent en un clic après vérification de la signature.

## Installation

**macOS 11+** (Apple Silicon et Intel, une seule version universelle) : ouvrez le `.dmg` et glissez Neat NAS dans Applications. L’app n’est pas encore notarisée par Apple : au premier lancement, macOS indique qu’il ne peut pas la vérifier. Ouvrez Réglages Système → Confidentialité et sécurité, faites défiler vers le bas et cliquez sur **« Ouvrir quand même »**. Sous macOS 14 et antérieur, un clic droit sur l’app → Ouvrir fonctionne aussi.

**Windows 10/11** : lancez `-setup.exe`. Installation pour l’utilisateur courant, sans droits administrateur. Tant que l’installeur n’est pas signé, SmartScreen affiche un avertissement → Informations complémentaires → Exécuter quand même.

Les deux installeurs sont sur la [page des versions](https://github.com/jason-jm/neat-nas/releases/latest).

## Comment ça marche

Neat NAS parle SMB 2 et 3 lui-même, via un client écrit en Rust : rien n’est monté, aucun droit administrateur n’est requis. Lister un dossier de plusieurs centaines d’entrées prend quelques dizaines de millisecondes sur un réseau domestique. Les aperçus sont servis au lecteur intégré par un protocole interne `nasfile://` avec requêtes par plages, c’est pourquoi une vidéo démarre sans téléchargement préalable. L’interface est du React dans la web view du système (WebKit sur macOS, WebView2 sur Windows) grâce à [Tauri 2](https://tauri.app), un ordre de grandeur plus léger qu’une app Electron.

## Contribuer

Rapports de bugs, traductions et pull requests sont les bienvenus. Commencez par [docs/DEVELOPING.md](../DEVELOPING.md) : serveur Samba local pour les tests, hooks de développement et organisation du code.

## License

[MIT](../../LICENSE)
