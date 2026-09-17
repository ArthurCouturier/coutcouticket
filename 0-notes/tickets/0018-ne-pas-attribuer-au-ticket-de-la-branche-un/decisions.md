# Décisions — ticket 0018

<!-- Une entrée par décision, via ticket_decide / « coutcouticket decide ».
     Une décision remplacée n'est jamais supprimée : on ajoute une nouvelle entrée qui la cite. -->

## D1 — Trailer d'un commit de notes d'autres tickets (2026-09-17)

**Décision :** prepare-commit-msg lit les fichiers indexés. Si tous sont sous <notes_dir>/, qu'au moins un est dans le dossier d'un ticket et qu'aucun n'est dans le dossier du ticket de la branche, le commit reçoit les trailers Ticket des tickets dont les dossiers sont touchés, et pas celui de la branche. Sinon (code touché, dossier du ticket de la branche touché, notes hors dossiers de ticket seules comme doc/ ou BOARD.md, index vide) : trailer de la branche, comme avant. Un message qui porte déjà un trailer Ticket n'est pas modifié.

**Alternatives écartées :** Ne mettre aucun trailer : le commit n'apparaît plus dans files des tickets réellement touchés. Toujours le trailer de la branche : c'est le bogue constaté. Trailers des tickets touchés en plus de la branche : files de la branche listerait encore les notes étrangères (le filtre les masquerait, mais l'historique resterait faux).

**Pourquoi :** L'attribution suit le contenu réel du commit, et files <id> des tickets touchés retrouve ces commits. Une mise à jour de doc/ seule sur une branche relève du ticket de la branche : on garde le trailer. Notes ignorées par git (0008) : jamais indexées, l'index ne contient que du code et la règle retombe sur le trailer de la branche. Les chemins git sont relatifs à la racine du dépôt : on retire le préfixe du projet (rev-parse --show-prefix) pour gérer un projet dans un sous-dossier.

## D2 — Fichiers exclus de files <id> (2026-09-17)

**Décision :** files <id> (commités et non commités) exclut les fichiers situés dans le dossier d'un autre ticket et BOARD.md. Il garde le dossier du ticket, doc/, les autres fichiers de 0-global/ et tout le code.

**Alternatives écartées :** Ne filtrer que les dossiers d'autres tickets : BOARD.md apparaît dans presque tous les commits (régénéré par pre-commit) sans rien dire du ticket. Filtrer tout <notes_dir>/ sauf le dossier du ticket : on perdrait les pages de doc mises à jour par le ticket, utiles au relecteur de clôture. Réécrire l'historique des commits mal attribués (21e60b7) : hors de question, le filtre suffit.

**Pourquoi :** Le filtre corrige aussi les commits déjà mal attribués. BOARD.md est un fichier généré, déterministe, sans information propre au ticket. Le filtre compare le nom de dossier (naming::parse_dir_name) à l'id du ticket, sans lire les autres tickets : un dossier de ticket supprimé depuis reste reconnu.
