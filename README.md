# coutcouticket

Système de tickets et de documentation technique **en Markdown, dans chaque projet**,
conçu pour travailler avec Claude Code sans friction.

- Les tickets vivent dans `0-notes/` de chaque projet, versionnés avec le code.
- Tout ce qui est mécanique (identifiants, statuts, branches, board) est fait par un
  binaire Rust unique : CLI, serveur MCP, démon et hooks.
- Claude ne fait que ce qui demande du jugement : rédiger, décider, documenter.

## Architecture

```
coutcouticket (binaire unique, ~5 Mo)
├── CLI           init, new, start, status, log, decide, list, show, files, board, validate
├── mcp           serveur MCP stdio (secours)
├── daemon run    serveur MCP HTTP 127.0.0.1 + surveillance des notes (LaunchAgent)
└── hook          pre-commit, prepare-commit-msg, session-start
          │
          ▼
src/store.rs      cœur déterministe partagé par toutes les façades
```

Principes :
- **Les fichiers sont la source de vérité.** Le démon accélère, il ne détient rien.
  Les hooks git passent par la CLI et fonctionnent démon arrêté.
- **Contraintes plutôt que consignes.** Statuts en enum dans le schéma MCP,
  frontmatter strict, branche créée uniquement par l'outil, commit refusé si non conforme.
- **Sobriété.** Démon mesuré à ~6 Mo de RAM et 0 CPU au repos. Surveillance
  événementielle (FSEvents), hooks git en ~10 ms sur 200 tickets.

## Installation (une fois par machine)

Prérequis : Rust ≥ 1.89 (`rustup`), git, Claude Code.

```sh
cd ~/dev/coutcouticket
cargo install --path .              # installe ~/.cargo/bin/coutcouticket
coutcouticket daemon install        # LaunchAgent : démarre au login, relancé s'il tombe
coutcouticket daemon status         # → « coutcouticket 0.1.0 ok »
coutcouticket setup-claude --apply  # enregistre le MCP du démon dans Claude Code (portée utilisateur)
```

Plugin Claude Code (skill `ticket` + hook de démarrage de session), dans Claude Code :

```
/plugin marketplace add ~/dev/coutcouticket
/plugin install coutcouticket@coutcouticket
```

`~/.cargo/bin` doit être dans le `PATH` : les hooks git et le hook de session
appellent `coutcouticket`.

## Dans chaque projet

```sh
cd ~/dev/mon-projet
coutcouticket init
```

`init` est idempotent : il crée ce qui manque et répare une arborescence incomplète
sans jamais écraser de contenu.

- `.coutcouticket.toml` : types de branche, branches exemptées, largeur des id, projets
- `0-notes/0-global/README.md` : fonctionnement et méthode de recherche
- `0-notes/0-global/BOARD.md` : vue générée de tous les tickets
- `0-notes/tickets/` et `0-notes/doc/INDEX.md`
- les fichiers `journal.md` / `decisions.md` manquants des tickets existants
- le bloc coutcouticket dans `CLAUDE.md`
- les hooks git `pre-commit` et `prepare-commit-msg`
- l'enregistrement du projet auprès du démon

## Conventions

| Élément | Format |
|---|---|
| Dossier | `0-notes/tickets/0013-add-thing-to-etc/` |
| Branche | `feat/0013-add-thing-to-etc` (types : feat, fix, refacto, design, ci) |
| Branches exemptées | main, master, develop, dev, stag, staging |
| Trailer de commit | `Ticket: 0013` (ajouté automatiquement) |
| Statuts | todo, in-progress, blocked, review, done, cancelled |
| Priorités | p0 … p3 |

## Outils MCP

| Outil | Rôle |
|---|---|
| `ticket_create` | crée le ticket (id suivant, dossier, fichiers, board) |
| `ticket_start` | bascule/crée la branche, passe en `in-progress` |
| `ticket_set_status` | change le statut, consigné dans le journal |
| `ticket_log` | entrée d'avancement, « prochaine étape » obligatoire |
| `ticket_decide` | décision structurée |
| `ticket_list` | liste filtrable |
| `ticket_context` | point d'entrée de reprise |
| `ticket_files` | fichiers modifiés, dérivés de git |
| `notes_validate` | vérification complète |

## Diagnostic

```sh
coutcouticket daemon status
tail -f ~/Library/Logs/coutcouticket/daemon.log
coutcouticket projects list
coutcouticket validate
COUTCOUTICKET_DEBUG=1 coutcouticket daemon run   # démon au premier plan, événements détaillés
```

Secours sans démon : `claude mcp add --scope user coutcouticket-stdio -- coutcouticket mcp`.

Configuration globale : `~/.config/coutcouticket/` (`projects.toml`, `daemon.toml` avec
le port et le jeton, en 0600).

## Développement

```sh
cargo test            # unitaires + bout en bout (binaire réel, git, hooks, MCP stdio, démon HTTP)
cargo build --release
```

Les tickets de coutcouticket lui-même sont dans `0-notes/`.
