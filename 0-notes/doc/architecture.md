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

## Démon

- Streamable HTTP via rmcp, monté sur `/mcp` ; `/health` sans authentification.
- Sécurité : écoute sur 127.0.0.1, `allowed_hosts` restreint, jeton Bearer comparé
  en temps constant, toute requête portant un en-tête `Origin` refusée (403).
- En mode démon, le paramètre `project` est obligatoire et doit être enregistré.
- Watcher (thread dédié, `notify`) : surveille le dossier de config globale (registre)
  et le dossier de notes de chaque projet enregistré.

### Pièges du watcher

- Ne compter que les écritures (`write_paths`) : les événements de lecture
  existent sous Linux et relançaient la régénération en boucle.
- L'anti-rebond n'est prolongé que par des écritures et plafonné à 2 s : un
  lecteur continu ne doit pas pouvoir bloquer la régénération.
- Ignorer `BOARD.md` et les fichiers cachés (fichiers temporaires d'écriture atomique).

## Tests

- Unitaires dans chaque module (`cargo test`).
- `tests/e2e.rs` : binaire réel dans un dépôt git temporaire, avec
  `COUTCOUTICKET_HOME` isolé. Couvre le workflow et les hooks, le MCP stdio, et le
  démon HTTP (authentification, Origin, watcher sous lecture continue).
