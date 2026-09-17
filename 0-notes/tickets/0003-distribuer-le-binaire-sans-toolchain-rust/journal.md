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
