---
id: "0006"
title: "Faire fonctionner les hooks git hors du terminal"
type: fix
status: todo
priority: p1
projects: [core]
created: 2026-09-17
updated: 2026-09-17
---

## Description

Constaté dans le ticket 0002 (critère 5).

Les hooks `pre-commit` et `prepare-commit-msg` écrits par `coutcouticket init` (`src/init.rs`, gabarit autour de `HOOK_MARKER`) cherchent le binaire avec `command -v coutcouticket`. Un client git graphique lancé depuis le Dock hérite du PATH de launchd (`/usr/bin:/bin:/usr/sbin:/sbin`). Ce PATH ne contient ni `~/.cargo/bin` ni `/opt/homebrew/bin`.

Reproduction :
`env -i HOME="$HOME" PATH=/usr/bin:/bin:/usr/sbin:/sbin git commit --allow-empty -m "test PATH graphique"`
→ code 1, « coutcouticket introuvable dans le PATH. Installer le binaire (voir README) ou contourner ponctuellement avec --no-verify. »

Conséquence : dans tout projet initialisé, les commits sont impossibles depuis un client git graphique.

Piste : `init` écrit dans le hook le chemin absolu du binaire courant (`std::env::current_exe`), avec repli sur le PATH si ce chemin n'existe plus. Relancer `init` doit mettre à jour les hooks existants marqués `coutcouticket-hook`.

## Critères d'acceptation

- [ ] La commande de reproduction réussit dans un projet initialisé et le trailer « Ticket: <id> » est ajouté sur une branche de ticket
- [ ] Le binaire déplacé ou supprimé : le hook se rabat sur le PATH, puis affiche le message d'erreur actuel s'il reste introuvable
- [ ] Relancer `init` réécrit les hooks coutcouticket existants avec le nouveau gabarit, sans toucher aux hooks tiers
- [ ] Test de bout en bout couvrant l'exécution du hook avec un PATH réduit
- [ ] Critère 5 du ticket 0002 revérifié et coché

## Notes

