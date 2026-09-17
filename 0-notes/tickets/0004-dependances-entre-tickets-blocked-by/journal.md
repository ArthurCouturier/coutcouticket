# Journal — ticket 0004

<!-- Journal append-only, alimenté par ticket_log / « coutcouticket log ».
     Ne jamais réécrire une entrée passée. Chaque entrée finit par « Prochaine étape ». -->

## 2026-09-17 15:26 — statut : todo → in-progress

Démarrage sur la branche `feat/0004-dependances-entre-tickets-blocked-by`.

## 2026-09-17 15:30 — avancement

Champ blocked_by (model.rs, optionnel, normalisé), contrôles d'écriture et de lecture (ids, auto-référence, cycles) dans store.rs, commande depend, option new --blocked-by, outil MCP ticket_depend, colonne « Bloqué par » du board, blocked_by/open_blockers dans list/show/ticket_context, avertissements start et SessionStart. Tests unitaires store/model et e2e (dependances_cli, mcp_stdio) verts, hors hooks_hors_du_path (environnemental). Doc : architecture.md, INDEX, README, skill (Créer, routage), README global et gabarit.

**Prochaine étape :** Cocher les critères, relancer cargo test et clôturer

## 2026-09-17 15:31 — avancement

Critères vérifiés : validation de blocked_by (ids inconnus, auto-référence, cycles, à l'écriture et à la lecture) et colonne « Bloqué par » du board (dépendances ouvertes seulement), couverts par les tests unitaires et e2e. cargo test vert hors hooks_hors_du_path (licence Xcode). Utiliser blocked_by exige de réinstaller le binaire et de relancer le démon : l'ancien binaire refuse la clé.

**Prochaine étape :** Aucune

## 2026-09-17 15:31 — statut : in-progress → done

Dépendances entre tickets livrées : blocked_by validé, commande depend et outil ticket_depend, colonne « Bloqué par » dans le board.
