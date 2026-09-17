---
id: "0020"
title: "À valider : rotation du journal du démon"
type: feat
status: todo
priority: p3
projects: [daemon]
created: 2026-09-17
updated: 2026-09-17
---

## Description

**Proposition : à valider avec l'utilisateur avant de démarrer.**

`~/Library/Logs/coutcouticket/daemon.log` grossit sans limite, surtout depuis que le 0009 y ajoute des lignes horodatées. Proposition : rotation par taille (ex. 1 Mo, 3 fichiers conservés) faite par le démon au démarrage et à l'écriture, ou délégation à `newsyslog` (à trancher).

## Critères d'acceptation

- [ ] Taille du journal bornée et anciens fichiers conservés en nombre limité
- [ ] Aucune ligne perdue pendant la rotation

## Notes

