---
id: "0025"
title: "Démon sous Windows"
type: feat
status: done
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

- [x] `cargo build` et `cargo test` passent sur windows-latest en CI
- [x] `daemon install` sous Windows enregistre un démarrage automatique à l'ouverture de session sans droits administrateur ; `daemon uninstall` le retire ; `daemon status` répond
- [x] Hooks git fonctionnels avec Git for Windows (test e2e sur la CI Windows)
- [x] Binaire Windows publié par la release et procédure d'installation documentée
- [x] Aucune régression macOS (CI macOS verte)

## Notes

Preuves (PR brouillon #1, branche feat/0025-demon-sous-windows) :
- CI run 35235901067 : `test (macos-15)` et `test (windows-latest)` verts (clippy `-D warnings`,
  build, `cargo test --no-fail-fast`, dont `hooks_git_for_windows` et
  `daemon_tache_planifiee_windows`), puis `install.ps1` sous PowerShell 5.1 : installation,
  `daemon install`, mise à jour démon en marche, `daemon status`, `daemon uninstall`.
- Release en essai (run 35235915058, `-f essai=true`) : `build (x86_64-pc-windows-msvc)` produit
  `coutcouticket-0.1.0-x86_64-pc-windows-msvc.zip` et `.sha256`, rassemblés et vérifiés dans
  `SHA256SUMS` ; la publication (même étape, `*.zip` ajouté) n'a lieu qu'au prochain tag.

Critère 2 : validé par l'utilisateur sur un poste Windows le 2026-09-17 (voir le journal).
La procédure prévue (poste Windows 10/11, compte standard ; le type de compte utilisé n'a pas été précisé) :
`daemon install` sans invite UAC, `daemon status` → `ok`, aucune fenêtre console après
déconnexion puis reconnexion, trailer `Ticket:` ajouté aux commits (Git Bash et client
graphique), `daemon uninstall` qui retire la tâche `coutcouticket-daemon`.
