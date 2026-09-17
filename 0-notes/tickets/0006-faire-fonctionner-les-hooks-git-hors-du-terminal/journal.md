# Journal — ticket 0006

<!-- Journal append-only, alimenté par ticket_log / « coutcouticket log ».
     Ne jamais réécrire une entrée passée. Chaque entrée finit par « Prochaine étape ». -->

## 2026-09-17 14:45 — statut : todo → in-progress

Démarrage sur la branche `fix/0006-faire-fonctionner-les-hooks-git-hors-du-terminal`.

## 2026-09-17 14:48 — avancement

Correctif livré (commit 81f8e7f), voir la décision D1.
- `src/init.rs` : `hook_script` écrit le chemin absolu du binaire (`current_binary`, cité par `sh_quote`), avec repli sur `command -v`, sinon une erreur qui indique de relancer `init`. Avertissement dans le rapport d'`init` si le chemin est inconnu. La réécriture des hooks marqués existait déjà et couvre la migration.
- Écart assumé avec le critère 2 : le message d'erreur a été reformulé (il commence toujours par « coutcouticket introuvable ») pour dire de relancer `init`.
- Tests : 2 unitaires (`sh_quote` exécuté par sh avec apostrophe, espace, `$` et accent grave ; gabarit avec ou sans chemin) et l'e2e `hooks_hors_du_path` (ancien gabarit migré ; commit avec le PATH de launchd accepté et trailer présent ; binaire noté supprimé → repli sur le PATH, puis refus avec le PATH de launchd ; hook tiers intact). `cargo test` : 12 + 4 au vert.
- Réel : binaire réinstallé avec `cargo install`, `init` relancé ici (les 2 hooks mis à jour), commande `env -i` du 0002 acceptée.
- Doc : section « Hooks git » dans architecture.md, ligne d'INDEX.md, paragraphe PATH du README.
- Le démon lancé par launchd tourne encore sur l'ancien binaire (inode remplacé). Sans effet, car le code du démon n'a pas changé ; la déconnexion prévue au critère 1 du 0002 le relancera.

**Prochaine étape :** Aucune

## 2026-09-17 14:48 — statut : in-progress → done

Les hooks git appellent le binaire par le chemin noté lors d'init, avec repli sur le PATH : les commits passent depuis un client graphique (PATH de launchd). Couvert par l'e2e hooks_hors_du_path.
