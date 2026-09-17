# Journal — ticket 0011

<!-- Journal append-only, alimenté par ticket_log / « coutcouticket log ».
     Ne jamais réécrire une entrée passée. Chaque entrée finit par « Prochaine étape ». -->

## 2026-09-17 16:17 — avancement

Proposition validée par l'utilisateur le 2026-09-17, à réaliser après 0023 et 0024. Le titre garde son préfixe « À valider » : aucun outil ne permet encore de renommer un ticket.

**Prochaine étape :** Démarrer le ticket : `overview` en CLI et outil MCP `tickets_overview` sur le registre des projets.

## 2026-09-17 16:22 — statut : todo → in-progress

Démarrage sur la branche `feat/0011-a-valider-vue-des-tickets-de-tous-les-projets`.

## 2026-09-17 16:26 — avancement

Livré : module src/overview.rs (agrégation des tickets ouverts du registre, tri statut/priorité/projet/id, avertissements pour projets introuvables, config invalide, tickets/ absent ou notes avec problèmes), CLI `overview [--status] [--priority] [--json]`, outil MCP `tickets_overview` sans `project` (stdio et démon), OVERVIEW.md généré par le démon (déterministe, écrit si changé, régénéré par le watcher). Bug préexistant corrigé : le watcher ne voyait pas les changements du registre quand le dossier de config est derrière un lien symbolique (chemins FSEvents canoniques). Tests : unitaires overview + e2e overview_plusieurs_projets et daemon_http_auth_et_watcher étendu ; cargo test et cargo clippy --all-targets -D warnings passent. Doc : architecture.md, INDEX.md, README, skill (table de routage).

**Prochaine étape :** Aucune

## 2026-09-17 16:28 — statut : in-progress → done

Vue globale livrée : overview (CLI), tickets_overview (MCP, sans project) et OVERVIEW.md généré par le démon ; projets illisibles signalés sans échec. Relecture de clôture OK.
