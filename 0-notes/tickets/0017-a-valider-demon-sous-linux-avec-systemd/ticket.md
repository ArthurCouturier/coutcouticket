---
id: "0017"
title: "À valider : démon sous Linux avec systemd"
type: feat
status: todo
priority: p3
projects: [daemon]
created: 2026-09-17
updated: 2026-09-17
---

## Description

**Proposition : à valider avec l'utilisateur avant de démarrer.**

Le cœur, la surveillance des fichiers (inotify) et les tests fonctionnent déjà sous Linux, mais `daemon install` ne gère que le LaunchAgent macOS.

Proposition : `daemon install|uninstall|status` sous Linux, avec une unité systemd utilisateur (`~/.config/systemd/user/coutcouticket.service`, `Restart=always`, `systemctl --user enable --now`), et le binaire Linux ajouté à la release du 0003.

## Critères d'acceptation

- [ ] daemon install/uninstall fonctionnels sous Linux via systemd --user
- [ ] Le démon redémarre après une déconnexion puis reconnexion
- [ ] Binaire Linux x86_64 publié par la release

## Notes

