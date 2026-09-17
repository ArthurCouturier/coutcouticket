---
id: "0022"
title: "À valider : lister les tickets prêts à démarrer"
type: feat
status: todo
priority: p3
projects: [core, mcp]
created: 2026-09-17
updated: 2026-09-17
---

## Description

**Proposition : à valider avec l'utilisateur avant de démarrer.**

Depuis le 0004 (`blocked_by`), le board montre les dépendances ouvertes, mais rien ne répond directement à « que puis-je démarrer maintenant ? ». Proposition : `list --ready` (et un filtre `ready` dans `ticket_list`) qui renvoie les tickets todo sans dépendance ouverte, triés par priorité. Le skill l'utiliserait pour « quoi faire ensuite ». À décider aussi : `status done` signale les tickets qu'il débloque.

## Critères d'acceptation

- [ ] list --ready et ticket_list(ready) renvoient les tickets todo sans dépendance ouverte
- [ ] Le skill utilise ce filtre pour « quoi faire ensuite »

## Notes

