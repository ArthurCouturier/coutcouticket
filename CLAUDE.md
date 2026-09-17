# CLAUDE.md

## Projet

coutcouticket : système de tickets Markdown pour Claude Code. Binaire Rust unique
(CLI, serveur MCP, démon, hooks) + plugin Claude Code (`plugin/`).

- Architecture, invariants et pièges : `0-notes/doc/architecture.md` (à lire avant de modifier le code).
- Toute logique métier va dans `src/store.rs` ; les façades (CLI, MCP, hooks) ne font qu'appeler le cœur.
- Messages utilisateur et d'erreur en français, explicites, avec la correction à appliquer.
- Avant de rendre la main : `cargo test` doit passer (unitaires + `tests/e2e.rs`).
- Un nouvel outil MCP = une méthode dans `src/mcp.rs` + le test stdio + la table du README + le skill.

<!-- coutcouticket:start — bloc géré par « coutcouticket init », ne pas éditer -->
## Tickets et notes (coutcouticket)

- Ce projet utilise coutcouticket : tickets, doc et vue globale dans `0-notes/`.
- Avant d'explorer le code, lire `0-notes/doc/INDEX.md`.
- Toute action sur un ticket passe par le skill `ticket` et les outils MCP `ticket_*`
  (ou la CLI `coutcouticket`). Ne jamais éditer un frontmatter ni `BOARD.md` à la main.
- Branches strictement au format `<type>/<id>-<slug>` (ex. `feat/0013-add-thing`),
  créées uniquement via `ticket_start`.
- Fonctionnement complet : `0-notes/0-global/README.md`.
<!-- coutcouticket:end -->
