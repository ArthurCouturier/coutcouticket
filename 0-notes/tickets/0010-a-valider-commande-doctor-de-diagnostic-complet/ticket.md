---
id: "0010"
title: "À valider : commande doctor de diagnostic complet"
type: feat
status: todo
priority: p3
projects: [core]
created: 2026-09-17
updated: 2026-09-17
---

## Description

**Proposition : à valider avec l'utilisateur avant de démarrer.**

Aujourd'hui, un problème d'installation se diagnostique à la main, commande par commande (daemon status, plist, claude mcp get, PATH…). Le 2026-09-17, deux pannes n'ont été vues que par hasard : la licence Xcode non acceptée (git et cc renvoient le code 69, et le démon ne voit plus la branche courante) et le démon qui écoute tard après l'ouverture de session.

Proposition : `coutcouticket doctor` (et éventuellement un outil MCP `notes_doctor`) qui vérifie en une passe, avec pour chaque point un statut et la correction à appliquer :
- binaire trouvé dans le PATH et au chemin noté dans les hooks git et dans le plist ;
- git utilisable (y compris le cas de la licence Xcode) ;
- LaunchAgent chargé, démon joignable, versions du démon et de la CLI identiques ;
- enregistrement MCP dans Claude Code (URL et jeton identiques à daemon.toml) ;
- projet enregistré, hooks git à jour, bloc CLAUDE.md présent, `validate` sans erreur.

## Critères d'acceptation

- [ ] `coutcouticket doctor` liste chaque vérification avec ok/échec et la correction à appliquer
- [ ] Code de sortie non nul si une vérification échoue
- [ ] Le cas de la licence Xcode non acceptée est détecté et expliqué
- [ ] Tests avec environnement simulé (démon arrêté, binaire déplacé, enregistrement MCP différent)

## Notes

