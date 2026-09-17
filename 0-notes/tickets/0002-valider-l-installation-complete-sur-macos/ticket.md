---
id: "0002"
title: "Valider l'installation complète sur macOS"
type: ci
status: in-progress
priority: p0
projects: [daemon, plugin]
created: 2026-09-17
updated: 2026-09-17
---

## Description

Tout a été testé sous Linux (inotify). Le LaunchAgent, FSEvents, l'enregistrement MCP dans Claude Code et l'installation du plugin ne peuvent être validés que sur le Mac.

## Critères d'acceptation

- [ ] cargo install --path . puis coutcouticket daemon install : le démon répond après redémarrage de session
- [x] setup-claude --apply : les outils ticket_* apparaissent dans Claude Code
- [x] Plugin installé : le contexte coutcouticket est injecté au démarrage de session
- [x] Édition manuelle d'un ticket : BOARD.md régénéré en moins de 2 s (FSEvents)
- [ ] Hooks git fonctionnels depuis le terminal et depuis le client git habituel (PATH)
- [ ] init lancé sur un vrai projet existant sans perte de contenu

## Notes

