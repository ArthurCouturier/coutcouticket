---
id: "0013"
title: "À valider : démarrer un ticket dans un worktree git"
type: feat
status: todo
priority: p3
projects: [core, mcp, plugin]
created: 2026-09-17
updated: 2026-09-17
---

## Description

**Proposition : à valider avec l'utilisateur avant de démarrer.**

Le 2026-09-17, cinq tickets ont été traités en parallèle par des sous-agents, chacun dans son worktree git, créé à la main. Problèmes rencontrés : les outils MCP ne connaissent pas les worktrees (le démon n'accepte que les projets enregistrés), il a fallu passer par la CLI, et les notes (`0-notes/`, et BOARD.md en particulier) divergent d'une branche à l'autre.

Proposition : `ticket_start` avec une option `worktree` qui crée `../<projet>-wt/<id>` sur la branche du ticket. Le démon reconnaît les worktrees d'un projet enregistré. Il faut aussi définir où vivent les notes dans ce cas (partagées avec le dépôt principal ou propres à chaque branche) et comment le board se réconcilie à la fusion. Commande associée pour supprimer le worktree à la clôture.

## Critères d'acceptation

- [ ] `ticket_start` (MCP et CLI) peut créer un worktree sur la branche du ticket
- [ ] Les outils MCP acceptent un worktree d'un projet enregistré
- [ ] Stratégie des notes dans un worktree décidée et documentée
- [ ] Suppression du worktree proposée à la clôture

## Notes

- Constaté le 2026-09-17 (0004, 0018) : `Project::name()` prend le nom du dossier racine, si bien que le titre de BOARD.md généré dans un worktree devient « Tableau des tickets — 0018 ». Prendre le nom du dépôt principal (ou une clé de config).
