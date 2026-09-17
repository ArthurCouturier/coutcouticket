---
id: "0012"
title: "À valider : sauvegarde des notes hors dépôt"
type: feat
status: todo
priority: p3
projects: [core, daemon]
created: 2026-09-17
updated: 2026-09-17
---

## Description

**Proposition : à valider avec l'utilisateur avant de démarrer.**

Depuis le 0008, `init` place par défaut `0-notes/` hors du dépôt git. Les notes n'ont alors aucun historique ni aucune sauvegarde : un `rm`, un mauvais changement de branche (piège documenté dans le README) ou la perte du disque les efface définitivement.

Pistes à trancher ensemble :
- instantanés compressés faits par le démon après chaque modification (avec anti-rebond) dans `~/.local/share/coutcouticket/backups/<projet>/`, avec rotation ;
- ou un dépôt git privé dédié aux notes (commit automatique par le démon) ;
- plus une commande `coutcouticket restore` pour lister et restaurer un instantané.

## Critères d'acceptation

- [ ] Choix de la stratégie consigné (decisions.md) après validation de l'utilisateur
- [ ] Une modification de note produit une sauvegarde sans action manuelle, avec rotation bornée
- [ ] Une commande restaure une sauvegarde choisie sans écraser l'état courant sans confirmation
- [ ] Le démon reste à 0 CPU au repos

## Notes

