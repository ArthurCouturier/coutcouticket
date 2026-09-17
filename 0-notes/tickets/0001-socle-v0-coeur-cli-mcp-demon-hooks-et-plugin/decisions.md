# Décisions — ticket 0001

<!-- Une entrée par décision, via ticket_decide / « coutcouticket decide ».
     Une décision remplacée n'est jamais supprimée : on ajoute une nouvelle entrée qui la cite. -->

## D1 — Tickets dans chaque projet, logique centralisée (2026-09-17)

**Décision :** coutcouticket contient la logique, le MCP, le démon et le plugin ; les tickets et la doc vivent dans le 0-notes/ de chaque projet.

**Alternatives écartées :** Un dépôt hub unique contenant tous les tickets.

**Pourquoi :** Les tickets restent au plus près du code, versionnés avec lui, lisibles par Claude Code sans outil ; l'outil reste unique et maintenu à un seul endroit.

## D2 — Rust plutôt que TypeScript (2026-09-17)

**Décision :** Binaire unique en Rust, SDK MCP officiel rmcp.

**Alternatives écartées :** TypeScript/Node, cohérent avec la stack habituelle.

**Pourquoi :** Démon permanent : quelques Mo de RAM contre plusieurs dizaines en Node ; hooks git appelés à chaque commit, démarrage en millisecondes ; aucun runtime à installer ; typage strict qui attrape les erreurs à la compilation.

## D3 — Séparation déterministe / jugement (2026-09-17)

**Décision :** Tout le mécanique (id, statuts, branches, dates, board, validation) est dans le binaire ; Claude rédige, décide et documente via le skill.

**Alternatives écartées :** Système entièrement piloté par des consignes dans un skill ou CLAUDE.md.

**Pourquoi :** Une règle écrite peut être oubliée par un LLM, une contrainte d'outil non. Statuts en enum dans le schéma MCP, commit refusé si non conforme.

## D4 — Démon HTTP local + stdio de secours (2026-09-17)

**Décision :** Démon en LaunchAgent exposant le MCP en Streamable HTTP sur 127.0.0.1 avec jeton Bearer et refus des requêtes navigateur ; mode stdio conservé en secours. Les fichiers restent la source de vérité.

**Alternatives écartées :** stdio uniquement, lancé par le client à chaque session.

**Pourquoi :** Surveillance des notes et régénération du board en continu, vue multi-projets, un seul process partagé. Le stdio évite qu'une panne du démon bloque le travail.

## D5 — Convention de branche stricte (2026-09-17)

**Décision :** Branche <type>/<id sur 4 chiffres>-<slug>, types feat, fix, refacto, design, ci ; exemptées main, master, develop, dev, stag, staging. Le suffixe est exactement le nom du dossier du ticket.

**Alternatives écartées :** Branches libres avec trailer recommandé ; identifiants non paddés.

**Pourquoi :** Correspondance branche/dossier sans conversion possible à rater ; 4 chiffres pour aller jusqu'à 9999 tickets ; trailer ajouté automatiquement donc traçabilité conservée après squash.

## D6 — Frontmatter parsé à la main (2026-09-17)

**Décision :** Sous-ensemble strict de YAML : clés connues uniquement, ordre fixe à l'écriture, clé inconnue refusée.

**Alternatives écartées :** Crate YAML générique.

**Pourquoi :** Aucune dépendance YAML (écosystème Rust instable sur ce point) ; toute dérive est détectée au lieu d'être tolérée silencieusement.

## D7 — Watcher : écritures seules, anti-rebond plafonné (2026-09-17)

**Décision :** Seuls les événements de création, modification de contenu et suppression déclenchent la régénération ; l'anti-rebond (300 ms) n'est prolongé que par des écritures et plafonné à 2 s.

**Alternatives écartées :** Anti-rebond relancé par tout événement.

**Pourquoi :** Deux bugs trouvés par les tests : les lectures du board relançaient la régénération en boucle, et un lecteur continu (éditeur, indexation, agent) empêchait toute régénération.
