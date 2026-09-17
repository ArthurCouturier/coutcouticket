# Notes du projet — fonctionnement

Ce dossier est géré avec **coutcouticket**. Il centralise les tickets, la
documentation technique et la vue d'ensemble du projet.

## Arborescence

```
0-notes/
├── 0-global/
│   ├── README.md     ← ce fichier : fonctionnement et méthode de recherche
│   └── BOARD.md      ← GÉNÉRÉ : vue de tous les tickets. Ne jamais éditer.
├── tickets/
│   └── 0013-add-thing-to-etc/
│       ├── ticket.md      ← frontmatter (source de vérité) + description + critères
│       ├── journal.md     ← avancement, append-only, finit par « Prochaine étape »
│       └── decisions.md   ← choix pris, alternatives, pourquoi
└── doc/
    ├── INDEX.md      ← une ligne par page : sujet + « lire quand… »
    └── *.md          ← documentation technique à jour
```

## Règles

1. **Le frontmatter de `ticket.md` ne s'édite jamais à la main.** Il passe par
   les outils (MCP `ticket_*` ou CLI `coutcouticket`). Le corps (description,
   critères, notes) peut être édité librement.
2. **`BOARD.md` est régénéré automatiquement** à chaque modification.
3. **`journal.md` est append-only** : on ajoute, on ne réécrit pas.
4. **Une décision remplacée n'est pas supprimée** : nouvelle entrée qui la cite.
5. **La doc décrit l'état actuel**, les tickets racontent l'histoire. Clôturer
   un ticket implique de mettre à jour la doc concernée et `doc/INDEX.md`.

## Statuts

`todo` → `in-progress` → `review` → `done`, avec `blocked` et `cancelled`.

## Dépendances

Un ticket peut attendre d'autres tickets (`blocked_by` dans le frontmatter), posé via
`ticket_create`/`ticket_depend` ou `coutcouticket new --blocked-by 3` /
`coutcouticket depend 13 --on 3 [--remove]`. Ids inconnus et cycles sont refusés.
`BOARD.md` affiche, pour chaque ticket ouvert, les dépendances encore ouvertes
(colonne « Bloqué par ») ; une dépendance `done` ou `cancelled` ne bloque plus.

## Convention git (stricte)

- Branche : `<type>/<id>-<slug>`, ex. `feat/0013-add-thing-to-etc`.
  Le suffixe est exactement le nom du dossier du ticket.
- Types autorisés et branches exemptées : voir `.coutcouticket.toml`.
- Le hook `pre-commit` refuse un commit sur une branche non conforme.
- Le hook `prepare-commit-msg` ajoute automatiquement le trailer `Ticket: 0013`.
- Fichiers modifiés par un ticket : `coutcouticket files 13` (dérivé de git).

## Chercher efficacement

| Besoin | Méthode |
|--------|---------|
| Vue d'ensemble, quoi faire ensuite | lire `0-global/BOARD.md` |
| Reprendre un ticket | `ticket_context` (MCP) ou `coutcouticket show 13` |
| Comprendre une partie du code | `doc/INDEX.md` puis la page indiquée |
| Pourquoi un choix a été fait | `rg -n "<mot-clé>" 0-notes/tickets/*/decisions.md` |
| Tickets d'un projet ou d'un statut | `coutcouticket list --project backend --status todo` |
| Retrouver un sujet dans tout l'historique | `rg -n "<mot-clé>" 0-notes/tickets` |
| Qui a touché un fichier | `git log --grep "Ticket:" -- <fichier>` |
