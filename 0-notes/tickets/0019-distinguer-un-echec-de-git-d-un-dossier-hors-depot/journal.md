# Journal — ticket 0019

<!-- Journal append-only, alimenté par ticket_log / « coutcouticket log ».
     Ne jamais réécrire une entrée passée. Chaque entrée finit par « Prochaine étape ». -->

## 2026-09-17 15:56 — statut : todo → in-progress

Démarrage sur la branche `fix/0019-distinguer-un-echec-de-git-d-un-dossier-hors-depot`.

## 2026-09-17 16:00 — avancement

git.rs distingue « pas un dépôt » (rev-parse 128 + « not a git repository », LC_ALL=C) d'un git en échec : message avec commande, code, stderr et correction connue (licence Xcode : sudo xcodebuild -license ou xcode-select -s CommandLineTools ; outils absents : xcode-select --install). is_repo et branch_exists renvoient Result. files/start propagent l'erreur ; context expose git_error (show, ticket_context) ; hook SessionStart ajoute une ligne « git en échec » ; init avertit et saute hooks et .gitignore. Binaire injectable COUTCOUTICKET_GIT_BIN. Tests : unitaires git.rs + e2e git_en_echec (faux git code 69, panne 128, binaire introuvable, vrai dossier hors dépôt, MCP stdio). cargo test complet vert. Doc architecture.md (section Appels git), INDEX.md, README Diagnostic.

**Prochaine étape :** Aucune

## 2026-09-17 16:01 — statut : in-progress → done

git.rs distingue « pas un dépôt » d'un git en échec (commande, code, stderr, correction Xcode) ; ticket_context expose git_error. Relecture de clôture OK ; skill complété pour git_error.
