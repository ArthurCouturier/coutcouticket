---
id: "0015"
title: "À valider : changelog généré depuis les tickets clos"
type: feat
status: todo
priority: p3
projects: [core]
created: 2026-09-17
updated: 2026-09-17
---

## Description

**Proposition : à valider avec l'utilisateur avant de démarrer.**

Les notes de clôture (`status done`, avec son résumé) et les trailers `Ticket:` contiennent déjà de quoi produire des notes de version. La release publiée au 0003 en aurait besoin.

Proposition : `coutcouticket changelog --since <tag|date>`, qui regroupe par type (feat, fix…) les tickets passés à done dans l'intervalle, avec leur résumé de clôture. Sortie Markdown réutilisable dans une GitHub Release.

## Critères d'acceptation

- [ ] Changelog Markdown groupé par type pour un intervalle de dates ou depuis un tag git
- [ ] Utilise le résumé de clôture consigné par `status done`
- [ ] Test sur un projet d'exemple

## Notes

