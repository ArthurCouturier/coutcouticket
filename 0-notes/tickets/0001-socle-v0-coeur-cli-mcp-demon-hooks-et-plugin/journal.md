# Journal — ticket 0001

<!-- Journal append-only, alimenté par ticket_log / « coutcouticket log ».
     Ne jamais réécrire une entrée passée. Chaque entrée finit par « Prochaine étape ». -->

## 2026-09-17 10:13 — avancement

Socle complet construit et testé sous Linux : 10 tests unitaires, 3 tests de bout en bout (workflow git et hooks, MCP stdio, démon HTTP avec authentification et watcher), stables sur 5 exécutions. Mesures sur 200 tickets : démon 6,3 Mo RAM et 0 CPU au repos, validate 6 ms, pre-commit 10 ms, binaire release 4,9 Mo.

**Prochaine étape :** Valider sur macOS (ticket 0002) avant de passer en done.

## 2026-09-17 10:13 — statut : todo → review

Code complet ; relecture et validation macOS attendues (0002).

## 2026-09-17 15:17 — avancement

Validation macOS terminée (0002 clos), comme l'exigeait l'entrée précédente. Les 6 critères sont cochés. Correctifs issus de la validation : 0006 (fait), 0007 et 0009 (à faire).

**Prochaine étape :** Aucune

## 2026-09-17 15:17 — statut : review → done

Socle v0 livré et validé sous Linux puis sur macOS (0002).
