---
id: "0007"
title: "Rendre setup-claude --apply idempotent"
type: fix
status: review
priority: p2
projects: [core]
created: 2026-09-17
updated: 2026-09-17
---

## Description

Constaté dans le ticket 0002 (critère 2).

Quand le serveur `coutcouticket` est déjà enregistré dans Claude Code, `coutcouticket setup-claude --apply` échoue : `claude mcp add` répond « MCP server coutcouticket already exists in user config ». `src/main.rs` (`Cmd::SetupClaude`) propose alors de lancer à la main la même commande `claude mcp add`, qui échouerait de la même façon. L'enregistrement existant reste intact : aucune perte, mais le message induit en erreur.

Objectif : relancer la commande doit être sans danger. Si l'enregistrement existant est identique (même URL, même jeton), ne rien faire et le dire. S'il diffère (par exemple jeton ou port changé), le remplacer (`claude mcp remove` puis `add`) ou indiquer exactement les deux commandes à lancer.

## Critères d'acceptation

- [x] Deux `setup-claude --apply` successifs : le second se termine avec le code 0 et indique que l'enregistrement est déjà à jour
- [ ] Enregistrement existant avec un autre jeton ou une autre URL : il est remplacé, et `claude mcp list` montre la nouvelle valeur
- [x] Aucun message d'erreur ne propose une commande manuelle qui échouerait de la même façon
- [x] Test couvrant les cas « absent », « identique » et « différent » (binaire claude simulé)

## Notes

- Critère 2 prouvé avec le binaire `claude` simulé (`tests/e2e.rs`, `setup_claude_idempotent`) ; vérification réelle restante côté utilisateur (voir journal).

