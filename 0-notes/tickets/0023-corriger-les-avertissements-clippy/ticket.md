---
id: "0023"
title: "Corriger les avertissements clippy"
type: refacto
status: done
priority: p3
projects: [core]
created: 2026-09-17
updated: 2026-09-17
---

## Description

`cargo clippy` affiche 6 avertissements (board.rs, config.rs, fsutil.rs, hooks.rs…), relevés le 2026-09-17 pendant le 0019. Objectif : les corriger sans changer le comportement, puis ajouter `cargo clippy -- -D warnings` à la CI (`.github/workflows/ci.yml`, créé par le 0003) pour éviter qu'ils reviennent.

## Critères d'acceptation

- [x] `cargo clippy --all-targets -- -D warnings` passe
- [x] La CI lance clippy
- [x] `cargo test` passe

## Notes

- La CI lance clippy : étape « Clippy » de `.github/workflows/ci.yml` (composant `clippy` ajouté au profil `minimal`). Le passage réel sera visible au premier run après fusion.

