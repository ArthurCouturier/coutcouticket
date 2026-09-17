---
id: "0003"
title: "Distribuer le binaire sans toolchain Rust"
type: ci
status: todo
priority: p2
projects: [core]
created: 2026-09-17
updated: 2026-09-17
---

## Description

Aujourd'hui l'installation passe par cargo install. Produire un binaire macOS arm64 publié à chaque tag, et une commande de mise à jour qui redémarre le démon.

## Critères d'acceptation

- [ ] Workflow de release produisant un binaire macOS arm64
- [ ] Procédure de mise à jour documentée (binaire + daemon install)

## Notes

