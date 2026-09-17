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

## 2026-09-17 17:12 — avancement

Le 2026-09-17, l'utilisateur indique que la vérification sur un poste Windows est validée (procédure des Notes : `daemon install` sans UAC, démarrage à l'ouverture de session sans fenêtre, hooks, `uninstall`). Critère 2 coché. Le binaire Windows sera publié par la release v0.2.0.

**Prochaine étape :** Aucune

## 2026-09-17 17:19 — avancement

Release v0.2.0 publiée (run 35238825629, vert) : https://github.com/ArthurCouturier/coutcouticket/releases/tag/v0.2.0. Archives : `coutcouticket-0.2.0-x86_64-pc-windows-msvc.zip` et `.zip.sha256`, les deux archives macOS, et `SHA256SUMS`, qui liste le zip Windows. La CI de main (run 35238823065) est verte sous macOS et Windows. `install.sh` a installé la 0.2.0 depuis la release dans un dossier temporaire.
Preuves du ticket :
- run CI 35236987954 sur main ;
- essai de release 35235915058 ;
- release 35238825629 ;
- validation manuelle de l'utilisateur sur un poste Windows. Le type de compte (standard ou administrateur) n'a pas été précisé.

**Prochaine étape :** Aucune

## 2026-09-17 17:19 — statut : review → done

Démon Windows livré : tâche planifiée à l'ouverture de session, hooks Git for Windows, install.ps1, CI Windows. Binaire publié dans la release v0.2.0 et validé par l'utilisateur sur un poste Windows. Relecture de clôture faite : ses corrections (release, notes) sont appliquées.
