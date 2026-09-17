# Décisions — ticket 0005

<!-- Une entrée par décision, via ticket_decide / « coutcouticket decide ».
     Une décision remplacée n'est jamais supprimée : on ajoute une nouvelle entrée qui la cite. -->

## D1 — Lecture seule du relecteur par disallowedTools et consigne, pas par motifs Bash (2026-09-17)

**Décision :** tools: Read, Grep, Glob, Bash ; disallowedTools: Write, Edit, NotebookEdit ; le prompt limite Bash à coutcouticket show/files/validate/list, git en lecture et commandes de lecture. Pas de champ model (hérite de la session).

**Alternatives écartées :** Motifs Bash(coutcouticket show:*) dans tools : prise en charge non documentée pour le champ tools des agents de plugin, risque de perdre Bash en silence. Pas de Bash : impossible de lire le diff ni de lancer validate. model: sonnet ou haiku : la relecture est une tâche de jugement, un modèle plus faible laisserait passer des critères non prouvés.

**Pourquoi :** Les agents de plugin ne peuvent pas fixer permissionMode ni hooks : les commandes Bash restent soumises aux permissions de l'utilisateur, ce qui borne le risque. La garantie d'écriture nulle vient des outils interdits ; la consigne couvre le reste.

## D2 — Relecture après le log final, avant status done (2026-09-17)

**Décision :** Dans Clôturer : cocher, doc, ticket_log final, relecture par coutcouticket:relecteur-cloture, puis status done. À CORRIGER : corriger, journaliser, relancer.

**Alternatives écartées :** Relire avant le log final : le relecteur ne pourrait pas juger l'entrée de clôture. Relire après done : trop tard, le board annonce déjà le ticket terminé.

**Pourquoi :** Le relecteur vérifie aussi que la dernière entrée du journal est prête ; il doit donc passer après elle, et avant le changement de statut qui est l'engagement visible.
