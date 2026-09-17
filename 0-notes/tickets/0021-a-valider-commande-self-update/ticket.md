---
id: "0021"
title: "À valider : commande self-update"
type: feat
status: todo
priority: p3
projects: [core]
created: 2026-09-17
updated: 2026-09-17
---

## Description

**Proposition : à valider avec l'utilisateur avant de démarrer.**

Le 0003 fournit `install.sh` et écarte un client HTTP embarqué (D4). Proposition : `coutcouticket self-update [--version X]` qui lance `curl … install.sh | sh -s -- --dir <dossier du binaire courant>`, puis affiche l'ancienne et la nouvelle version. Au passage, `daemon install` pourrait gérer lui-même la course `bootout`/`bootstrap` (nouvel essai en Rust au lieu du script).

## Critères d'acceptation

- [ ] self-update met à jour le binaire au même emplacement et relance le démon
- [ ] daemon install réessaie de lui-même si bootstrap échoue juste après bootout

## Notes

