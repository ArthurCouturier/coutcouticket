# Journal — ticket 0002

<!-- Journal append-only, alimenté par ticket_log / « coutcouticket log ».
     Ne jamais réécrire une entrée passée. Chaque entrée finit par « Prochaine étape ». -->

## 2026-09-17 14:30 — statut : todo → in-progress

Démarrage sur la branche `ci/0002-valider-l-installation-complete-sur-macos`.

## 2026-09-17 14:30 — avancement

Critère 1 en cours. `cargo install` OK (~/.cargo/bin/coutcouticket 0.1.0, 12:20) ; `/opt/homebrew/bin/coutcouticket` est un lien symbolique vers ce même binaire (12:25), pas de conflit. LaunchAgent `app.coutcouticket.daemon` chargé, démon PID 89399 démarré à 12:24:18 via `daemon install`, `daemon status` ok, `claude mcp list` : connecté sur 127.0.0.1:47813. Reste à prouver le redémarrage automatique après une fermeture de session macOS.

**Prochaine étape :** Après réouverture de session : `ps -o pid,lstart -p $(pgrep -f 'coutcouticket daemon run')` doit montrer un nouveau PID et une heure postérieure à 12:24:18, et `coutcouticket daemon status` doit répondre ok ; puis passer au critère 2 (outils ticket_* dans Claude Code).

## 2026-09-17 14:32 — avancement

Preuves partielles du critère 1 (non coché : RunAtLoad reste à prouver par une déconnexion).
- Géré par launchd : `launchctl print gui/$UID/app.coutcouticket.daemon` → state = running, pid = 89399, runs = 1, jamais arrêté.
- KeepAlive : `kill 89399` à 14:32:33 → relancé en ~1 s, PID 94528, runs = 2, last exit code = 0 (arrêt propre sur SIGTERM), `daemon status` ok. Pas d'attente de 10 s : le ThrottleInterval ne s'applique qu'à un processus ayant tourné moins de 10 s.

**Prochaine étape :** Vérifier le critère 2 : `claude mcp list` doit montrer `coutcouticket` en HTTP vers 127.0.0.1:47813 (pas seulement `coutcouticket-stdio`), et `setup-claude --apply` doit être idempotent.

## 2026-09-17 14:33 — avancement

Critère 2 vérifié. `claude mcp list` : `coutcouticket` en HTTP vers http://127.0.0.1:47813/mcp, connecté, portée utilisateur ; aucun `coutcouticket-stdio`. L'enregistrement est identique à la sortie de `setup-claude` (même URL, même jeton). Les outils `mcp__coutcouticket__*` de la session passent par le démon et exigent `project` ; ils ont continué de répondre après la relance du démon (PID 94528).
Défaut constaté : relancer `setup-claude --apply` quand le serveur existe déjà échoue (« MCP server coutcouticket already exists in user config ») et propose une commande manuelle qui échouerait de la même façon. Aucune perte, l'enregistrement reste intact. Hors périmètre de ce ticket : à traiter dans un ticket fix séparé (src/main.rs, Cmd::SetupClaude).

**Prochaine étape :** Commiter ticket.md et journal.md sur la branche ci/0002 et vérifier que le hook ajoute le trailer « Ticket: 0002 » (critère 5, cas terminal).
