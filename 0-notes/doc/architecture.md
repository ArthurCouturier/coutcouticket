# Architecture de coutcouticket

## Vue d'ensemble

Un binaire unique, plusieurs façades sur un même cœur :

| Façade | Point d'entrée | Utilisée par |
|---|---|---|
| CLI | `src/main.rs` | l'utilisateur, les hooks |
| MCP stdio | `src/mcp.rs` (`serve_stdio`) | Claude Code en secours |
| Démon HTTP | `src/daemon.rs` (`run`) | Claude Code, via LaunchAgent (macOS) ou tâche planifiée (Windows) |
| Hooks | `src/hooks.rs` | git, Claude Code (SessionStart) |

Toute logique métier est dans `src/store.rs`. Les façades valident les entrées et
appellent le cœur, rien de plus.

## Modules

| Module | Responsabilité |
|---|---|
| `store.rs` | opérations sur les tickets : scan, création, statut, journal, décisions, contexte, validation |
| `model.rs` | frontmatter strict (parse/rendu), `Status`, `Priority`, `TicketId` |
| `naming.rs` | slug, nom de dossier, nom et classification de branche |
| `config.rs` | `.coutcouticket.toml`, registre des projets, configuration du démon, dossier de config globale |
| `board.rs` | rendu déterministe de `BOARD.md` |
| `overview.rs` | vue des tickets ouverts de tous les projets du registre (`overview`, `tickets_overview`, `OVERVIEW.md`) |
| `init.rs` | création et réparation idempotentes de l'arborescence, bloc CLAUDE.md, hooks git |
| `git.rs` | appels au binaire git |
| `claude.rs` | `setup-claude` : lecture de `claude mcp get`, ajout ou remplacement idempotent |
| `fsutil.rs` | écriture atomique, écriture si changement, verrou inter-processus, `canonicalize` sans préfixe `\\?\` |
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
- **Chemins portables** : toute canonicalisation passe par `fsutil::canonicalize`
  (Windows : `C:\…` et non `\\?\C:\…`, que git et les hooks ne comprennent pas) ;
  `Project::rel` rend les chemins relatifs avec des `/` sur toutes les plateformes
  (sorties CLI, MCP, `BOARD.md` identiques). Le frontmatter accepte les fins de ligne
  CRLF (git avec `core.autocrlf`) ; `.gitattributes` impose LF dans ce dépôt (gabarits
  `include_str!`, scripts sh).
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

## Vue de tous les projets (`overview.rs`)

- `overview::build(roots, filter)` : pour chaque racine du registre (`Registry::load().projects`),
  `Project::open` puis `scan`, et garde les tickets ouverts (`TicketSummary` complet, donc
  `open_blockers`) avec `project` (nom du dossier racine) et `project_path` (racine absolue,
  à passer aux outils `ticket_*`). Lecture seule, sans verrou.
- Tri : rang du statut dans `Status::ALL` (en cours, en revue, bloqué, à faire), priorité,
  nom du projet, chemin, id. Filtres optionnels statut et priorité ; un statut fermé est
  refusé (la vue ne montre que l'ouvert).
- **Un projet illisible ne fait jamais échouer la vue** : `.coutcouticket.toml` absent
  (introuvable), config invalide, `tickets/` absent ou erreur de lecture → une entrée dans
  `warnings` avec la correction. Des problèmes de scan (tickets invalides) ajoutent un
  avertissement qui renvoie vers `validate`, et les tickets valides restent affichés.
- Façades : CLI `overview [--status] [--priority] [--json]` (`render_text`), MCP
  `tickets_overview` (sans `project`, y compris en mode démon, qui ne vérifie alors aucun
  enregistrement : la vue ne lit que le registre).
- `OVERVIEW.md` dans le dossier de config globale : écrit par le démon uniquement
  (`regenerate_file`, `write_if_changed`), au démarrage du watcher puis après chaque lot
  où un projet ou le registre a changé. Rendu déterministe (`render_markdown`, aucun
  horodatage), liens absolus entre chevrons vers les `ticket.md`. Ses écritures dans le
  dossier global ne relancent rien (seul `projects.toml` y est suivi).

## Démon

- Streamable HTTP via rmcp, monté sur `/mcp` ; `/health` sans authentification.
- Sécurité : écoute sur 127.0.0.1, `allowed_hosts` restreint, jeton Bearer comparé
  en temps constant, toute requête portant un en-tête `Origin` refusée (403).
- En mode démon, le paramètre `project` est obligatoire et doit être enregistré
  (sauf `tickets_overview`, qui n'en a pas).
- Configuration globale (`config::global_dir`) : `COUTCOUTICKET_HOME`, sinon
  `%APPDATA%\coutcouticket` (Windows), sinon `$XDG_CONFIG_HOME/coutcouticket`, sinon
  `~/.config/coutcouticket`. Jeton de `daemon.toml` tiré par `getrandom`. Permissions :
  0600 sous Unix ; sous Windows, ACL héritées de `%APPDATA%` (utilisateur, SYSTEM,
  administrateurs), rien de codé.
- Ordre de démarrage de `run` : config du démon, **bind du port en premier**, puis
  construction du routeur, puis lancement du watcher dans son thread. Les connexions
  arrivées avant `axum::serve` attendent dans la file du noyau au lieu d'être refusées.
  Ne rien insérer de lent (registre, FSEvents, régénération des boards) avant le bind.
- Watcher (thread dédié, `notify`) : surveille le dossier de config globale (registre)
  et le dossier de notes de chaque projet enregistré. Il régénère les `BOARD.md` touchés
  et `OVERVIEW.md`.
- Journal (`daemon.log`) : chaque ligne commence par un horodatage à la milliseconde
  (macro `log!` → `write_log` de `daemon.rs`). Lignes clés : « lancement du démon … processus lancé
  il y a N ms » (délai exec → `run`, via `proc_pidinfo`, macOS uniquement),
  « à l'écoute … », « surveillance prête (n projet(s), N ms …) », « arrêt sur erreur : … ».
  Par défaut sur stderr (launchd le redirige) ; `daemon run --log-file F` (option cachée,
  utilisée par la tâche Windows) écrit dans `F` et note le pid dans `daemon.pid` à côté,
  **après** le bind (une instance refusée n'écrase pas le pid de l'instance active).
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
- Windows (module `daemon::windows`, XML dans `windows_task_xml`) : tâche planifiée
  `coutcouticket-daemon` (surchargeable par `COUTCOUTICKET_DAEMON_TASK`) à la racine du
  planificateur, créée par `schtasks /Create /XML` (fichier UTF-16 avec BOM,
  `%LOCALAPPDATA%\coutcouticket\daemon-task.xml`). Déclencheur `LogonTrigger` limité à
  `USERDOMAIN\USERNAME`, `InteractiveToken`, `LeastPrivilege` : aucun droit administrateur
  (un déclencheur pour tout utilisateur, comme `schtasks /SC ONLOGON`, en exige).
  `ExecutionTimeLimit PT0S` (72 h par défaut), pas d'arrêt sur batterie, `IgnoreNew`,
  `RestartOnFailure` (1 min, 999 fois), `Priority 5` (7 par défaut = sous la normale, même
  piège que le bridage launchd). Action : `conhost.exe --headless "<exe>" daemon run
  --log-file "%LOCALAPPDATA%\coutcouticket\daemon.log"` pour ne pas ouvrir de fenêtre
  console ; conhost absent → exe lancé directement, `daemon install` le signale.
  `install` et `uninstall` arrêtent d'abord l'instance en cours (`schtasks /End`, puis
  `taskkill` du pid de `daemon.pid` si `tasklist` confirme coutcouticket, puis attente de la
  libération du port, 5 s max) ; `install` lance ensuite la tâche (`/Run`) et attend
  `/health` (10 s). `daemon status` précise, en cas d'échec, si la tâche existe.
  L'export (`schtasks /Query /XML`) omet `LeastPrivilege`, valeur par défaut, et remplace
  l'utilisateur du `Principal` par son SID.
- Linux : pas de service (`daemon install` explique de lancer `daemon run` à la main).

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
- Windows : `Command::new("claude")` ne cherche que `claude.exe` ; `claude_bin` cherche
  `claude.exe`, `claude.cmd` (installation npm) puis `claude.bat` dans le PATH (`find_in_path`).

### Pièges du watcher

- Ne compter que les écritures (`write_paths`) : les événements de lecture
  existent sous Linux et relançaient la régénération en boucle.
- L'anti-rebond n'est prolongé que par des écritures et plafonné à 2 s : un
  lecteur continu ne doit pas pouvoir bloquer la régénération.
- Ignorer `BOARD.md` et les fichiers cachés (fichiers temporaires d'écriture atomique).
- Les événements portent des chemins canoniques (FSEvents : `/tmp` → `/private/tmp`).
  Le dossier de config globale est donc canonicalisé (`fsutil::canonicalize`) avant de comparer à `projects.toml` ;
  sinon un `HOME` ou `COUTCOUTICKET_HOME` derrière un lien symbolique masque les
  changements du registre. Les racines du registre sont déjà canoniques (`Registry::add`).

## Appels git (`git.rs`)

- Tout passe par `run` (binaire `COUTCOUTICKET_GIT_BIN`, défaut `git`) puis `failure`,
  qui construit l'erreur : commande, code de sortie, stderr, et correction connue
  (`known_fix` : licence Xcode → `sudo xcodebuild -license` ou
  `sudo xcode-select -s /Library/Developer/CommandLineTools` ; outils Apple absents →
  `xcode-select --install`). Binaire introuvable : message dédié.
- **« Pas un dépôt » ≠ « git en échec ».** `is_repo` renvoie `Result<bool>` : faux
  uniquement si `rev-parse` sort en 128 avec « not a git repository » (lancé avec
  `LC_ALL=C` pour que le texte ne soit pas traduit). Tout autre échec est une erreur.
  Piège d'origine : licence Xcode non acceptée, `/usr/bin/git` sort en 69, et l'ancien
  `is_repo` booléen faisait croire à un dossier hors dépôt.
- Codes attendus traités comme des réponses, pas des erreurs : `symbolic-ref --quiet`
  sort en 1 si HEAD est détaché, `show-ref --verify --quiet` en 1 si la branche
  n'existe pas, `check-ignore` en 1 si le chemin n'est pas ignoré.
- Propagation : `files` et `start` échouent avec le message de git. `context`
  (`show`, `ticket_context`) ne bloque pas : `current_branch` à null et `git_error`
  renseigné (null si git fonctionne ; hors dépôt, les deux sont null). Le hook
  SessionStart ajoute une ligne « git en échec ». `init` avertit et saute hooks git et
  `.gitignore` (impossible de savoir si les notes sont suivies).

## Hooks git

- Générés par `init.rs` (`hook_script`), reconnus au marqueur `# coutcouticket-hook`.
  Un hook sans marqueur n'est jamais modifié ; un hook marqué est réécrit si son
  contenu diffère du gabarit.
- Le hook appelle le binaire par le chemin absolu noté lors de `init`
  (`current_exe`, non canonicalisé), avec repli sur le PATH. Piège : un client git
  graphique hérite du PATH de launchd (`/usr/bin:/bin:/usr/sbin:/sbin`), sans
  `~/.cargo/bin` ni `/opt/homebrew/bin`. Binaire déplacé hors du PATH → relancer `init`.
- Windows : Git for Windows exécute les hooks avec son `sh` (MSYS). Le chemin noté est
  converti en `C:/…/coutcouticket.exe` (`windows_sh_path`), que MSYS accepte tel quel ;
  `[ -x ]` et `command -v coutcouticket` y trouvent le `.exe`. Pas de bit d'exécution à poser.
- `pre-commit` régénère `BOARD.md` et ne l'ajoute au commit que s'il n'est pas ignoré
  (`git::is_ignored`) : `git add` d'un chemin ignoré échoue et bloquerait le commit.
- `prepare-commit-msg` (sauf merge) choisit les trailers `Ticket:` d'après les fichiers
  indexés (`git::staged_files`, qui respecte le `GIT_INDEX_FILE` de `commit -a`) via
  `Project::commit_tickets` : si le commit ne touche que des notes, dont au moins un
  dossier de ticket et pas celui du ticket de la branche, il reçoit les trailers des
  tickets touchés ; sinon (code, dossier du ticket de la branche, doc ou `BOARD.md`
  seuls, index vide) celui de la branche. Un message qui porte déjà un trailer
  `Ticket:` n'est pas modifié (`git::add_trailers`). Notes ignorées par git : jamais
  indexées, donc toujours le trailer de la branche.
- Rattachement d'un chemin : `Project::path_owner` → `Code`, `Ticket(id)` (dossier
  `tickets/<id>-<slug>/`, reconnu par `naming::parse_dir_name`), `Board` ou `Notes`.
  Piège : git donne des chemins relatifs à la racine du dépôt, pas du projet ; le
  préfixe du projet (`git::show_prefix`) est retiré d'abord.
- `files <id>` (`Project::files`) : fichiers des commits portant le trailer, plus les
  changements non commités sur la branche du ticket, sans les dossiers des autres
  tickets ni `BOARD.md`. Le filtre corrige aussi les commits mal attribués avant ce
  comportement.

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
- `overview.rs` : plusieurs projets temporaires (dont un introuvable, un à config invalide,
  un avec ticket invalide) : tri, filtres, avertissements, rendu texte et Markdown déterministe.
- `tests/e2e.rs` : binaire réel dans un dépôt git temporaire, avec
  `COUTCOUTICKET_HOME` isolé. Couvre le workflow et les hooks, les trailers des commits de notes et `files`,
  les dépendances (CLI),
  la vue `overview` sur trois projets enregistrés dont un supprimé (texte, JSON, filtres,
  `tickets_overview` en stdio), le MCP stdio, et le démon HTTP (authentification, Origin,
  watcher sous lecture continue, `tickets_overview` sans `project`, `OVERVIEW.md` suivant
  les notes et le registre, écoute avant le watcher et journal horodaté), les hooks avec le PATH de
  launchd, le .gitignore des notes, et `setup-claude --apply` avec un faux `claude`
  (script shell : absent, identique, différent, autre portée, échec), et git en échec
  avec un faux git (`COUTCOUTICKET_GIT_BIN` : licence Xcode en code 69, panne en 128,
  binaire introuvable) comparé à un vrai dossier hors dépôt.
- `git.rs` : message d'échec et correction Xcode testés sur des `Output` construits.
- Plateformes : la CI tourne sur `macos-15` et `windows-latest`. Réservés à Unix (scripts
  sh simulés, PATH de launchd) : `hooks_hors_du_path`, `setup_claude_idempotent`,
  `git_en_echec`, `plist_is_well_formed`. Réservés à Windows : `hooks_git_for_windows`
  (sh de Git for Windows, binaire noté en `C:/…`, PATH sans le binaire, binaire disparu)
  et `daemon_tache_planifiee_windows` (install, export XML, status, réinstallation avec
  changement de pid, uninstall). Ce dernier installe réellement la tâche dans la session :
  il ne s'exécute que si `COUTCOUTICKET_TEST_WINDOWS_SERVICE` est défini (CI), et utilise
  la config réelle (`%APPDATA%`), la tâche ne recevant pas l'environnement du test.
  `daemon_journal_dans_un_fichier` (toutes plateformes) couvre `--log-file` et `daemon.pid`.
  Chemins canoniques comparés via `canon` (sans `\\?\`).
