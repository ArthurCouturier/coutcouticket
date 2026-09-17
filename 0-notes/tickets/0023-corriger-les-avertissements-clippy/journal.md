# Journal — ticket 0023

<!-- Journal append-only, alimenté par ticket_log / « coutcouticket log ».
     Ne jamais réécrire une entrée passée. Chaque entrée finit par « Prochaine étape ». -->

## 2026-09-17 16:18 — statut : todo → in-progress

Démarrage sur la branche `refacto/0023-corriger-les-avertissements-clippy`.

## 2026-09-17 16:19 — avancement

Les 6 avertissements clippy corrigés sans changement de comportement ni #[allow] : 2 sort_by → sort_by_key (Reverse pour l'ordre décroissant, tri stable identique) dans board.rs ; 4 if imbriqués fusionnés en let chains (config.rs, fsutil.rs, hooks.rs, store.rs). cargo clippy --all-targets --locked -- -D warnings passe, cargo test passe (35 unitaires + 10 e2e). CI : composant clippy ajouté et étape « Clippy » avant les tests (D1). Doc : distribution.md et README (section Développement). À noter : le profil rustup minimal n'inclut pas clippy, d'où --component clippy.

**Prochaine étape :** Aucune

## 2026-09-17 16:20 — statut : in-progress → done

Avertissements clippy corrigés sans #[allow] ; la CI lance clippy -D warnings avant les tests.
