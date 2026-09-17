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

## 2026-09-17 14:37 — avancement

Critère 3 : hook testé à la main, reste l'observation dans une vraie session (non coché).
- Pas de contexte vu au début de la session précédente : normal, le plugin a été installé après son démarrage.
- La commande exacte de `plugin/hooks/hooks.json`, alimentée avec le JSON SessionStart (cwd = racine), renvoie un `additionalContext` valide (code 0) : projet, paramètre `project`, compteurs, branche → ticket 0002, prochaine étape.
- cwd hors projet (/tmp) : aucune sortie, code 0. cwd = src/ : la racine est bien retrouvée.
- La copie installée (~/.claude/plugins/cache/coutcouticket/coutcouticket/0.1.0/hooks/hooks.json) est identique à la source.
- À noter : `|| true` masque toute erreur du hook ; un échec ne se verrait que par l'absence de contexte.
Critère 5, client graphique : risque signalé. Une app lancée par le Dock hérite du PATH de launchd (/usr/bin:/bin:/usr/sbin:/sbin), sans ~/.cargo/bin ni /opt/homebrew/bin. Les hooks git échoueraient alors avec « coutcouticket introuvable dans le PATH ». Pas encore observé : à confirmer avec le client de l'utilisateur. Ticket fix à créer à la clôture (par ex. chemin absolu du binaire écrit dans les hooks par `init`).

**Prochaine étape :** Au redémarrage de Claude Code dans ce dossier : vérifier que le contexte « [coutcouticket] Projet : … » est injecté avec le ticket 0002 et cette prochaine étape, puis cocher le critère 3 et passer au critère 4 (BOARD.md régénéré en moins de 2 s après une modification manuelle).

## 2026-09-17 14:39 — avancement

Critère 3 vérifié et coché. Au démarrage d'une nouvelle session Claude Code dans ce dossier, le hook SessionStart a injecté le contexte « [coutcouticket] Projet : … » : chemin du projet, paramètre `project`, compteurs (1 en cours, 1 en revue, 0 bloqué, 3 à faire), branche courante → ticket 0002 (in-progress) et la prochaine étape notée.

**Prochaine étape :** Critère 4 : l'utilisateur passe à la main le statut du ticket 0005 de `todo` à `blocked` dans son éditeur. Comparer les mtimes de ticket.md et BOARD.md (écart < 2 s), vérifier que 0005 apparaît dans « Bloqué », puis remettre `todo` avec ticket_set_status.

## 2026-09-17 14:41 — avancement

Critère 4 vérifié et coché. L'utilisateur a changé à la main, dans son éditeur, le statut du ticket 0005 (`todo` → `blocked`).
- mtime de ticket.md : 14:40:37,726 ; mtime de BOARD.md : 14:40:38,099. BOARD.md a donc été régénéré **0,37 s** après l'enregistrement, par le démon et via FSEvents.
- Contenu correct : 0005 figure dans « Bloqué (1) » et l'en-tête affiche « 1 bloqué · 2 à faire ».
- Statut remis à `todo` avec `ticket_set_status`, ce qui a ajouté une entrée explicative au journal du 0005. BOARD.md et le ticket.md du 0005 sont de nouveau identiques à HEAD, et `notes_validate` ne signale aucun problème.

**Prochaine étape :** Critère 5 : faire un commit sur la branche ci/0002 depuis le terminal et vérifier que le hook ajoute le trailer « Ticket: 0002 ». Refaire ensuite l'essai depuis le client git graphique habituel de l'utilisateur, pour voir si le PATH de launchd fait échouer le hook.

## 2026-09-17 14:43 — avancement

Critère 5 : moitié terminal OK, moitié client graphique en échec. Critère **non coché**.
- Terminal : le commit 1f199aa a été accepté et le hook prepare-commit-msg a ajouté le trailer « Ticket: 0002 ».
- Client graphique, simulé comme demandé par l'utilisateur avec le PATH de launchd : `env -i HOME="$HOME" PATH=/usr/bin:/bin:/usr/sbin:/sbin git commit --allow-empty -m "test PATH graphique"` est refusé par le hook pre-commit, code 1, message « coutcouticket introuvable dans le PATH. Installer le binaire (voir README) ou contourner ponctuellement avec --no-verify. ». Aucun commit créé (HEAD reste 1f199aa), donc rien à annuler.
- Cause : les hooks générés par `init` cherchent `coutcouticket` via `command -v`. Le PATH de launchd ne contient ni ~/.cargo/bin ni /opt/homebrew/bin. Un client git lancé depuis le Dock bloquerait donc tous les commits du projet.
- À corriger dans le ticket fix prévu (par exemple écrire dans les hooks le chemin absolu du binaire, avec repli sur le PATH). Ensuite, refaire cette même commande pour cocher le critère.

**Prochaine étape :** Critère 6 : obtenir de l'utilisateur le nom du projet existant. Dans ce projet, vérifier que `git status` est propre, relever CLAUDE.md et les hooks existants (.git/hooks, core.hooksPath, husky, lefthook), lancer `coutcouticket init`, puis montrer `git status` et `git diff` et vérifier que CLAUDE.md et les hooks sont intacts.

## 2026-09-17 14:44 — avancement

Bilan demandé par l'utilisateur. Ticket **non clôturé** : 3 critères sur 6 restent ouverts.
- Critère 1 (démon après redémarrage de session) : non coché. KeepAlive est prouvé ; il manque la preuve de RunAtLoad après une vraie déconnexion ou un redémarrage macOS.
- Critère 2 (setup-claude --apply) : coché maintenant, sur la base de la vérification de 14:33. Le défaut de ré-exécution est suivi dans le ticket 0007.
- Critères 3 et 4 : cochés (voir les entrées précédentes).
- Critère 5 (hooks git) : non coché. Le terminal fonctionne, mais le hook échoue avec le PATH de launchd. Correction suivie dans le ticket 0006.
- Critère 6 (init sur un vrai projet) : non fait. Le nom du projet cible n'a pas encore été donné.
Tickets fix créés : 0006 (hooks git hors terminal, p1) et 0007 (setup-claude --apply idempotent, p2).
Le 0001 reste en review : son journal subordonne le passage en done à la validation du 0002.

**Prochaine étape :** Obtenir de l'utilisateur le projet cible du critère 6 et y lancer `coutcouticket init` (git status propre d'abord, puis relever CLAUDE.md et les hooks existants et vérifier qu'ils sont intacts). Après une fermeture de session macOS, prouver le critère 1 avec `ps -o pid,lstart -p $(pgrep -f 'coutcouticket daemon run')` et `coutcouticket daemon status`.

## 2026-09-17 14:48 — avancement

Critère 5 revérifié après le correctif 0006 (commit 81f8e7f, binaire réinstallé, `init` relancé ici) et coché. `env -i HOME="$HOME" PATH=/usr/bin:/bin:/usr/sbin:/sbin git commit --allow-empty -m "test PATH graphique"` : code 0, trailer « Ticket: 0006 » ajouté, car le commit a été fait sur la branche du 0006. Le commit vide de test (948f7d9) a été retiré ensuite avec `git reset --soft HEAD~1`, après avoir vérifié qu'il était vide.

**Prochaine étape :** Critère 6 : obtenir de l'utilisateur le chemin exact du projet existant, puis y lancer `coutcouticket init` (git status propre d'abord ; relever CLAUDE.md et les hooks existants, husky ou lefthook compris, et vérifier qu'ils sont intacts). Critère 1 en dernier, après la déconnexion macOS faite par l'utilisateur.
