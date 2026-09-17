# Journal — ticket 0024

<!-- Journal append-only, alimenté par ticket_log / « coutcouticket log ».
     Ne jamais réécrire une entrée passée. Chaque entrée finit par « Prochaine étape ». -->

## 2026-09-17 16:18 — statut : todo → in-progress

Démarrage sur la branche `ci/0024-passer-les-actions-artifact-de-la-release-a-node`.

## 2026-09-17 16:20 — avancement

release.yml : upload-artifact v4→v7, download-artifact v4→v8 (runs.using node24 vérifié par gh api sur v7, v7.0.1, v8, v8.0.1). checkout@v5 et rust-cache@v2 déclarent déjà node24 : ci.yml inchangé. Ajout de workflow_dispatch (entrée essai, défaut true) : tout le build + téléversement + rassemblement des artefacts + sha256sum -c, publication réservée au push d'un tag v*. actionlint OK sur les deux workflows, cargo test OK. Doc distribution.md (section « Essayer la release sans tag », piège « Actions et Node ») et INDEX.md mis à jour. Critères non cochés : ils ne s'observent que sur un run GitHub (rien n'a été poussé). À faire par la session principale : git push -u origin ci/0024-passer-les-actions-artifact-de-la-release-a-node && gh workflow run release.yml --ref ci/0024-passer-les-actions-artifact-de-la-release-a-node -f essai=true ; vérifier que le run passe (jobs build x2 et release, étape « Publier la release GitHub » sautée, aucune release créée) et qu'aucune annotation Node 20 n'apparaît (gh run view <id>), puis cocher les deux critères. Attention : workflow_dispatch n'est lançable sur une branche que si release.yml avec workflow_dispatch existe aussi sur la branche par défaut ; sinon gh répond HTTP 404/422 et il faut d'abord fusionner sur main.

**Prochaine étape :** Pousser la branche, lancer gh workflow run release.yml --ref ci/0024-passer-les-actions-artifact-de-la-release-a-node -f essai=true, vérifier run vert sans annotation Node 20, cocher les critères et passer en done

## 2026-09-17 16:22 — statut : in-progress → review

Relecture de clôture faite : rien ne bloque la revue (étape de version renommée ensuite pour refléter le mode essai). Reste à vérifier sur GitHub : git push -u origin ci/0024-passer-les-actions-artifact-de-la-release-a-node puis gh workflow run release.yml --ref ci/0024-passer-les-actions-artifact-de-la-release-a-node -f essai=true (si GitHub refuse faute de workflow_dispatch sur main : fusionner d'abord puis --ref main). Attendu : run vert, étape « Publier la release GitHub » sautée, aucune release créée, aucune annotation Node 20 (gh run view <id>). Ensuite cocher les deux critères, journaliser l'id du run avec next_step Aucune, passer en done. Au merge : conflit probable sur le tableau de 0-notes/doc/distribution.md (garder la ligne CI de main et la ligne Release de 0024).

## 2026-09-17 16:26 — avancement

Fusionné sur main et poussé. Essai lancé avec `gh workflow run release.yml --ref main -f essai=true` (run 35233148603) : vert.
- Jobs `build (aarch64-apple-darwin)`, `build (x86_64-apple-darwin)` et `release` réussis.
- « Assembler et vérifier les sommes SHA-256 » réussi, « Publier la release GitHub » sauté (skipped).
- Aucune nouvelle release : v0.1.0 reste la dernière.
- Seules annotations : les notices « Essai sur main… ». Plus d'avertissement Node 20.
La CI de main (run 35233139661, avec clippy) est verte aussi.

**Prochaine étape :** Aucune

## 2026-09-17 16:27 — statut : review → done

Actions artifact passées à Node 24 (upload v7, download v8). L'essai de release manuel, sans publication, passe (run 35233148603). Relecture de clôture : OK.
