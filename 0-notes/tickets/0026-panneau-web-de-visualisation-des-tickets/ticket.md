---
id: "0026"
title: "Panneau web de visualisation des tickets"
type: feat
status: done
priority: p2
projects: [daemon, core]
created: 2026-09-17
updated: 2026-09-17
---

## Description

Demandé par l'utilisateur le 2026-09-17 : un petit panneau web pour voir les tickets (tous projets), sans trahir le principe de l'app, qui est de faire tourner le moins de choses possible, le plus efficacement possible. Aucun service exposé : tout reste local, ou ne s'ouvre qu'à la demande.

**Architecture à trancher avec l'utilisateur avant de démarrer.** Options étudiées :

1. **Servi par le démon existant** (`GET /ui` sur 127.0.0.1:47813) : aucun processus en plus, puisque le démon tourne déjà. Page HTML/JS embarquée dans le binaire (`include_str!`, quelques Ko), données lues via un point d'accès JSON qui réutilise `overview`. La commande `coutcouticket ui` ouvre le navigateur. Rafraîchissement par événements (SSE), déclenché par le watcher, sans sondage. Point délicat : l'authentification (le démon exige un jeton Bearer et refuse l'en-tête `Origin`) ; piste : jeton de session à usage unique passé par `coutcouticket ui`.
2. **Page statique générée** (`OVERVIEW.html` à côté d'`OVERVIEW.md`, régénérée par le démon) : aucun serveur, ouverte en `file://`, filtres en JS côté client, lecture seule. C'est le plus sobre, mais le rafraîchissement demande de recharger la page, et aucune action n'est possible.
3. **Serveur éphémère** (`coutcouticket ui` lance un serveur sur un port libre, qui s'arrête à la fermeture de l'onglet grâce à un battement de cœur) : utile seulement si le démon n'est pas installé.
4. **Application de bureau** (Tauri, webview système) : on l'ouvre et on la ferme comme un logiciel, mais elle demande une autre chaîne de build, pèse environ 10 Mo et impose une distribution par plateforme. C'est à l'opposé de la sobriété visée.

Recommandation : option 1, avec une v1 en lecture seule. L'option 2 peut servir de repli quand le démon est arrêté. Les actions (changer un statut, journaliser) viendraient dans un second temps, par les mêmes fonctions du cœur que le MCP.

## Critères d'acceptation

- [x] Architecture choisie avec l'utilisateur et consignée (decisions.md)
- [x] `coutcouticket ui` ouvre le panneau : tickets ouverts de tous les projets, filtres par projet, statut et priorité, dépendances ouvertes, lien vers le ticket.md
- [x] Mise à jour sans rechargement manuel quand une note change, sans sondage périodique
- [x] Aucun processus supplémentaire au repos, et aucune dépendance JS externe ni CDN : page embarquée dans le binaire
- [x] Accès limité à 127.0.0.1 et protégé (jeton, protection contre le rebinding DNS et le CSRF), avec tests
- [x] Fonctionne sous macOS et Windows

## Notes

- Preuves : `tests/e2e.rs::panneau_web` ; tests unitaires `ui::tests`, `overview::tests::detail_limite_aux_projets_enregistres`, `store::tests::detail_*` ; vérification dans Chrome headless (journal du 2026-09-17).
- Lien vers le ticket.md : chemin absolu affiché avec un bouton « Copier », car les navigateurs bloquent `file://` depuis une page http (D2).
- Windows : confirmé par la CI windows-latest (runs 35243734552 et 35244007995, `panneau_web` vert). L'ouverture réelle du navigateur par `cmd /C start` n'a pas été essayée à la main.
- macOS : validé par l'utilisateur le 2026-09-17 (`cargo install`, `daemon install`, puis `coutcouticket ui`).

