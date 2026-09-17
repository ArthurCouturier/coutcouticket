---
id: "0018"
title: "Ne pas attribuer au ticket de la branche un commit de notes d'autres tickets"
type: fix
status: todo
priority: p2
projects: [core]
created: 2026-09-17
updated: 2026-09-17
---

## Description

Constaté le 2026-09-17 : le commit 21e60b7 (clôture de 0001 et 0002, création de 0009), fait sur la branche du 0008, porte le trailer `Ticket: 0008`. Résultat : `coutcouticket files 0008` liste des fichiers d'autres tickets. Le hook `prepare-commit-msg` déduit l'id uniquement du nom de la branche.

Objectif : quand le commit ne touche que des notes (`0-notes/tickets/<id>-…`) d'autres tickets, ne pas ajouter le trailer de la branche, ou ajouter ceux des tickets touchés (à décider). Et `files` ne compte pas les fichiers de notes d'autres tickets.

## Critères d'acceptation

- [ ] Un commit sur la branche d'un ticket qui ne modifie que les notes d'autres tickets ne reçoit pas le trailer de la branche (ou reçoit ceux des tickets touchés, selon la décision consignée)
- [ ] files <id> n'inclut pas les notes d'autres tickets
- [ ] Test e2e couvrant le cas

## Notes

