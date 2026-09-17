---
id: "0003"
title: "Distribuer le binaire sans toolchain Rust"
type: ci
status: done
priority: p2
projects: [core]
created: 2026-09-17
updated: 2026-09-17
---

## Description

Aujourd'hui l'installation passe par cargo install. Produire un binaire macOS arm64 publié à chaque tag, et une commande de mise à jour qui redémarre le démon.

## Critères d'acceptation

- [x] Workflow de release produisant un binaire macOS arm64 (confirmé par v0.1.0, run 35231556877)
- [x] Procédure de mise à jour documentée (binaire + daemon install)

## Notes

- Mise à jour : `install.sh` (racine), testé avec une fausse release locale (`COUTCOUTICKET_BASE_URL=file://…`).
- Doc : `0-notes/doc/distribution.md`.

