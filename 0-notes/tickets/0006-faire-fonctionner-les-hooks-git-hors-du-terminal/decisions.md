# Décisions — ticket 0006

<!-- Une entrée par décision, via ticket_decide / « coutcouticket decide ».
     Une décision remplacée n'est jamais supprimée : on ajoute une nouvelle entrée qui la cite. -->

## D1 — Localisation du binaire par les hooks git (2026-09-17)

**Décision :** `init` écrit dans chaque hook le chemin absolu du binaire qui l'exécute (`current_exe`, non canonicalisé, cité pour sh). Le hook l'utilise s'il est exécutable, sinon il se rabat sur `command -v coutcouticket`, et sinon il échoue avec un message qui indique de relancer `init`. Relancer `init` réécrit les hooks marqués dont le contenu diffère.

**Alternatives écartées :** Recherche dans le PATH seule (état précédent) : échoue depuis un client graphique. Compléter le PATH dans le hook avec ~/.cargo/bin et /opt/homebrew/bin : liste figée qui manque les autres installations. Sourcer le profil du shell (~/.zprofile) : lent, dépend du shell, effets de bord. `launchctl config user path` : réglage global du système, demande sudo et un redémarrage. Canonicaliser le chemin : casse à chaque mise à jour Homebrew (chemin versionné du Cellar).

**Pourquoi :** Le chemin connu au moment de `init` est le seul qui fonctionne quel que soit le PATH hérité. Le repli sur le PATH couvre un binaire déplacé ou réinstallé ailleurs sans relancer `init`. Compromis accepté : déplacer le binaire hors du PATH impose de relancer `init`, et le message d'erreur le dit.
