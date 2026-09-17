# Architecture de coutcouticket

## Vue d'ensemble

Un binaire unique, plusieurs façades sur un même cœur :

| Façade | Point d'entrée | Utilisée par |
|---|---|---|
| CLI | `src/main.rs` | l'utilisateur, les hooks |
| MCP stdio | `src/mcp.rs` (`serve_stdio`) | Claude Code en secours |
| Démon HTTP | `src/daemon.rs` (`run`) | Claude Code, via LaunchAgent |
| Hooks | `src/hooks.rs` | git, Claude Code (SessionStart) |

Toute logique métier est dans `src/store.rs`. Les façades valident les entrées et
appellent le cœur, rien de plus.

## Modules

| Module | Responsabilité |
|---|---|
| `store.rs` | opérations sur les tickets : scan, création, statut, journal, décisions, contexte, validation |
| `model.rs` | frontmatter strict (parse/rendu), `Status`, `Priority`, `TicketId` |
| `naming.rs` | slug, nom de dossier, nom et classification de branche |
| `config.rs` | `.coutcouticket.toml`, registre des projets, configuration du démon |
| `board.rs` | rendu déterministe de `BOARD.md` |
| `init.rs` | création et réparation idempotentes de l'arborescence, bloc CLAUDE.md, hooks git |
| `git.rs` | appels au binaire git |
| `claude.rs` | `setup-claude` : lecture de `claude mcp get`, ajout ou remplacement idempotent |
| `fsutil.rs` | écriture atomique, écriture si changement, verrou inter-processus |
| `templates.rs` + `templates/` | contenus embarqués dans le binaire |

## Invariants

- **Toute écriture passe par `ProjectLock`** (fichier de verrou dans le dossier
  temporaire système, clé = hachage FNV stable de la racine). CLI, démon et hooks
  peuvent tourner en même temps.
- **`BOARD.md` est déterministe** : aucun horodatage, écrit seulement si le contenu
  change. Sinon bruit git et boucle avec le watcher.
- **Le frontmatter n'accepte que les 8 clés connues.** Ajouter un champ = modifier
  `FRONTMATTER_KEYS`, `Frontmatter`, `render` et le parse ensemble.
- **Suffixe de branche = nom de dossier**, sans conversion.
- **`init` ne réécrit jamais un contenu utilisateur** : `create_if_missing` partout,
  sauf le bloc balisé de `CLAUDE.md` et les hooks portant le marqueur coutcouticket.
  Le `.gitignore` n'est modifié que par ajout en fin de fichier.
- **Notes versionnées ou non : jamais changé à l'insu de l'utilisateur.** `init` ajoute
  `/<notes_dir>/` au `.gitignore` (option `--no-gitignore`) sauf si une règle équivalente
  existe (`ignores_dir`) ou si des fichiers de notes sont déjà suivis (`git::has_tracked_files`) :
  dans ce cas, avertissement seulement.
  Piège de la migration (`git rm -r --cached`) : git considère les fichiers ignorés comme
  jetables. Basculer vers un commit qui suit encore les notes les écrase, et le retour
  les supprime. L'avertissement d'`init` le signale.

## Démon

- Streamable HTTP via rmcp, monté sur `/mcp` ; `/health` sans authentification.
- Sécurité : écoute sur 127.0.0.1, `allowed_hosts` restreint, jeton Bearer comparé
  en temps constant, toute requête portant un en-tête `Origin` refusée (403).
- En mode démon, le paramètre `project` est obligatoire et doit être enregistré.
- Watcher (thread dédié, `notify`) : surveille le dossier de config globale (registre)
  et le dossier de notes de chaque projet enregistré.
- macOS : LaunchAgent `app.coutcouticket.daemon` (`RunAtLoad`, `KeepAlive`),
  journal dans `~/Library/Logs/coutcouticket/daemon.log`. Piège : avec
  `ProcessType=Background`, `Nice` et `LowPriorityIO`, le démon a mis environ 1 min
  à écouter après l'ouverture de session. Une session Claude Code ouverte dans ce
  délai n'a pas le MCP (connexion refusée, `/mcp` pour reconnecter).

### Enregistrement dans Claude Code (`claude.rs`)

- `setup-claude --apply` : `claude mcp get coutcouticket` → absent (code ≠ 0) : `add` ;
  identique (transport, URL, en-têtes) : rien ; différent : `remove --scope <portée lue>`
  puis nouvelle lecture, en boucle bornée (6 étapes), jusqu'à l'état attendu.
- `claude` n'a pas de sortie JSON pour `mcp get` : on lit le texte (`Type:`, `URL:`,
  bloc `Headers:` indenté, portée via la ligne « claude mcp remove … -s <portée> » ou `Scope:`).
  Un changement de format de Claude Code casse `parse_get` : tests unitaires dédiés.
- Piège : `claude mcp add` refuse un nom existant, quelle que soit la valeur. Et une
  portée `local`/`project` masque la portée `user` : elle est retirée aussi.
- Les jetons (`Bearer …`) sont masqués dans les messages d'erreur qui reprennent la
  sortie de `claude`. Binaire surchargeable par `COUTCOUTICKET_CLAUDE_BIN`.

### Pièges du watcher

- Ne compter que les écritures (`write_paths`) : les événements de lecture
  existent sous Linux et relançaient la régénération en boucle.
- L'anti-rebond n'est prolongé que par des écritures et plafonné à 2 s : un
  lecteur continu ne doit pas pouvoir bloquer la régénération.
- Ignorer `BOARD.md` et les fichiers cachés (fichiers temporaires d'écriture atomique).

## Hooks git

- Générés par `init.rs` (`hook_script`), reconnus au marqueur `# coutcouticket-hook`.
  Un hook sans marqueur n'est jamais modifié ; un hook marqué est réécrit si son
  contenu diffère du gabarit.
- Le hook appelle le binaire par le chemin absolu noté lors de `init`
  (`current_exe`, non canonicalisé), avec repli sur le PATH. Piège : un client git
  graphique hérite du PATH de launchd (`/usr/bin:/bin:/usr/sbin:/sbin`), sans
  `~/.cargo/bin` ni `/opt/homebrew/bin`. Binaire déplacé hors du PATH → relancer `init`.
- `pre-commit` régénère `BOARD.md` et ne l'ajoute au commit que s'il n'est pas ignoré
  (`git::is_ignored`) : `git add` d'un chemin ignoré échoue et bloquerait le commit.

## Tests

- Unitaires dans chaque module (`cargo test`).
- `tests/e2e.rs` : binaire réel dans un dépôt git temporaire, avec
  `COUTCOUTICKET_HOME` isolé. Couvre le workflow et les hooks, le MCP stdio, et le
  démon HTTP (authentification, Origin, watcher sous lecture continue), les hooks
  avec le PATH de launchd, le .gitignore des notes, et `setup-claude --apply` avec un
  faux `claude` (script shell : absent, identique, différent, autre portée, échec).
