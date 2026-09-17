---
id: "0001"
title: "Socle v0 : cœur, CLI, MCP, démon, hooks et plugin"
type: feat
status: review
priority: p1
projects: [core, mcp, daemon, plugin]
created: 2026-09-17
updated: 2026-09-17
---

## Description

Première version complète de coutcouticket : cœur déterministe partagé, CLI, serveur MCP (stdio et HTTP), démon avec surveillance des notes et LaunchAgent macOS, hooks git et SessionStart, plugin Claude Code (skill ticket). Construite et testée sous Linux ; la validation macOS fait l'objet du ticket 0002.

## Critères d'acceptation

- [x] init crée et répare l'arborescence de façon idempotente
- [x] Convention de branche <type>/<id sur 4 chiffres>-<slug> appliquée par pre-commit
- [x] Trailer Ticket: <id> ajouté automatiquement aux commits
- [x] 9 outils MCP exposés en stdio et via le démon HTTP authentifié
- [x] Démon : BOARD.md régénéré sur écriture, aucun CPU au repos
- [x] Tests unitaires et de bout en bout au vert

## Notes

