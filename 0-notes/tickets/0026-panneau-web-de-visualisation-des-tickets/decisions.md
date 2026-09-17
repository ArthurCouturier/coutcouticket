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
