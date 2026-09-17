# Décisions — ticket 0011

<!-- Une entrée par décision, via ticket_decide / « coutcouticket decide ».
     Une décision remplacée n'est jamais supprimée : on ajoute une nouvelle entrée qui la cite. -->

## D1 — Module overview.rs et vue des seuls tickets ouverts (2026-09-17)

**Décision :** La vue globale vit dans src/overview.rs (build, rendus texte et Markdown, écriture d'OVERVIEW.md), appelée par la CLI (overview), le MCP (tickets_overview) et le démon. Elle ne montre que les tickets ouverts ; un filtre --status sur done/cancelled est refusé avec un message. Un projet illisible (introuvable, config invalide, tickets/ absent, notes avec problèmes) produit une entrée dans warnings, jamais une erreur.

**Alternatives écartées :** Tout dans store.rs : le cœur par projet y est déjà long, et la vue multi-projets ne touche pas à Project. Accepter done/cancelled : la vue deviendrait un list global, hors du besoin « quoi faire maintenant ».

**Pourquoi :** Le cœur reste unique (les façades n'appellent que overview::build) sans alourdir Project. Les tickets valides d'un projet partiellement invalide restent visibles, avec un avertissement qui renvoie vers validate.

## D2 — OVERVIEW.md généré par le démon (2026-09-17)

**Décision :** Oui : le démon écrit OVERVIEW.md dans le dossier de config globale (~/.config/coutcouticket/), au démarrage du watcher puis après chaque lot où un projet ou le registre a changé. Contenu déterministe (aucun horodatage), écrit seulement s'il change, liens absolus vers les ticket.md. La CLI overview n'écrit pas le fichier (lecture seule).

**Alternatives écartées :** Pas de fichier : moins de code, mais aucune vue globale lisible sans lancer une commande. Écriture par la CLI overview aussi : une commande de lecture qui écrit surprendrait.

**Pourquoi :** Le coût est faible (même build que la CLI) et le fichier sert de tableau de bord ouvrable dans un éditeur. Il vit hors de tout projet : pas de bruit git. Les écritures du démon dans le dossier global ne relancent rien (seul projects.toml y est suivi). Démon arrêté : le fichier peut être en retard, la CLI reste la vue en direct.

## D3 — Chemin canonique du registre dans le watcher (2026-09-17)

**Décision :** spawn_watcher canonicalise le dossier de config globale avant de comparer les chemins des événements à projects.toml.

**Alternatives écartées :** Comparer après canonicalisation de chaque chemin d'événement : un appel système par événement pour rien.

**Pourquoi :** FSEvents rapporte des chemins canoniques (/tmp → /private/tmp) : avec un COUTCOUTICKET_HOME ou un HOME derrière un lien symbolique, les changements du registre n'étaient jamais vus (bug préexistant, révélé par le test d'OVERVIEW.md). Changement minimal pour ne pas gêner le ticket 0025 qui touche daemon.rs.
