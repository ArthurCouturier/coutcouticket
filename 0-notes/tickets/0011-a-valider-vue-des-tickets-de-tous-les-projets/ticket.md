---
id: "0011"
title: "À valider : vue des tickets de tous les projets"
type: feat
status: todo
priority: p3
projects: [core, mcp, daemon]
created: 2026-09-17
updated: 2026-09-17
---

## Description

**Proposition : à valider avec l'utilisateur avant de démarrer.**

Le démon connaît déjà le registre de tous les projets suivis (ici coutcouticket et je-taime-app), mais chaque board ne montre qu'un seul projet. Quand on jongle entre plusieurs projets, rien ne répond à la question « quoi faire maintenant, tous projets confondus ? ».

Proposition : `coutcouticket overview` et un outil MCP `tickets_overview`, qui listent les tickets ouverts (en cours, en revue, bloqués, puis à faire par priorité) de tous les projets enregistrés, avec le nom du projet. Option : un fichier généré `~/.config/coutcouticket/OVERVIEW.md`, régénéré par le démon.

## Critères d'acceptation

- [ ] Commande et outil MCP listant les tickets ouverts de tous les projets enregistrés, triés par statut puis priorité
- [ ] Un projet enregistré mais introuvable est signalé sans faire échouer la vue
- [ ] Test avec au moins deux projets enregistrés

## Notes

