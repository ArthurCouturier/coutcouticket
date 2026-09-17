# Journal — ticket 0026

<!-- Journal append-only, alimenté par ticket_log / « coutcouticket log ».
     Ne jamais réécrire une entrée passée. Chaque entrée finit par « Prochaine étape ». -->

## 2026-09-17 17:31 — statut : todo → in-progress

Démarrage sur la branche `feat/0026-panneau-web-de-visualisation-des-tickets`.

## 2026-09-17 17:32 — avancement

Ticket démarré et architecture décidée avec l'utilisateur (D1 : servi par le démon, lecture seule, kanban, détail au clic). Maquette HTML/CSS/JS autonome faite sur les vraies données (43 tickets, 2 projets). Elle est hors dépôt, dans le scratchpad de la session, et soumise à l'utilisateur. La page sert aussi de base à l'implémentation : couche `api` qui bascule entre maquette et démon (`api/overview`, `api/ticket`, `api/events` en SSE), Markdown minimal échappé, thème clair/sombre, aucune dépendance externe.

**Prochaine étape :** Recueillir les retours de l'utilisateur sur la maquette. Ensuite : embarquer la page (`src/ui/index.html`), ajouter au démon les routes `/ui`, `/ui/api/overview`, `/ui/api/ticket` et `/ui/api/events` (SSE alimenté par le watcher), l'authentification par jeton de session et la commande `coutcouticket ui`.

## 2026-09-17 17:56 — avancement

Panneau implémenté (commit 23689d4), sécurité consignée en D2.

- **Démon :** routes `/ui/…` (page, `app.js`, `app.css`, `login`, `api/overview`, `api/ticket`, `api/events` en SSE) et `POST /ui-login-code` derrière le Bearer. Le watcher publie un message sur un `broadcast` après chaque lot.
- **Cœur :** `Project::detail` et `strip_html_comments` (store), `overview::detail` limité aux racines du registre.
- **CLI :** `coutcouticket ui [--print]`.
- **Page :** JS et CSS séparés pour une CSP stricte. Filtre de statut ajouté. Avertissements affichés avec leur projet. Chemin de ticket.md avec un bouton « Copier ». Détail relu à chaque événement. Message « session expirée » après un redémarrage du démon.
- **Tests :** unitaires (hôte et origine, codes à usage unique, cookie, décodage, page sans script en ligne, détail et registre) et e2e `panneau_web`. `cargo test` et `cargo clippy --all-targets -D warnings` passent.
- **Navigateur réel** (Chrome headless piloté en CDP, démon de démonstration sur un port libre avec un HOME temporaire) :
  - la connexion redirige vers `/ui/` ;
  - le cookie est invisible en JS ;
  - kanban : 4 colonnes, dépendance ouverte affichée ;
  - les filtres fonctionnent ;
  - le détail s'affiche, avec le HTML des notes échappé ;
  - après une édition de note, le titre et le journal ouvert se mettent à jour sans rechargement ;
  - démon arrêté : « reconnexion… » ; démon relancé : « session expirée » ;
  - aucune erreur console ni violation CSP.
- **Mesures** (release, macOS, 20 tickets, `ps`) :
  - binaire : 3 804 768 → 3 904 592 octets (+98 Ko) ;
  - RSS au repos : 10 144 Ko avant, 10 048 Ko après (bruit) ;
  - CPU au repos : 0:00.01 sur 20 s, dans les deux cas ;
  - avec 3 flux SSE ouverts et 20 requêtes : RSS de 10,8 Mo ;
  - flux fermés : 0 connexion restante, CPU inchangé sur 15 s ;
  - arrêt par SIGTERM immédiat avec un flux ouvert.
- **Windows :** non vérifié en local (pas de rustup ni de cible msvc). Le code n'a aucun `cfg` propre à Unix hors `open_browser` (`cmd /C start "" <url>`), et `panneau_web` n'utilise rien de propre à Unix.
- **Non exercé en test :** l'ouverture réelle du navigateur par `open`, pour ne pas ouvrir d'onglet chez l'utilisateur. `--print` couvre le reste du chemin.

**Prochaine étape :** Pousser la branche et vérifier que la CI windows-latest passe (`panneau_web` compris), puis cocher le critère Windows. Côté utilisateur : `cargo install --path .`, `coutcouticket daemon install`, puis `coutcouticket ui`. Si possible, essai sous Windows (ouverture par `cmd /C start`).

## 2026-09-17 17:58 — avancement

**Doc mise à jour :** section « Panneau web » d'architecture.md (et sécurité de `/mcp` distinguée, tests), INDEX.md, README (section « Panneau web », commande `ui`) et skill (routage « voir les tickets dans un navigateur »).

**Relecture de clôture (subagent relecteur-cloture) :**
- Les cinq premiers critères sont prouvés. Le critère Windows attend la CI `windows-latest`.
- Le relecteur demandait de commiter les notes et la doc : c'est fait dans ce lot.
- Il a aussi relevé que le compteur du panneau ignorait le filtre de statut : corrigé (commit 5d04fb7). `cargo test` et clippy passent.

**Statut :** passage en review.

**Prochaine étape :** Pousser la branche, vérifier la CI windows-latest (`panneau_web` compris), puis cocher le critère Windows, consigner le run CI au journal et passer en done.

## 2026-09-17 17:58 — statut : in-progress → review

Implémentation terminée et prouvée sous macOS ; le critère Windows attend la CI windows-latest.
