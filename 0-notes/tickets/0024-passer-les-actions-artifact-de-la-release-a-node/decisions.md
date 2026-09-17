# Décisions — ticket 0024

<!-- Une entrée par décision, via ticket_decide / « coutcouticket decide ».
     Une décision remplacée n'est jamais supprimée : on ajoute une nouvelle entrée qui la cite. -->

## D1 — Versions des actions : upload-artifact@v7, download-artifact@v8, checkout et rust-cache inchangés (2026-09-17)

**Décision :** Monter actions/upload-artifact de v4 à v7 et actions/download-artifact de v4 à v8 (dernières majeures, runs.using node24, vérifié via gh api sur les tags v7, v7.0.1, v8, v8.0.1). Garder actions/checkout@v5 et Swatinem/rust-cache@v2 : ils déclarent déjà node24.

**Alternatives écartées :** upload@v6 / download@v7 : premières majeures en node24, mais déjà une majeure de retard et la paire v7/v8 est celle documentée ensemble (ESM, envoi direct). Monter checkout en v7 : sans effet sur l'avertissement Node 20, hors périmètre.

**Pourquoi :** upload v5 et download v6 sont encore en node20 par défaut. Changements incompatibles relus : download v8 échoue sur somme d'artefact invalide (digest-mismatch: error) et ne dézippe que les artefacts zippés ; upload v7 zippe toujours par défaut (archive: true) et name/merge-multiple sont inchangés, donc aucun paramètre du workflow à modifier. Runner minimal 2.327.1 : sans objet sur les runners hébergés GitHub.

## D2 — Essai de release par workflow_dispatch sans publication (2026-09-17)

**Décision :** release.yml accepte workflow_dispatch avec une entrée booléenne essai (défaut true). Un lancement manuel exécute tests, builds, archives, téléversement puis rassemblement des artefacts et vérification sha256sum -c, mais jamais la publication : l'étape gh release create est conditionnée à un push de tag v*. essai=false échoue avec un message explicite ; le contrôle tag/version est remplacé par une notice (version lue dans Cargo.toml).

**Alternatives écartées :** Tag de préversion (v0.1.1-rc1) : crée un tag et une release publique à nettoyer. Workflow d'essai séparé : duplique la chaîne de build et peut diverger. Publier une release brouillon en mode dispatch : laisse des brouillons à supprimer et exige contents: write pour rien.

**Pourquoi :** Valider la chaîne réelle (y compris download-artifact, qui n'est exercé que par le job release) sans créer de tag. Le job release garde contents: write même en essai car permissions n'accepte pas d'expression ; le risque est nul puisque l'étape de publication est sautée et que seuls les utilisateurs avec droit d'écriture peuvent lancer un dispatch.
