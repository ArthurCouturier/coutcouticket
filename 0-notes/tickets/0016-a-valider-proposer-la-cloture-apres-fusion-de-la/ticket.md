---
id: "0016"
title: "À valider : proposer la clôture après fusion de la branche"
type: feat
status: todo
priority: p3
projects: [core, plugin]
created: 2026-09-17
updated: 2026-09-17
---

## Description

**Proposition : à valider avec l'utilisateur avant de démarrer.**

Quand la branche d'un ticket est fusionnée dans main, le ticket peut rester en `review` ou `in-progress` par oubli, et sa branche locale traîne.

Proposition : un hook git `post-merge` (installé par `init`) ou une vérification dans `session-start` qui repère les tickets dont la branche est fusionnée et non clos. Le contexte de session les signale (« 0007 fusionné mais en review : clôturer ? »), sans jamais changer de statut automatiquement. En option : `coutcouticket prune` pour supprimer les branches fusionnées des tickets clos.

## Critères d'acceptation

- [ ] Tickets à branche fusionnée mais non clos signalés au démarrage de session
- [ ] Aucun changement de statut automatique
- [ ] Option de nettoyage des branches fusionnées des tickets done ou cancelled, avec confirmation

## Notes

