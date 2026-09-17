---
name: ticket
description: Gestion des tickets, du journal, des décisions et de la doc technique d'un projet coutcouticket (dossier 0-notes/). À utiliser dès qu'il s'agit de créer, démarrer, reprendre, faire avancer, bloquer, relire ou clôturer un ticket, de consigner un choix technique, de savoir quoi faire ensuite, ou de mettre à jour la doc technique du projet.
---

# Skill ticket — coutcouticket

Les outils MCP `ticket_*` font tout ce qui est mécanique : identifiants, dossiers,
statuts, branches, dates, board. Toi, tu rédiges le contenu et tu décides quand
agir. Ne refais jamais à la main ce qu'un outil fait.

## Règles absolues

1. **Jamais d'édition manuelle** du frontmatter de `ticket.md`, de `BOARD.md`, ni
   des entrées passées de `journal.md` et `decisions.md`. Tout passe par les outils.
2. **Jamais de branche créée à la main** : uniquement via `ticket_start`.
3. **Toujours passer `project`** : le chemin absolu de la racine, donné au démarrage
   de session. S'il manque, le trouver en remontant jusqu'à `.coutcouticket.toml`.
4. **Un outil renvoie une erreur ? La lire et corriger la cause.** Ne jamais la
   contourner par une édition de fichier.
5. Le corps de `ticket.md` (Description, Critères d'acceptation, Notes) s'édite
   librement avec les outils d'édition habituels.

## Router l'intention

| L'utilisateur veut… | Faire |
|---|---|
| savoir où on en est, quoi faire ensuite | lire `0-notes/0-global/BOARD.md` |
| créer un ticket | procédure **Créer** |
| travailler sur un ticket | procédure **Démarrer / reprendre** |
| noter une avancée ou s'arrêter | procédure **Journaliser** |
| trancher entre plusieurs options | procédure **Décider** |
| signaler un blocage | `ticket_set_status` → `blocked`, `note` = cause et ce qui débloquerait |
| faire relire | `ticket_set_status` → `review`, `note` = quoi relire et comment tester |
| terminer | procédure **Clôturer** |
| comprendre une partie du code | `0-notes/doc/INDEX.md`, puis la page indiquée |
| savoir pourquoi un choix a été fait | `rg -n "<mot-clé>" 0-notes/tickets/*/decisions.md` |
| voir ce qu'un ticket a modifié | `ticket_files` |
| vérifier l'état des notes | `notes_validate` |

## Créer

1. Vérifier qu'un ticket équivalent n'existe pas déjà (`ticket_list`, BOARD).
2. Un ticket = un résultat livrable et vérifiable. Trop gros ? Proposer un découpage.
3. `ticket_create` avec :
   - `title` : court, orienté résultat (« Ajouter la pagination de la liste des plantes »).
     Il devient le slug du dossier et de la branche.
   - `type` : `feat` fonctionnalité, `fix` correction, `refacto` restructuration sans
     changement fonctionnel, `design` interface ou UX, `ci` build, déploiement, outillage.
   - `priority` : `p0` bloquant ou urgent, `p1` important, `p2` normal (défaut), `p3` confort.
   - `projects` : les projets de dev touchés.
   - `description` : contexte, problème, objectif. Assez pour qu'un autre développeur
     (ou toi dans trois semaines) comprenne sans la conversation.
   - `acceptance` : critères observables et testables. Pas « ça marche bien ».
4. Annoncer l'id et la branche retournés. **Ne pas démarrer** sans demande explicite.

## Démarrer / reprendre

1. `ticket_context` : lire la « prochaine étape », la branche attendue, la branche courante.
2. Lire `ticket.md`, puis `decisions.md` (ne pas remettre en cause une décision
   consignée sans le dire explicitement), puis la fin de `journal.md`.
3. Lire `0-notes/doc/INDEX.md` et les pages utiles avant d'explorer le code.
4. Si `on_ticket_branch` est faux : `ticket_start`. Il bascule ou crée la branche
   et passe le statut à `in-progress`. Arbre git sale ? Le signaler avant d'agir.
5. Reprendre à la prochaine étape notée.

## Journaliser

Appeler `ticket_log` :
- à chaque jalon significatif (une étape franchie, un problème compris) ;
- **avant de rendre la main** en fin de session, même si le travail est inachevé ;
- quand un obstacle ou une découverte change le plan.

`text` : factuel et bref. Ce qui a été fait, constaté, appris, avec les pistes
écartées quand elles ont coûté du temps. `next_step` : une action concrète et
immédiatement exécutable (« Écrire le test du cas liste vide dans PlantListTest »),
jamais « continuer ». Une entrée par jalon, pas une par fichier modifié.

## Décider

Consigner avec `ticket_decide` tout choix qu'on pourrait regretter ou redébattre :
architecture, librairie, modèle de données, compromis de performance ou de sécurité,
comportement fonctionnel ambigu. Pas les détails d'implémentation triviaux.

- `decision` : ce qui est retenu, sans ambiguïté.
- `alternatives` : les options écartées, une phrase chacune.
- `why` : contraintes, compromis acceptés, conséquences.

Si le choix revient à l'utilisateur, lui présenter les options d'abord et ne
consigner qu'après sa réponse. Une décision qui en remplace une autre la cite
(« Remplace D2 »).

## Clôturer

Dans cet ordre, sans en sauter :
1. Vérifier chaque critère d'acceptation et les cocher dans `ticket.md` (`- [x]`).
   Un critère non rempli : le signaler, ne pas clôturer.
2. Mettre à jour la doc : pour chaque partie du système modifiée, mettre à jour la
   page de `0-notes/doc/` concernée, ou la créer, et sa ligne dans `INDEX.md`.
   La doc décrit l'état actuel, pas l'historique.
3. `ticket_log` avec le résumé final et `next_step` = « Aucune » (ou le ticket de suite).
4. `ticket_set_status` → `done`, `note` = résumé en une ou deux phrases.
5. Proposer les tickets de suite éventuels, sans les créer d'office.

## Documentation technique

- `0-notes/doc/INDEX.md` : une ligne par page, colonne « Lire quand… » précise.
  C'est l'index qui rend la recherche efficace : il doit rester exact.
- Une page par domaine (auth, paiement, déploiement…), pas par ticket.
- Contenu : responsabilités, flux principaux, points d'entrée dans le code (chemins),
  invariants et pièges. Pas de copie du code.

## Si le MCP est indisponible

La CLI fait exactement la même chose :
`coutcouticket new|start|status|log|decide|list|show|files|board|validate` (voir
`coutcouticket --help`). Démon arrêté : `coutcouticket daemon status` pour
diagnostiquer, et le signaler à l'utilisateur.
