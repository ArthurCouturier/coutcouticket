---
id: "0024"
title: "Passer les actions artifact de la release à Node 24"
type: ci
status: review
priority: p3
projects: [core]
created: 2026-09-17
updated: 2026-09-17
---

## Description

Le run de release v0.1.0 (35231556877) affiche un avertissement : `actions/upload-artifact@v4` et `actions/download-artifact@v4` visent Node.js 20, qui est déprécié, et GitHub les exécute de force sous Node 24. Objectif : passer à des versions qui ciblent Node 24 (vérifier les versions majeures publiées) avant que Node 20 ne soit retiré.

## Critères d'acceptation

- [ ] Le workflow release ne montre plus d'avertissement Node 20
- [ ] Une release de test (tag de préversion ou workflow_dispatch) passe

## Notes

