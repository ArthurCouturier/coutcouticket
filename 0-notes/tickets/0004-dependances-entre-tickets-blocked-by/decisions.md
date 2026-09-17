# Décisions — ticket 0004

<!-- Une entrée par décision, via ticket_decide / « coutcouticket decide ».
     Une décision remplacée n'est jamais supprimée : on ajoute une nouvelle entrée qui la cite. -->

## D1 — Surface pour poser les dépendances (2026-09-17)

**Décision :** Option --blocked-by à la création (CLI new, champ blocked_by de ticket_create) et nouvelle commande « depend <id> --on <ids> [--remove] » exposée en MCP par un outil dédié ticket_depend. Chaque ajout ou retrait est consigné dans le journal.

**Alternatives écartées :** Seulement à la création : impossible de réparer ou compléter après coup sans éditer le frontmatter. Outil générique ticket_update : surface plus large et moins contrainte que le reste des outils, qui sont tous à intention unique. Étendre ticket_set_status : mélange deux notions.

**Pourquoi :** Un outil à intention unique garde le schéma MCP explicite (id = celui qui attend, on = prérequis) et permet de refuser proprement les cycles au moment de l'ajout. Le retrait ne vérifie pas l'existence des ids pour permettre de réparer une référence cassée.

## D2 — Sens de « bloqué » et affichage (2026-09-17)

**Décision :** Une dépendance bloque tant que son statut est ouvert (ni done ni cancelled). BOARD.md ajoute une colonne « Bloqué par » (liens vers les dépendances encore ouvertes) aux seules sections de statut ouvert. Le statut blocked reste manuel et indépendant ; start n'est pas refusé sur un ticket bloqué (avertissement CLI et hook SessionStart).

**Alternatives écartées :** Passer automatiquement le ticket en blocked : écrit dans les frontmatters à chaque changement de statut d'un autre ticket, surprenant. Refuser start : trop rigide (travail préparatoire possible). Mention dans la cellule du titre : moins lisible qu'une colonne.

**Pourquoi :** Le board reste une pure vue dérivée des fichiers, sans écriture en cascade. La colonne ne change que les tables ouvertes ; les tickets fermés gardent l'ancien format. Les problèmes de dépendances (id inconnu, auto-référence, cycle) sont ajoutés au scan, donc visibles dans validate, BOARD.md et le contexte de session.
