# Journal — ticket 0003

<!-- Journal append-only, alimenté par ticket_log / « coutcouticket log ».
     Ne jamais réécrire une entrée passée. Chaque entrée finit par « Prochaine étape ». -->

## 2026-09-17 15:25 — statut : todo → in-progress

Démarrage sur la branche `ci/0003-distribuer-le-binaire-sans-toolchain-rust`.

## 2026-09-17 15:30 — avancement

Livré : release.yml (tag v* → vérification tag = version Cargo.toml, cargo test, build aarch64 natif + x86_64 croisé sur macos-15, signature ad hoc, archives coutcouticket-<version>-<target>.tar.gz + .sha256 + SHA256SUMS, gh release create), ci.yml (cargo test sur push main/PR), install.sh (dernière version par redirection, SHA-256, xattr quarantaine, remplacement atomique, relance du démon via daemon install avec 3 essais), README (Installation, Mise à jour, publication), doc distribution.md. Vérifié : sh -n, YAML (ruby), install.sh contre une fausse release file:// (installation neuve, remplacement sur place, --dir + --no-daemon, somme fausse, version absente, option inconnue, absence de release sur GitHub) ; cargo test OK hors hooks_hors_du_path (environnemental). shellcheck et actionlint absents. daemon install relu : idempotent (load_or_create garde port et jeton, plist réécrit, bootout puis bootstrap) ; risque de course bootout/bootstrap compensé par des essais dans le script. Non vérifiable ici : exécution réelle du workflow.

**Prochaine étape :** Utilisateur : fusionner la branche sur main, puis « git tag v0.1.0 && git push origin v0.1.0 » ; vérifier que l'onglet Actions passe et que la release contient coutcouticket-0.1.0-aarch64-apple-darwin.tar.gz et sa .sha256 ; puis « curl -fsSL https://raw.githubusercontent.com/ArthurCouturier/coutcouticket/main/install.sh | sh » et « coutcouticket daemon status » ; cocher le critère 1 et passer le ticket en done.

## 2026-09-17 15:30 — statut : in-progress → review

À relire : .github/workflows/release.yml et ci.yml, install.sh, README. Critère 1 en attente du premier tag v0.1.0 poussé après fusion sur main (procédure dans le journal).

## 2026-09-17 16:12 — avancement

Premier tag `v0.1.0` poussé. Le workflow release (run 35231556877) passe : builds aarch64 et x86_64, release https://github.com/ArthurCouturier/coutcouticket/releases/tag/v0.1.0 avec les 2 archives, leurs `.sha256` et `SHA256SUMS`. La CI (run 35231209722) passe sur main.
`curl …/main/install.sh | sh -s -- --dir <tmp> --no-daemon` : téléchargement, somme vérifiée, binaire Mach-O arm64 installé, `--version` renvoie 0.1.0. Avertissements corrects pour le PATH, le binaire masqué et les hooks. Installation réelle non touchée.
Point mineur : GitHub signale que `actions/upload-artifact@v4` et `download-artifact@v4` visent Node.js 20, déprécié (exécutées de force sous Node 24).

**Prochaine étape :** Aucune (suite possible : passer les actions artifact à une version Node 24 dans release.yml)

## 2026-09-17 16:13 — statut : review → done

Release v0.1.0 publiée par le workflow (archives macOS arm64 et x86_64, avec leurs sommes). install.sh vérifié, et procédure de mise à jour documentée. Relecture de clôture : OK.
