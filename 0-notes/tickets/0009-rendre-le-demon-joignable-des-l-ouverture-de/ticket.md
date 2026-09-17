---
id: "0009"
title: "Rendre le démon joignable dès l'ouverture de session macOS"
type: fix
status: review
priority: p1
projects: [daemon]
created: 2026-09-17
updated: 2026-09-17
---

## Description

Constat du 2026-09-17 après un redémarrage macOS (ticket 0002) : launchd a lancé le démon à 15:12:13, juste après le boot de 15:11, mais le port 47813 n'écoutait qu'environ 1 min plus tard. La session Claude Code ouverte pendant ce délai a échoué à se connecter au MCP (ConnectionRefused), et il a fallu `/mcp` pour reconnecter.

Cause probable, non prouvée : le plist du LaunchAgent lance le démon en priorité basse (`ProcessType=Background`, `Nice=10`, `LowPriorityIO`), et macOS le ralentit fortement pendant la charge de l'ouverture de session. Autre piste : l'ordre de démarrage (écoute HTTP après l'initialisation du watcher et du registre).

Objectif : le MCP doit être disponible pour la première session Claude Code après l'ouverture de session.

## Critères d'acceptation

- [x] Cause du délai identifiée et mesurée (horodatages dans daemon.log entre le lancement et l'écoute)
- [ ] Après un redémarrage macOS, le port 47813 écoute moins de 5 s après le lancement du processus
- [ ] Une session Claude Code ouverte juste après la connexion voit les outils ticket_* sans /mcp
- [x] Le démon reste à 0 CPU au repos
- [x] cargo test passe

## Notes

- Horodatages et mesure en local en place (5-6 ms lancement → écoute). La mesure au
  vrai redémarrage reste à faire par l'utilisateur (procédure dans le journal).
- L'ancien `run` écoutait déjà avant l'init du watcher : cause retenue = bridage launchd
  (D1). Si le délai persiste au redémarrage, la ligne « processus lancé il y a N ms »
  dira s'il est avant `run` (exec, disque, Gatekeeper) ou après.

