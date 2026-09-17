# Décisions — ticket 0026

<!-- Une entrée par décision, via ticket_decide / « coutcouticket decide ».
     Une décision remplacée n'est jamais supprimée : on ajoute une nouvelle entrée qui la cite. -->

## D1 — Panneau servi par le démon, lecture seule, kanban avec détail au clic (2026-09-17)

**Décision :** Choix de l'utilisateur, le 2026-09-17 :
- **Hébergement :** le panneau est servi par le démon existant (`GET /ui` sur 127.0.0.1:47813), avec une page HTML/CSS/JS embarquée dans le binaire, sans framework ni CDN. `coutcouticket ui` ouvre le navigateur.
- **Portée de la v1 :** lecture seule.
- **Présentation :** kanban tous projets (colonnes : en cours, en revue, bloqué, à faire), filtres par projet et priorité, dépendances ouvertes affichées. Un clic sur un ticket ouvre son détail : description, critères, décisions, journal.
- **Rafraîchissement :** poussé par le watcher (SSE), sans sondage.

**Alternatives écartées :** - Page statique `OVERVIEW.html` : écartée, car sans mise à jour en direct ni détail riche. Elle reste un repli possible si le démon est arrêté.
- Serveur éphémère lancé par `ui` : inutile, le démon tourne déjà.
- Application de bureau (Tauri) : écartée pour sa chaîne de build, son poids et sa distribution par OS, contraires à la sobriété du projet.
- Actions depuis le panneau (statut, journal) : reportées à un ticket ultérieur.

**Pourquoi :** Rien ne tourne en plus au repos : le démon est déjà lancé, déjà limité à 127.0.0.1 et déjà protégé par ses hôtes autorisés et son refus des requêtes cross-origin. Le panneau ne coûte que pendant qu'un onglet est ouvert. La lecture seule évite de créer une nouvelle surface d'écriture, et donc le CSRF sur des actions, en v1. Les données réutilisent `overview` (0011) et les fonctions de contexte du cœur.

## D2 — Authentification et en-têtes de sécurité du panneau (2026-09-17)

**Décision :** - **Connexion :** `coutcouticket ui` demande au démon, avec le jeton Bearer, un code de connexion (POST `/ui-login-code`, refusé si un en-tête Origin est présent). Le code est aléatoire, à usage unique et valable 2 min, avec 16 codes au plus en attente. `/ui/login?code=…` consomme le code et pose le cookie `cct_ui` (`HttpOnly; SameSite=Strict; Path=/ui`). Le secret du cookie est tiré au lancement du démon : redémarrer le démon ferme les sessions, et la page affiche alors « session expirée : relancer coutcouticket ui ». Les comparaisons se font en temps constant.
- **Après la connexion :** une page intermédiaire redirige vers `./` par `meta refresh`, plutôt qu'un 302. La navigation part ainsi de la même origine, et tous les navigateurs envoient le cookie Strict. Vérifié dans Chrome headless.
- **Garde commune à toutes les routes `/ui` :**
  - Host : 127.0.0.1 ou localhost, sur le port du démon (protection contre le DNS rebinding).
  - Origin, si présent : `http://127.0.0.1:<port>` ou `http://localhost:<port>`.
  - `Sec-Fetch-Site`, si présent : `same-origin` ou `none`. `same-site` est refusé, car SameSite ignore le port : sinon, une page servie sur un autre port local recevrait le cookie.
  - Sans session : 401, en page HTML, ou en JSON pour l'API.
  - `/mcp` n'est pas modifié.
- **En-têtes :**
  - `Content-Security-Policy: default-src 'none'; script-src 'self'; style-src 'self'; connect-src 'self'; img-src data:; base-uri 'none'; form-action 'none'; frame-ancestors 'none'` ;
  - `X-Content-Type-Options: nosniff`, `Referrer-Policy: no-referrer`, `X-Frame-Options: DENY` ;
  - `Cache-Control: no-store` sur toutes les réponses ;
  - COOP et CORP à `same-origin`.
- **Fichiers séparés :** le JS et le CSS sont servis à part (`app.js`, `app.css`). Ils sont publics, car ils ne contiennent aucune donnée. La CSP n'autorise aucun script ni style en ligne. Le texte des notes est échappé avant le rendu Markdown.
- **API :** `api/ticket` exige que `project` soit identique, caractère pour caractère, à une racine du registre. Sinon, la réponse est un 404, sans aucun accès disque. L'id est validé par `TicketId` (400 s'il est invalide). La racine ouverte doit être exactement la racine enregistrée : `find_root` ne remonte jamais vers un projet parent.
- **SSE :** le watcher alimente un `tokio::sync::broadcast` après chaque lot qui régénère OVERVIEW.md. Le flux (`unfold`) vit avec la connexion et ne lance aucune tâche. Un commentaire de maintien part toutes les 30 s. Le flux se ferme à l'arrêt du démon (CancellationToken) : sans cela, l'arrêt progressif l'attendrait.
- **Lien vers ticket.md :** les navigateurs bloquent les liens `file://` depuis une page http. Le détail affiche donc le chemin absolu, avec un bouton « Copier ».

**Alternatives écartées :** - **Jeton du démon dans l'URL ou dans le cookie :** le jeton resterait dans l'historique, et le cookie donnerait accès à /mcp.
- **Jeton de panneau dérivé du jeton du démon :** sans HMAC, faute de dépendance crypto, la dérivation serait faible, et le jeton serait réutilisable depuis l'historique.
- **Fragment `#t=` lu par le JS :** le jeton serait exposé au script et resterait valable.
- **Redirection 302 après la connexion :** l'envoi du cookie Strict dépendrait du navigateur.
- **CSP avec `'unsafe-inline'`, ou avec l'empreinte du script :** des fichiers séparés sont plus simples et plus stricts.
- **Route de lecture brute de ticket.md :** ce serait une surface de plus, pour un contenu que l'onglet Ticket affiche déjà.

**Pourquoi :** Le navigateur ne peut pas porter le Bearer. Un code éphémère, obtenu avec le Bearer, transmet l'autorisation sans jamais exposer le jeton du démon. La lecture seule, la CSP stricte, les contrôles Host, Origin et Sec-Fetch-Site et le cookie HttpOnly et Strict couvrent le DNS rebinding, le CSRF, le XSS et le clickjacking, sans aucune dépendance supplémentaire.
