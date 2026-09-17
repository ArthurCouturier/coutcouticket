# Journal — ticket 0018

<!-- Journal append-only, alimenté par ticket_log / « coutcouticket log ».
     Ne jamais réécrire une entrée passée. Chaque entrée finit par « Prochaine étape ». -->

## 2026-09-17 15:56 — statut : todo → in-progress

Démarrage sur la branche `fix/0018-ne-pas-attribuer-au-ticket-de-la-branche-un`.

## 2026-09-17 16:00 — avancement

Implémenté selon D1 et D2 : Project::path_owner et Project::commit_tickets (store.rs), git::staged_files, git::show_prefix, git::add_trailers (ajouts en fin de git.rs, add_trailer remplacé), hook prepare-commit-msg branché dessus, filtre dans Project::files (commités et non commités). Tests : unitaires rattachement_des_chemins et trailers_selon_le_contenu_du_commit, e2e trailers_des_commits_de_notes (notes d'autres tickets, commit -a, doc seule, notes mêlées, trailer déjà présent, commit mal attribué, files 1/2/3) ; le test e2e échoue avec l'ancien comportement. cargo test complet vert. Vérifié sur le dépôt réel : files 0008 ne liste plus les 7 fichiers de notes d'autres tickets venus de 21e60b7. Doc : architecture.md (Hooks git, Tests), 0-global/README.md et son gabarit, README.md, description MCP de ticket_files.

**Prochaine étape :** Aucune

## 2026-09-17 16:01 — statut : in-progress → done

Commits de notes d'autres tickets attribués aux tickets touchés (D1) ; files exclut notes étrangères et BOARD.md (D2). Relecture de clôture OK.
