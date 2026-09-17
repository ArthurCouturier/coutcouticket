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
- **Le frontmatter n'accepte que les 9 clés connues.** Ajouter un champ = modifier
  `FRONTMATTER_KEYS`, `Frontmatter`, `render` et le parse ensemble. Une clé optionnelle
  (comme `blocked_by`) n'est écrite que si elle a une valeur : les tickets existants
  ne changent pas.
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

## Dépendances entre tickets

- `blocked_by: ["0003", "0007"]` dans le frontmatter, optionnel (absent = aucune).
  Parse tolérant (`3`, `#3`, `'0003'`), liste toujours triée et dédoublonnée
  (`model::normalize_ids`), réécrite sur `id_width` chiffres.
- Écriture : `Project::create` (`CreateInput.blocked_by`) et `Project::depend`
  (ajout ou retrait, entrée « dépendances » dans le journal). Façades :
  `new --blocked-by`, `depend <id> --on <ids> [--remove]`, MCP `ticket_create.blocked_by`
  et `ticket_depend`.
- Contrôles à l'écriture (`check_new_dependencies`) : ids existants, pas
  d'auto-référence, pas de cycle (`find_path` du nouveau prérequis vers le ticket).
  Le retrait ne vérifie pas l'existence : il sert à réparer un id inconnu.
- Contrôles à la lecture : `scan` ajoute aux problèmes les ids inconnus, les
  auto-références et chaque cycle une fois (`find_cycles`), avec la commande de
  correction. Ils apparaissent donc dans `validate`, `BOARD.md` et le contexte de session.
- « Bloquant » = dépendance existante dont le statut est ouvert
  (`Project::open_blockers`). `TicketSummary` expose `blocked_by` et `open_blockers`
  (`list`, `show`, `ticket_context`, retour des outils d'écriture).
- `BOARD.md` : colonne « Bloqué par » (liens) dans les sections de statut ouvert
  uniquement. Le statut `blocked` reste manuel et indépendant des dépendances.
- `start` n'est pas refusé sur un ticket bloqué : la CLI avertit, le hook
  SessionStart signale les dépendances ouvertes du ticket de la branche courante.

## Démon

- Streamable HTTP via rmcp, monté sur `/mcp` ; `/health` sans authentification.
- Sécurité : écoute sur 127.0.0.1, `allowed_hosts` restreint, jeton Bearer comparé
  en temps constant, toute requête portant un en-tête `Origin` refusée (403).
- En mode démon, le paramètre `project` est obligatoire et doit être enregistré.
- Ordre de démarrage de `run` : config du démon, **bind du port en premier**, puis
  construction du routeur, puis lancement du watcher dans son thread. Les connexions
  arrivées avant `axum::serve` attendent dans la file du noyau au lieu d'être refusées.
  Ne rien insérer de lent (registre, FSEvents, régénération des boards) avant le bind.
- Watcher (thread dédié, `notify`) : surveille le dossier de config globale (registre)
  et le dossier de notes de chaque projet enregistré.
- Journal (`daemon.log`) : chaque ligne commence par un horodatage à la milliseconde
  (macro `log!` de `daemon.rs`). Lignes clés : « lancement du démon … processus lancé
  il y a N ms » (délai exec → `run`, via `proc_pidinfo`, macOS uniquement),
  « à l'écoute … », « surveillance prête (n projet(s), N ms …) ».
- macOS : LaunchAgent `app.coutcouticket.daemon` (`RunAtLoad`, `KeepAlive`,
  `ProcessType=Interactive`, sans `Nice` ni `LowPriorityIO`), journal dans
  `~/Library/Logs/coutcouticket/daemon.log`. Piège : avec `ProcessType=Background`,
  `Nice` et `LowPriorityIO` (ancien plist), le port n'écoutait qu'environ 1 min après
  l'ouverture de session et la première session Claude Code n'avait pas le MCP
  (connexion refusée, `/mcp` pour reconnecter). L'ancien code écoutait déjà avant
  l'initialisation du watcher : le bridage launchd pendant la charge de connexion est
  la cause retenue. Ne pas le réintroduire : le démon est événementiel et ne consomme
  rien au repos, le bridage ne fait que retarder le démarrage. Le plist installé n'est
  réécrit que par `daemon install` (à relancer après toute modification du gabarit).

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

## Plugin Claude Code

Dossier `plugin/` (manifeste `plugin/.claude-plugin/plugin.json`, marketplace à la
racine dans `.claude-plugin/marketplace.json`). Les composants sont découverts par
convention de dossier, sans déclaration dans le manifeste :

| Composant | Fichier | Rôle |
|---|---|---|
| Skill `ticket` | `plugin/skills/ticket/SKILL.md` | procédures (Créer, Démarrer, Journaliser, Décider, Clôturer) |
| Subagent `relecteur-cloture` | `plugin/agents/relecteur-cloture.md` | relecture de clôture en lecture seule, verdict `OK` / `À CORRIGER` |
| Hook SessionStart | `plugin/hooks/hooks.json` | appelle `coutcouticket hook session-start` |

- Le relecteur est invoqué par la procédure « Clôturer » sous le nom
  `coutcouticket:relecteur-cloture`, avec l'id du ticket et la racine du projet. Il juge,
  donc il vit dans le plugin et non dans le binaire. Il n'utilise que la CLI
  (`show`, `files`, `validate`, `list`) et git en lecture : pas de dépendance au démon MCP.
- Lecture seule : `disallowedTools: Write, Edit, NotebookEdit`, et le prompt limite Bash
  aux commandes de lecture. Un échec git doit apparaître dans la section « Git » du
  verdict, qui passe alors à `À CORRIGER`.
- Vérifier la structure : `claude plugin validate --strict plugin` (couvre aussi les
  agents). Un agent de plugin ne peut pas déclarer `hooks`, `mcpServers` ni
  `permissionMode`.

## Tests

- Unitaires dans chaque module (`cargo test`).
- `store.rs` : tests du cœur sur un projet temporaire sans git (dépendances, cycles, board).
- `tests/e2e.rs` : binaire réel dans un dépôt git temporaire, avec
  `COUTCOUTICKET_HOME` isolé. Couvre le workflow et les hooks, les dépendances (CLI),
  le MCP stdio, et le démon HTTP (authentification, Origin, watcher sous lecture
  continue, écoute avant le watcher et journal horodaté), les hooks avec le PATH de
  launchd, le .gitignore des notes, et `setup-claude --apply` avec un faux `claude`
  (script shell : absent, identique, différent, autre portée, échec).
