---
id: "0025"
title: "Démon sous Windows"
type: feat
status: in-progress
priority: p2
projects: [daemon, core]
created: 2026-09-17
updated: 2026-09-17
---

## Description

Demandé par l'utilisateur le 2026-09-17. Aujourd'hui, `daemon install|uninstall|status` ne gère que le LaunchAgent macOS, et plusieurs parties supposent Unix : hooks git en shell, verrou dans le dossier temporaire, chemins, `proc_pidinfo`, etc.

Objectif : le binaire compile et fonctionne sous Windows (x86_64-pc-windows-msvc), et le démon démarre automatiquement à l'ouverture de session.
- **Démarrage automatique :** tâche planifiée à l'ouverture de session (`schtasks /Create /SC ONLOGON`, relance en cas d'échec), ou clé `HKCU\…\Run`, sans droits administrateur. Choix à consigner.
- **Portabilité :** chemins de config (`%APPDATA%`), journal (`%LOCALAPPDATA%`), hooks git compatibles avec Git for Windows (sh fourni par Git), watcher (ReadDirectoryChangesW via notify), verrou inter-processus.
- **CI et release :** tests sur `windows-latest` et archive `.zip` Windows publiée ; `install.ps1` éventuel.

## Critères d'acceptation

- [ ] `cargo build` et `cargo test` passent sur windows-latest en CI
- [ ] `daemon install` sous Windows enregistre un démarrage automatique à l'ouverture de session sans droits administrateur ; `daemon uninstall` le retire ; `daemon status` répond
- [ ] Hooks git fonctionnels avec Git for Windows (test e2e sur la CI Windows)
- [ ] Binaire Windows publié par la release et procédure d'installation documentée
- [ ] Aucune régression macOS (CI macOS verte)

## Notes

