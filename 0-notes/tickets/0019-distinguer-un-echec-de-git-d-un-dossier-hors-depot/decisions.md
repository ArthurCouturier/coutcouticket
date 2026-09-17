# Décisions — ticket 0019

<!-- Une entrée par décision, via ticket_decide / « coutcouticket decide ».
     Une décision remplacée n'est jamais supprimée : on ajoute une nouvelle entrée qui la cite. -->

## D1 — Contexte : erreur git exposée, pas propagée (2026-09-17)

**Décision :** context (show, ticket_context) ne échoue pas quand git échoue : current_branch à null et nouveau champ git_error (toujours sérialisé, null si git va bien). files et start, eux, échouent avec le message de git.

**Alternatives écartées :** Faire échouer ticket_context entier ; ne rien ajouter et journaliser seulement ; champ absent quand vide (skip_serializing_if).

**Pourquoi :** La reprise d'un ticket (chemins, prochaine étape) reste utile sans git ; un champ toujours présent se découvre dans le schéma et distingue null (hors dépôt ou HEAD détaché) d'une panne.

## D2 — Détection du cas « pas un dépôt » (2026-09-17)

**Décision :** is_repo renvoie Result<bool> : faux seulement si git rev-parse sort en 128 avec « not a git repository », lancé avec LC_ALL=C. Tout autre échec (code 69 Xcode, dubious ownership, binaire absent) est une erreur. init avertit et saute hooks et .gitignore au lieu d'échouer.

**Alternatives écartées :** Se fier au seul code 128 (aussi renvoyé pour dubious ownership, -C invalide) ; détecter .git soi-même (faux pour worktrees, GIT_DIR) ; faire échouer init.

**Pourquoi :** Le texte est stable en locale C ; init doit rester utilisable pour créer les notes, et sans savoir si les notes sont suivies il ne faut pas toucher au .gitignore.
