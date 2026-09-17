<!-- coutcouticket:start — bloc géré par « coutcouticket init », ne pas éditer -->
## Tickets et notes (coutcouticket)

- Ce projet utilise coutcouticket : tickets, doc et vue globale dans `{{notes_dir}}/`.
- Avant d'explorer le code, lire `{{notes_dir}}/doc/INDEX.md`.
- Toute action sur un ticket passe par le skill `ticket` et les outils MCP `ticket_*`
  (ou la CLI `coutcouticket`). Ne jamais éditer un frontmatter ni `BOARD.md` à la main.
- Branches strictement au format `<type>/<id>-<slug>` (ex. `feat/0013-add-thing`),
  créées uniquement via `ticket_start`.
- Fonctionnement complet : `{{notes_dir}}/0-global/README.md`.
<!-- coutcouticket:end -->
