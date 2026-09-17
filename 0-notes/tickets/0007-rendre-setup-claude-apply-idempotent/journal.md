# Journal — ticket 0007

<!-- Journal append-only, alimenté par ticket_log / « coutcouticket log ».
     Ne jamais réécrire une entrée passée. Chaque entrée finit par « Prochaine étape ». -->

## 2026-09-17 15:26 — statut : todo → in-progress

Démarrage sur la branche `fix/0007-rendre-setup-claude-apply-idempotent`.

## 2026-09-17 15:29 — avancement

setup-claude --apply rendu idempotent (src/claude.rs) : lecture de « claude mcp get » (pas de sortie JSON disponible), rien si identique, sinon remove sur la portée lue puis add en portée user, avec relecture de contrôle. Messages d'erreur sans commande manuelle vouée à échouer, jetons masqués. Tests : unitaires du parseur + e2e setup_claude_idempotent avec faux claude (absent, identique, différent, portée local, échec, binaire introuvable). cargo test OK hors hooks_hors_du_path (licence Xcode, connu). README et architecture.md à jour. Non lancé : vrai setup-claude --apply (interdit ici).

**Prochaine étape :** Utilisateur : après cargo install, lancer deux fois « coutcouticket setup-claude --apply » (le second doit dire « déjà enregistré … à jour ») ; puis changer le jeton dans ~/.config/coutcouticket/daemon.toml (ou le port), relancer --apply, vérifier que « claude mcp get coutcouticket » montre la nouvelle valeur, redémarrer le démon et remettre la valeur voulue. Si OK, cocher le critère 2 et passer 0007 en done.

## 2026-09-17 15:29 — statut : in-progress → review

À relire : src/claude.rs, Cmd::SetupClaude dans src/main.rs, test e2e setup_claude_idempotent. Reste à vérifier avec le vrai claude (critère 2), procédure dans le journal.
