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
├── CLI           init, new, start, status, depend, log, decide, list, overview, ui, show, files, board, validate
├── mcp           serveur MCP stdio (secours)
├── daemon run    serveur MCP HTTP 127.0.0.1 + panneau web + surveillance des notes (LaunchAgent / tâche planifiée)
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
  événementielle (FSEvents, ReadDirectoryChangesW), hooks git en ~10 ms sur 200 tickets.

## Installation (une fois par machine)

Prérequis : macOS (Apple Silicon ou Intel) ou Windows 10/11 x64, git, Claude Code.

### macOS

Binaire publié (sans toolchain Rust) :

```sh
curl -fsSL https://raw.githubusercontent.com/ArthurCouturier/coutcouticket/main/install.sh | sh
coutcouticket daemon install        # LaunchAgent : démarre au login, relancé s'il tombe
coutcouticket daemon status         # → « coutcouticket 0.2.0 ok »
coutcouticket setup-claude --apply  # enregistre le MCP du démon dans Claude Code (portée utilisateur)
```

Le script télécharge la dernière release GitHub, vérifie sa somme SHA-256, retire la
quarantaine Gatekeeper et installe dans `~/.local/bin` (ou à la place du `coutcouticket`
déjà présent dans le `PATH`). Options : `sh -s -- --version 0.2.0 --dir ~/bin --no-daemon`.
Archives manuelles : page [Releases](https://github.com/ArthurCouturier/coutcouticket/releases)
(`coutcouticket-<version>-aarch64-apple-darwin.tar.gz` et `.sha256`).

### Windows

Prérequis : [Git for Windows](https://git-scm.com/download/win) (ses hooks s'exécutent
avec le `sh` qu'il fournit). Aucun droit administrateur n'est nécessaire. Dans PowerShell :

```powershell
irm https://raw.githubusercontent.com/ArthurCouturier/coutcouticket/main/install.ps1 | iex
coutcouticket daemon install        # tâche planifiée : démarre à l'ouverture de session, sans fenêtre
coutcouticket daemon status         # → « coutcouticket 0.2.0 ok »
coutcouticket setup-claude --apply  # enregistre le MCP du démon dans Claude Code (portée utilisateur)
```

Le script télécharge `coutcouticket-<version>-x86_64-pc-windows-msvc.zip`, vérifie sa somme
SHA-256, installe `coutcouticket.exe` dans `%LOCALAPPDATA%\Programs\coutcouticket` (ou à la
place de celui déjà présent dans le `PATH`) et ajoute ce dossier au `PATH` de l'utilisateur
(ouvrir un nouveau terminal ensuite). Paramètres :
`& ([scriptblock]::Create((irm .../install.ps1))) -Version 0.2.0 -Dir C:\outils -NoDaemon`.

Installation manuelle : télécharger le `.zip` et son `.sha256` depuis la page
[Releases](https://github.com/ArthurCouturier/coutcouticket/releases), vérifier
(`Get-FileHash -Algorithm SHA256 <zip>`), extraire `coutcouticket.exe` dans un dossier du
`PATH` utilisateur, puis lancer les trois commandes ci-dessus.

`daemon install` crée la tâche planifiée `coutcouticket-daemon` (déclencheur : ouverture de
session de l'utilisateur courant, relance en cas d'échec) et la démarre ; `daemon uninstall`
l'arrête et la retire. Configuration dans `%APPDATA%\coutcouticket`, journal dans
`%LOCALAPPDATA%\coutcouticket\daemon.log`.

### Depuis les sources

Toutes plateformes (Rust ≥ 1.89, `rustup`) :

```sh
cd ~/dev/coutcouticket
cargo install --path .              # installe ~/.cargo/bin/coutcouticket
# puis daemon install, daemon status et setup-claude --apply comme ci-dessus
```

`setup-claude --apply` peut être relancé sans danger : il lit l'enregistrement existant
(`claude mcp get coutcouticket`). Identique (même URL, même jeton) : rien n'est modifié.
Différent (port ou jeton changé, autre portée) : l'ancien est retiré de sa portée
(`claude mcp remove --scope …`) puis recréé en portée utilisateur. Sans `--apply`, la
commande `claude mcp add` est seulement affichée. Binaire `claude` hors du PATH :
`COUTCOUTICKET_CLAUDE_BIN=/chemin/vers/claude`.

Plugin Claude Code (skill `ticket`, subagent `relecteur-cloture` + hook de démarrage de
session), dans Claude Code :

```
/plugin marketplace add ~/dev/coutcouticket
/plugin install coutcouticket@coutcouticket
```

Sans clone local, la marketplace s'ajoute depuis GitHub :
`/plugin marketplace add ArthurCouturier/coutcouticket`.

Le subagent `coutcouticket:relecteur-cloture` relit un ticket avant sa clôture, en
lecture seule et avec un contexte neuf : critères cochés réellement prouvés, doc et
`INDEX.md` à jour, journal final prêt. Il rend un verdict `OK` ou `À CORRIGER`. La
procédure « Clôturer » du skill l'invoque avant `status done`.

Le dossier du binaire (`~/.local/bin` ou `~/.cargo/bin`) doit être dans le `PATH` : le hook de session appelle `coutcouticket`.
Les hooks git, eux, appellent le binaire par le chemin noté lors de `init` (avec repli
sur le `PATH`) : ils fonctionnent aussi depuis un client git graphique. Après avoir
déplacé le binaire, relancer `coutcouticket init` dans chaque projet.

## Mise à jour

```sh
curl -fsSL https://raw.githubusercontent.com/ArthurCouturier/coutcouticket/main/install.sh | sh
# Windows (PowerShell) :
irm https://raw.githubusercontent.com/ArthurCouturier/coutcouticket/main/install.ps1 | iex
```

Le script remplace le binaire au même emplacement (les hooks git, le LaunchAgent et la
tâche planifiée restent valides) puis, si le démon est installé, lance `coutcouticket daemon install` :
idempotent, il garde port et jeton et redémarre le démon sur la nouvelle version.
Vérifier avec `coutcouticket daemon status`.

Depuis les sources : `cargo install --path .` puis `coutcouticket daemon install`
(sans cette seconde commande, le démon continue d'exécuter l'ancienne version).

Binaire installé dans un autre dossier qu'avant : relancer `coutcouticket init` dans
chaque projet (`coutcouticket projects list`) pour mettre à jour le chemin des hooks git.

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
- la ligne `/0-notes/` dans `.gitignore` : les notes restent hors dépôt
- l'enregistrement du projet auprès du démon

Options : `--no-git-hooks`, `--no-claude-md`, `--no-register`, `--no-gitignore`.

Notes hors dépôt : pas de partage par git ni de visibilité dans les PR ; `BOARD.md`
est régénéré par `pre-commit` sans être ajouté au commit. Si des fichiers de
`0-notes/` sont déjà suivis par git, `init` ne touche pas au `.gitignore` et indique
comment sortir les notes du dépôt (`git rm -r --cached 0-notes`).

> **Attention :** après cette migration, basculer sur une branche ou un commit qui suit
> encore `0-notes/` (ancienne branche, `main` non fusionnée, `git bisect`) écrase sans
> prévenir les notes locales par leur ancienne version, et le retour les supprime.
> Fusionner la migration partout et sauvegarder `0-notes/` avant tout changement de branche.

## Conventions

| Élément | Format |
|---|---|
| Dossier | `0-notes/tickets/0013-add-thing-to-etc/` |
| Branche | `feat/0013-add-thing-to-etc` (types : feat, fix, refacto, design, ci) |
| Branches exemptées | main, master, develop, dev, stag, staging |
| Trailer de commit | `Ticket: 0013` (ajouté automatiquement ; un commit qui ne touche que les notes d'autres tickets reçoit leurs trailers) |
| Statuts | todo, in-progress, blocked, review, done, cancelled |
| Priorités | p0 … p3 |
| Dépendances | `blocked_by: ["0003"]`, via `new --blocked-by 3` ou `depend 13 --on 3 [--remove]` |

Dépendances : un ticket ouvert affiche dans `BOARD.md` (colonne « Bloqué par ») ses
dépendances encore ouvertes ; une dépendance `done` ou `cancelled` ne bloque plus.
`list`, `show` et `ticket_context` exposent `blocked_by` et `open_blockers`.

## Vue de tous les projets

```sh
coutcouticket overview                          # tickets ouverts de tous les projets enregistrés
coutcouticket overview --status todo --priority p1
coutcouticket overview --json
```

Tickets ouverts triés par statut (en cours, en revue, bloqué, puis à faire), priorité,
projet et id, avec le nom du projet et les dépendances encore ouvertes. Un projet
enregistré mais introuvable, ou dont les notes sont invalides, est signalé en fin de
liste (avec la correction) sans faire échouer la vue. Même contenu en MCP :
`tickets_overview`, sans paramètre `project`.

Le démon tient aussi à jour `~/.config/coutcouticket/OVERVIEW.md` (régénéré à chaque
changement de notes ou du registre, réécrit seulement si son contenu change). Démon
arrêté, le fichier peut être en retard : `overview` reste la vue en direct.

### Panneau web

```sh
coutcouticket ui            # ouvre le panneau dans le navigateur par défaut
coutcouticket ui --print    # affiche l'adresse de connexion sans ouvrir le navigateur
```

Kanban en lecture seule des tickets ouverts de tous les projets : colonnes en cours, en
revue, bloqué, à faire ; filtres par projet, statut et priorité ; dépendances ouvertes.
Un clic sur un ticket affiche sa description, ses critères, ses décisions, son journal et
le chemin de son `ticket.md`. La page se met à jour seule quand une note change (le
démon pousse les changements, sans sondage). Elle est servie par le démon
(`http://127.0.0.1:47813/ui/`), embarquée dans le binaire, sans CDN ni dépendance
externe ; sans onglet ouvert, elle ne coûte rien.

Accès : `ui` obtient du démon un lien de connexion à usage unique (valable 2 minutes)
qui pose un cookie de session. Après un redémarrage du démon, la page indique
« session expirée » : relancer `coutcouticket ui`. Démon injoignable ou d'une version
antérieure : `coutcouticket daemon install`, puis `coutcouticket daemon status`.

## Outils MCP

| Outil | Rôle |
|---|---|
| `ticket_create` | crée le ticket (id suivant, dossier, fichiers, board), dépendances optionnelles |
| `ticket_start` | bascule/crée la branche, passe en `in-progress` |
| `ticket_set_status` | change le statut, consigné dans le journal |
| `ticket_depend` | ajoute ou retire des dépendances (`blocked_by`), refuse ids inconnus et cycles |
| `ticket_log` | entrée d'avancement, « prochaine étape » obligatoire |
| `ticket_decide` | décision structurée |
| `ticket_list` | liste filtrable |
| `tickets_overview` | tickets ouverts de tous les projets enregistrés, sans `project` (filtres statut, priorité) |
| `ticket_context` | point d'entrée de reprise |
| `ticket_files` | fichiers modifiés, dérivés de git (sans les notes des autres tickets ni `BOARD.md`) |
| `notes_validate` | vérification complète |

## Diagnostic

```sh
coutcouticket daemon status
tail -f ~/Library/Logs/coutcouticket/daemon.log                      # macOS
Get-Content -Wait $env:LOCALAPPDATA\coutcouticket\daemon.log          # Windows
schtasks /Query /TN coutcouticket-daemon /V /FO LIST                  # Windows : état de la tâche
coutcouticket projects list
coutcouticket validate
COUTCOUTICKET_DEBUG=1 coutcouticket daemon run   # démon au premier plan, événements détaillés
```

Git en échec : `files`, `start` et `show` citent la commande git, son code et sa sortie
d'erreur (`ticket_context` : champ `git_error`), distincts du cas « pas un dépôt git ».
Sous macOS, le code 69 « You have not agreed to the Xcode license agreements » se corrige
par `sudo xcodebuild -license` (ou `sudo xcode-select -s /Library/Developer/CommandLineTools`).
Autre binaire git : `COUTCOUTICKET_GIT_BIN`.

Secours sans démon : `claude mcp add --scope user coutcouticket-stdio -- coutcouticket mcp`.

Configuration globale : `~/.config/coutcouticket/` (Windows : `%APPDATA%\coutcouticket\`)
avec `projects.toml`, `daemon.toml` (port et jeton ; 0600 sous macOS, ACL du profil sous
Windows) et `OVERVIEW.md` généré par le démon.

## Développement

```sh
cargo test            # unitaires + bout en bout (binaire réel, git, hooks, MCP stdio, démon HTTP)
cargo clippy --all-targets -- -D warnings   # exigé par la CI
cargo build --release
```

Publier une version : aligner `version` dans `Cargo.toml`, commiter, puis
`git tag vX.Y.Z && git push origin vX.Y.Z`. Le workflow `release` teste, construit les
binaires macOS (arm64, x86_64) et Windows (x64), et crée la release GitHub. Détails :
`0-notes/doc/distribution.md`.

Les tickets de coutcouticket lui-même sont dans `0-notes/`.
