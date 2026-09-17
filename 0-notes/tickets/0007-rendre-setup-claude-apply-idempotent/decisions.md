# Décisions — ticket 0007

<!-- Une entrée par décision, via ticket_decide / « coutcouticket decide ».
     Une décision remplacée n'est jamais supprimée : on ajoute une nouvelle entrée qui la cite. -->

## D1 — Remplacement automatique et lecture texte de claude mcp get (2026-09-17)

**Décision :** setup-claude --apply lit « claude mcp get coutcouticket » (sortie texte), ne fait rien si transport, URL et en-têtes sont identiques, sinon lance « claude mcp remove --scope <portée lue> » puis « claude mcp add » en portée user, et relit pour vérifier (boucle bornée à 6 étapes). Logique dans le nouveau module src/claude.rs ; binaire surchargeable par COUTCOUTICKET_CLAUDE_BIN.

**Alternatives écartées :** Se contenter d'afficher les deux commandes remove/add : l'utilisateur doit agir alors que l'état voulu est connu. Parser ~/.claude.json directement : format interne et non documenté, et ignore les portées local/project. Mettre la logique dans store.rs : store.rs porte les tickets, pas l'intégration Claude Code.

**Pourquoi :** claude mcp get/list n'ont pas d'option JSON (vérifié avec --help). La ligne « claude mcp remove <nom> -s <portée> » donne la portée de façon fiable, avec repli sur « Scope: ». Une portée local/project masque la portée user, d'où son retrait. Remplacer est sans perte : l'ancien enregistrement ne pointait plus vers le démon actuel. Risque accepté : un changement de format de Claude Code casse la lecture ; il est couvert par les tests unitaires et donne un message explicite.
