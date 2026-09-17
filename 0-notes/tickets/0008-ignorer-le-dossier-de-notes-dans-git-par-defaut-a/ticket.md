---
id: "0008"
title: "Ignorer le dossier de notes dans git par défaut à l'init"
type: feat
status: todo
priority: p2
projects: [core]
created: 2026-09-17
updated: 2026-09-17
---

## Description

Demande de l'utilisateur : `coutcouticket init` ajoute par défaut le dossier de notes (`notes_dir`, `0-notes` par défaut) et tout son contenu au `.gitignore` du projet.

Aujourd'hui, `init` ne touche pas au `.gitignore`, et les notes sont versionnées : 26 fichiers suivis dans ce dépôt.

Points à traiter :
- Ligne ajoutée : `/<notes_dir>/`, ancrée à la racine. Créer `.gitignore` s'il n'existe pas ; sinon ajouter la ligne à la fin, sans toucher au reste ; ne rien faire si une règle équivalente existe déjà (idempotence, invariant « init ne réécrit jamais un contenu utilisateur »).
- Opt-out sur le modèle des options existantes (`--no-git-hooks`, `--no-claude-md`) : `--no-gitignore`.
- Si des fichiers du dossier sont déjà suivis par git, le `.gitignore` ne les retire pas : l'indiquer dans le rapport d'`init` avec la commande `git rm -r --cached <notes_dir>`, sans l'exécuter.
- Conséquences à documenter : notes non partagées via git et non visibles dans les PR, et `BOARD.md` n'apparaît plus dans les diffs. Les trailers `Ticket:` et `ticket_files` restent inchangés (ils portent sur les commits de code).
- Décider (ticket_decide) du comportement pour les projets qui versionnent déjà leurs notes, dont coutcouticket lui-même : relancer `init` ne doit pas les faire ignorer à leur insu.

## Critères d'acceptation

- [ ] `init` sur un projet sans .gitignore crée un .gitignore contenant `/0-notes/`, et `git status` n'affiche plus rien sous 0-notes
- [ ] `init` sur un .gitignore existant ajoute seulement la ligne, à la fin ; le reste du fichier est identique octet pour octet
- [ ] Relancer `init` ne duplique pas la ligne ; une règle équivalente déjà présente est reconnue
- [ ] `init --no-gitignore` ne touche pas au .gitignore
- [ ] `notes_dir` personnalisé dans .coutcouticket.toml : c'est ce dossier qui est ignoré
- [ ] Notes déjà suivies par git : avertissement avec la commande `git rm -r --cached`, rien n'est retiré de l'index
- [ ] Tests e2e couvrant ces cas, README et architecture.md à jour

## Notes

