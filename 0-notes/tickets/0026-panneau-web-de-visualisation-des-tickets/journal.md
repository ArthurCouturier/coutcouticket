# Journal — ticket 0026

<!-- Journal append-only, alimenté par ticket_log / « coutcouticket log ».
     Ne jamais réécrire une entrée passée. Chaque entrée finit par « Prochaine étape ». -->

## 2026-09-17 17:31 — statut : todo → in-progress

Démarrage sur la branche `feat/0026-panneau-web-de-visualisation-des-tickets`.

## 2026-09-17 17:32 — avancement

Ticket démarré et architecture décidée avec l'utilisateur (D1 : servi par le démon, lecture seule, kanban, détail au clic). Maquette HTML/CSS/JS autonome faite sur les vraies données (43 tickets, 2 projets). Elle est hors dépôt, dans le scratchpad de la session, et soumise à l'utilisateur. La page sert aussi de base à l'implémentation : couche `api` qui bascule entre maquette et démon (`api/overview`, `api/ticket`, `api/events` en SSE), Markdown minimal échappé, thème clair/sombre, aucune dépendance externe.

**Prochaine étape :** Recueillir les retours de l'utilisateur sur la maquette. Ensuite : embarquer la page (`src/ui/index.html`), ajouter au démon les routes `/ui`, `/ui/api/overview`, `/ui/api/ticket` et `/ui/api/events` (SSE alimenté par le watcher), l'authentification par jeton de session et la commande `coutcouticket ui`.
