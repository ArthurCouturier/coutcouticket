# Distribution et mise à jour

## Vue d'ensemble

| Élément | Fichier | Rôle |
|---|---|---|
| CI | `.github/workflows/ci.yml` | `cargo clippy --all-targets --locked -- -D warnings` (tout avertissement fait échouer) + `cargo test --locked` + `sh -n install.sh` sur push `main` et PR (macOS) |
| Release | `.github/workflows/release.yml` | sur tag `v*` : tests, build, archives, GitHub Release ; lancement manuel = essai sans publication |
| Installation | `install.sh` (racine) | télécharge, vérifie, installe, relance le démon |

Seul macOS est publié en binaire. Ailleurs : `cargo install --git https://github.com/ArthurCouturier/coutcouticket`.

## Publier une version

1. Mettre `version` à jour dans `Cargo.toml` (et `Cargo.lock` via `cargo build`), commiter sur `main`.
2. `git tag vX.Y.Z && git push origin vX.Y.Z`.
3. Le workflow `release` :
   - vérifie que le tag vaut `v` + la version de `Cargo.toml` (échec explicite sinon) ;
   - sur `macos-15` (Apple Silicon) : `cargo test` puis build `aarch64-apple-darwin` ;
     `x86_64-apple-darwin` est compilé en croisé sur le même runner, sans tests ;
   - re-signe en ad hoc (`codesign --force --sign -`) et vérifie la signature ;
   - produit `coutcouticket-<version>-<target>.tar.gz` (dossier du même nom : binaire + README)
     et `<archive>.sha256` (format `shasum -a 256`), plus un `SHA256SUMS` global ;
   - téléverse les archives (`actions/upload-artifact@v7`, un artefact par cible), puis le job
     `release` les rassemble (`actions/download-artifact@v8`, `merge-multiple`), construit
     `SHA256SUMS` et le vérifie (`sha256sum -c`) ;
   - publie avec `gh release create --verify-tag --generate-notes` (étape réservée au push d'un tag `v*`).

Tag erroné : supprimer le tag local et distant, corriger, recréer. Un workflow échoué ne
publie rien (la release n'est créée qu'après les deux builds).

## Essayer la release sans tag

```sh
gh workflow run release.yml --ref <branche> -f essai=true
gh run watch   # ou : gh run list --workflow release.yml
```

Le lancement manuel (`workflow_dispatch`) exécute tout sauf la publication : tests, builds,
signature, archives, artefacts téléversés puis rassemblés et vérifiés. La version vient de
`Cargo.toml` (pas de contrôle de tag). `-f essai=false` échoue avec un message explicite :
seul un push de tag publie. Les archives restent téléchargeables dans les artefacts du run.

## install.sh

Options : `--version X.Y.Z` (défaut : dernière release), `--dir DOSSIER`, `--no-daemon`.
Variables équivalentes : `COUTCOUTICKET_VERSION`, `COUTCOUTICKET_INSTALL_DIR`,
`COUTCOUTICKET_NO_DAEMON=1`. `COUTCOUTICKET_BASE_URL` remplace la source des archives
(`file://` accepté : c'est ainsi qu'on le teste sans release).

Déroulé :
1. Dernière version : redirection de `github.com/<repo>/releases/latest` vers `/tag/vX.Y.Z`
   (pas d'API, pas de quota).
2. Dossier : celui du `coutcouticket` trouvé dans le `PATH`, sinon `~/.local/bin`.
3. Téléchargement de l'archive et de sa somme, vérification SHA-256, extraction,
   suppression de `com.apple.quarantine`, test `--version`.
4. Copie à côté de la cible puis `mv` (remplacement atomique).
5. Si `~/Library/LaunchAgents/app.coutcouticket.daemon.plist` existe : `daemon install`
   (3 essais), puis attente de `daemon status` (10 s max).
6. Avertissements : dossier hors `PATH`, autre binaire qui masque le nouveau, hooks git
   pointant vers l'ancien emplacement.

## Mise à jour manuelle

```sh
curl -fsSL https://raw.githubusercontent.com/ArthurCouturier/coutcouticket/main/install.sh | sh
# ou, depuis les sources :
cargo install --path . && coutcouticket daemon install
coutcouticket daemon status   # doit afficher la nouvelle version
```

## Invariants et pièges

- **Chemins mémorisés** : les hooks git notent le chemin de `current_exe` (non canonicalisé)
  au moment d'`init` ; le plist note `current_exe` **canonicalisé** au moment de
  `daemon install`. Remplacer le binaire au même chemin garde tout valide. D'où le choix du
  dossier par défaut. Changer de dossier : relancer `init` dans chaque projet et `daemon install`.
- **Le démon garde l'ancien binaire en mémoire** tant qu'il n'est pas relancé.
  `daemon install` le relance (`launchctl bootout` puis `bootstrap`).
- **`daemon install` est idempotent** : `DaemonConfig::load_or_create` garde le port et le
  jeton (la config MCP de Claude Code reste valide), le plist est réécrit, le service rechargé.
  Piège : `bootstrap` juste après `bootout` peut échouer (« Bootstrap failed: 5 ») si l'arrêt
  n'est pas fini ; `install.sh` réessaie.
- **Jamais d'écriture sur place d'un binaire** : macOS garde en cache la signature par inode
  et tue (SIGKILL) un binaire signé modifié sur place. Toujours copier puis renommer.
- **Signature** : un binaire arm64 doit être signé (ad hoc suffit). Pas de notarisation :
  `curl` ne pose pas d'attribut de quarantaine ; une archive venue d'un navigateur en a un,
  retiré par `install.sh` (sinon : `xattr -d com.apple.quarantine coutcouticket`).
- **Actions et Node** : toutes les actions des workflows tournent sous Node 24
  (`runs.using: node24`). Vérifier avant de monter une action :
  `gh api "repos/<owner>/<action>/contents/action.yml?ref=<tag>" --jq .content | base64 -d | grep using`.
  `upload-artifact` et `download-artifact` se montent ensemble (v7/v8) : `download-artifact@v8`
  échoue sur une somme d'artefact invalide (`digest-mismatch: error` par défaut) et ne
  décompresse que les artefacts zippés (`upload-artifact` zippe par défaut, `archive: true`).
- **Tag et version** : `coutcouticket --version` et `/health` affichent `CARGO_PKG_VERSION` ;
  le workflow refuse un tag qui ne correspond pas.
