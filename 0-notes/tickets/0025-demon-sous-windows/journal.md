# Journal — ticket 0025

<!-- Journal append-only, alimenté par ticket_log / « coutcouticket log ».
     Ne jamais réécrire une entrée passée. Chaque entrée finit par « Prochaine étape ». -->

## 2026-09-17 16:22 — statut : todo → in-progress

Démarrage sur la branche `feat/0025-demon-sous-windows`.

## 2026-09-17 16:53 — avancement

Portage Windows terminé et prouvé en CI (PR brouillon #1). Code : config dans %APPDATA%, jeton via getrandom, fsutil::canonicalize sans \\?\, Project::rel en /, frontmatter tolérant CRLF, hooks avec C:/…/coutcouticket.exe, claude.cmd trouvé dans le PATH, démon : --log-file + daemon.pid, tâche planifiée XML (LogonTrigger utilisateur, conhost --headless, PT0S, priorité 5, relance). CI : matrice macos-15 + windows-latest (clippy, build, tests --no-fail-fast, install.ps1 réel sous PS 5.1 dont mise à jour démon en marche). Release : job build-windows (zip + sha256, CRT statique), essai run 35235915058 vert. Pièges rencontrés : chemin du .plist avec / dans un test unitaire, liens OVERVIEW.md aux séparateurs mêlés, export schtasks sans LeastPrivilege (valeur par défaut). Critère 2 partiellement prouvé : compte CI administrateur, ouverture de session réelle non rejouable.

**Prochaine étape :** Utilisateur : sur un poste Windows avec un compte standard, suivre la « Vérification restante » des Notes de ticket.md (daemon install sans UAC, fermeture puis ouverture de session sans fenêtre et status ok, hooks, uninstall), puis cocher le critère 2 et passer en done.

## 2026-09-17 16:56 — statut : in-progress → review

Portage Windows prouvé en CI (PR brouillon #1) : build/tests macOS + Windows, hooks Git for Windows, tâche planifiée créée/lancée/retirée, zip de release en essai. À vérifier par l'utilisateur sur un poste Windows avec compte standard : daemon install sans UAC, démarrage à l'ouverture de session sans fenêtre console, puis cocher le critère 2 (procédure dans les Notes de ticket.md).
