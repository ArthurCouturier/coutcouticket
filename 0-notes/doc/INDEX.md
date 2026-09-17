# Index de la documentation technique

Point d'entrée obligatoire avant d'explorer le code. Une ligne par page.
La doc décrit l'**état actuel** du système ; l'historique des choix vit dans
les `decisions.md` des tickets.

| Page | Sujet | Lire quand… |
|------|-------|-------------|
<!-- Exemple : | [auth.md](auth.md) | Authentification, tokens | on touche au login, aux sessions ou aux droits | -->
| [architecture.md](architecture.md) | Modules, invariants, dépendances entre tickets, démon, watcher, setup-claude, appels git (échec vs hors dépôt), hooks git, .gitignore, plugin (skill, subagent relecteur, hook de session), tests | on modifie le code de coutcouticket, on ajoute un champ ou un outil, on touche aux dépendances (blocked_by), au démon (démarrage, plist launchd, journal), aux appels git, aux hooks git, à init, à setup-claude ou au plugin Claude Code |
| [distribution.md](distribution.md) | Workflows CI et release, install.sh, mise à jour du binaire et du démon | on publie une version, on touche à `.github/`, à `install.sh`, à `daemon install` ou au chemin du binaire |
