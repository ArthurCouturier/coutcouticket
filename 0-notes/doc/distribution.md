# Distribution et mise à jour

## Vue d'ensemble

| Élément | Fichier | Rôle |
|---|---|---|
| CI | `.github/workflows/ci.yml` | matrice `macos-15` + `windows-latest` : `cargo clippy --all-targets --locked -- -D warnings` (tout avertissement fait échouer), `cargo build`, `cargo test --locked --no-fail-fast` ; puis `sh -n install.sh` (macOS) ou essai réel d'`install.ps1` (Windows) ; sur push `main` et PR |
| Release | `.github/workflows/release.yml` | sur tag `v*` : tests, build, archives, GitHub Release ; lancement manuel = essai sans publication |
| Installation macOS | `install.sh` (racine) | télécharge, vérifie, installe, relance le démon |
| Installation Windows | `install.ps1` (racine) | idem sous Windows (PowerShell 5.1 et 7) |

Binaires publiés : macOS (aarch64, x86_64) et Windows x64. Ailleurs (Linux) :
`cargo install --git https://github.com/ArthurCouturier/coutcouticket`.

## Publier une version

1. Mettre `version` à jour dans `Cargo.toml` (et `Cargo.lock` via `cargo build`), commiter sur `main`.
2. `git tag vX.Y.Z && git push origin vX.Y.Z`.
3. Le workflow `release` :
   - vérifie que le tag vaut `v` + la version de `Cargo.toml` (échec explicite sinon) ;
   - sur `macos-15` (Apple Silicon) : `cargo test` puis build `aarch64-apple-darwin` ;
     `x86_64-apple-darwin` est compilé en croisé sur le même runner, sans tests ;
   - re-signe en ad hoc (`codesign --force --sign -`) et vérifie la signature ;
   - sur `windows-latest` (job `build-windows`, shell bash) : `cargo test` (tâche planifiée
     réellement installée, `COUTCOUTICKET_TEST_WINDOWS_SERVICE=1`), build
     `x86_64-pc-windows-msvc` avec `RUSTFLAGS=-C target-feature=+crt-static` (pas de
     dépendance à `vcruntime140.dll`), `--version`, puis
     `coutcouticket-<version>-x86_64-pc-windows-msvc.zip` (7z ; dossier : `coutcouticket.exe`
     + README) et `.zip.sha256` au format `shasum` (deux espaces, sans `*`). Version lue par
     `cargo pkgid` (pas de python3 garanti) ;
   - produit `coutcouticket-<version>-<target>.tar.gz` (dossier du même nom : binaire + README)
     et `<archive>.sha256` (format `shasum -a 256`), plus un `SHA256SUMS` global ;
   - téléverse les archives (`actions/upload-artifact@v7`, un artefact par cible), puis le job
     `release` les rassemble (`actions/download-artifact@v8`, `merge-multiple`), construit
     `SHA256SUMS` et le vérifie (`sha256sum -c`) ;
   - publie avec `gh release create --verify-tag --generate-notes` (étape réservée au push d'un tag `v*`).

Tag erroné : supprimer le tag local et distant, corriger, recréer. Un workflow échoué ne
publie rien (la release n'est créée qu'après tous les builds : `needs: [build, build-windows]`).

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

## install.ps1 (Windows)

```powershell
irm https://raw.githubusercontent.com/ArthurCouturier/coutcouticket/main/install.ps1 | iex
# avec paramètres :
& ([scriptblock]::Create((irm https://raw.githubusercontent.com/ArthurCouturier/coutcouticket/main/install.ps1))) -Version 0.2.0
```

Paramètres `-Version`, `-Dir`, `-NoDaemon` ; mêmes variables d'environnement qu'`install.sh`
(`COUTCOUTICKET_BASE_URL` accepte `file://`, via `Net.WebClient`).

Déroulé :
1. Dernière version : même redirection `releases/latest` (`Invoke-WebRequest -Method Head`).
2. Dossier : celui du `coutcouticket.exe` trouvé dans le `PATH`, sinon
   `%LOCALAPPDATA%\Programs\coutcouticket`. Aucun droit administrateur.
3. Téléchargement du `.zip` et de sa somme, vérification (`Get-FileHash`), extraction,
   `Unblock-File` (marque « Internet »), test `--version`.
4. L'ancien `coutcouticket.exe` est renommé en `.exe.old` (possible même s'il tourne),
   le nouveau copié à sa place ; `.old` est supprimé au passage suivant.
5. Dossier ajouté au `Path` utilisateur (registre, nouveaux terminaux) s'il n'y est pas.
6. Si la tâche `coutcouticket-daemon` existe : `daemon install` (3 essais), puis `daemon status`.
7. Avertissements : autre binaire qui masque le nouveau, hooks git pointant vers l'ancien emplacement.

Pièges : le fichier est en **UTF-8 avec BOM** (sans BOM, Windows PowerShell 5.1 le lit en
ANSI et casse les accents) ; erreurs par `throw`, jamais `exit` (avec `irm | iex`, `exit`
fermerait le terminal) ; commandes natives via `Invoke-Native` (sous 5.1 avec
`ErrorActionPreference=Stop`, une sortie sur stderr devient une erreur terminale). La CI
Windows l'exécute sous PowerShell 5.1 sur une archive locale : installation, mise à jour
démon en marche, `daemon status`, `daemon uninstall`.

## Mise à jour manuelle

```sh
curl -fsSL https://raw.githubusercontent.com/ArthurCouturier/coutcouticket/main/install.sh | sh
# ou, depuis les sources :
cargo install --path . && coutcouticket daemon install
coutcouticket daemon status   # doit afficher la nouvelle version
```

## Invariants et pièges

- **Chemins mémorisés** : les hooks git notent le chemin de `current_exe` (non canonicalisé ;
  sous Windows au format `C:/…/coutcouticket.exe`) au moment d'`init` ; le plist (macOS) ou
  la tâche planifiée (Windows) note `current_exe` **canonicalisé** au moment de
  `daemon install`. Remplacer le binaire au même chemin garde tout valide. D'où le choix du
  dossier par défaut. Changer de dossier : relancer `init` dans chaque projet et `daemon install`.
- **Le démon garde l'ancien binaire en mémoire** tant qu'il n'est pas relancé.
  `daemon install` le relance (`launchctl bootout` puis `bootstrap` ; Windows : arrêt de
  l'instance notée dans `daemon.pid`, puis `schtasks /Run`).
- **`daemon install` est idempotent** : `DaemonConfig::load_or_create` garde le port et le
  jeton (la config MCP de Claude Code reste valide), le plist est réécrit, le service rechargé.
  Piège : `bootstrap` juste après `bootout` peut échouer (« Bootstrap failed: 5 ») si l'arrêt
  n'est pas fini ; `install.sh` réessaie.
- **Jamais d'écriture sur place d'un binaire** : macOS garde en cache la signature par inode
  et tue (SIGKILL) un binaire signé modifié sur place. Toujours copier puis renommer.
  Windows refuse d'écrire un `.exe` en cours d'exécution mais permet de le renommer.
- **Fins de ligne** : `.gitattributes` (`* text=auto eol=lf`) ; sans lui, `actions/checkout`
  sous Windows (`core.autocrlf=true`) extrairait les gabarits `include_str!` en CRLF.
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
