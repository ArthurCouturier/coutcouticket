---
id: "0014"
title: "À valider : recherche plein texte dans les notes"
type: feat
status: todo
priority: p3
projects: [core, mcp]
created: 2026-09-17
updated: 2026-09-17
---

## Description

**Proposition : à valider avec l'utilisateur avant de démarrer.**

La méthode de recherche documentée repose sur `rg`, qui n'est pas toujours installé et ne sait rien de la structure : il ne donne ni le ticket, ni le statut, ni la section (journal, décision, doc) où se trouve le résultat.

Proposition : `coutcouticket search <texte>` et un outil MCP `notes_search`, avec des filtres (statut, type de fichier : ticket, journal, décisions ou doc ; projet de dev). Chaque résultat indique l'id, le titre, le fichier, la ligne et un extrait. Aucune indexation persistante : un balayage suffit à cette échelle (quelques centaines de tickets).

## Critères d'acceptation

- [ ] Commande et outil MCP renvoyant id, titre, fichier, ligne et extrait
- [ ] Filtres par statut et par type de fichier
- [ ] Recherche insensible à la casse et aux accents
- [ ] Moins de 50 ms sur 200 tickets

## Notes

