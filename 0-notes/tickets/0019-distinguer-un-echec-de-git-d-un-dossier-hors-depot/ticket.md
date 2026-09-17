---
id: "0019"
title: "Distinguer un échec de git d'un dossier hors dépôt"
type: fix
status: done
priority: p2
projects: [core]
created: 2026-09-17
updated: 2026-09-17
---

## Description

Constaté le 2026-09-17 : la licence Xcode n'étant pas acceptée, `git` renvoie le code 69. `coutcouticket files` affiche alors « le projet n'est pas un dépôt git », et `ticket_context` renvoie `current_branch: null` sans explication. Le diagnostic est faux et fait perdre du temps.

Objectif : quand git échoue (binaire absent, code de sortie inattendu, stderr non vide), le message reprend la commande, le code et la sortie d'erreur de git, avec la correction connue (licence Xcode : `sudo xcodebuild -license`). Le cas « pas un dépôt » reste distinct.

## Critères d'acceptation

- [x] git en échec (code ≠ 0 hors cas « pas un dépôt ») : message avec commande, code et stderr
- [x] Licence Xcode non acceptée : correction `sudo xcodebuild -license` proposée
- [x] ticket_context expose l'erreur git au lieu d'un simple null
- [x] Tests avec un faux binaire git

## Notes

