# Décisions — ticket 0009

<!-- Une entrée par décision, via ticket_decide / « coutcouticket decide ».
     Une décision remplacée n'est jamais supprimée : on ajoute une nouvelle entrée qui la cite. -->

## D1 — Plist launchd sans bridage (ProcessType=Interactive) (2026-09-17)

**Décision :** Le LaunchAgent utilise ProcessType=Interactive et n'a plus ni Nice ni LowPriorityIO. RunAtLoad, KeepAlive et ThrottleInterval=10 inchangés.

**Alternatives écartées :** Garder Background (cause probable du délai d'environ 1 min après connexion). Standard ou ProcessType absent : le man launchd.plist indique qu'il applique encore un bridage léger du CPU et des E/S. Adaptive : suppose des transactions XPC, que le démon HTTP n'utilise pas. Garder Nice/LowPriorityIO seuls : ralentissent aussi le démarrage sous charge sans bénéfice.

**Pourquoi :** Le démon sert des appels MCP demandés par l'utilisateur, sensibles à la latence, et doit écouter dès l'ouverture de session. Il est événementiel (FSEvents, accept bloquant) : 0 CPU au repos mesuré (ps : 0:00.00 après 12 s), donc lever le bridage ne coûte rien au repos. Le man launchd.plist réserve Interactive aux jobs dont la réactivité en dépend, ce qui est le cas. Effet à confirmer après redémarrage réel (journal horodaté).

## D2 — Journal du démon horodaté sans nouvelle dépendance lourde (2026-09-17)

**Décision :** Macro log! dans daemon.rs : préfixe « AAAA-MM-JJ HH:MM:SS.mmm » (chrono, déjà présent). Au lancement, délai exec → run via proc_pidinfo(PROC_PIDTBSDINFO) : crate libc ajoutée en dépendance macOS uniquement (déjà dans Cargo.lock comme dépendance transitive). Ordre de run : config, bind, routeur, puis watcher ; ligne « surveillance prête » en fin d'init du watcher.

**Alternatives écartées :** Crate de journalisation (tracing-subscriber, log + env_logger) : dépendances plus lourdes pour trois lignes. Lancer ps -o lstart : résolution à la seconde et un processus de plus. Ne pas mesurer le délai exec → run : n'aurait pas distingué un exec lent (Gatekeeper, disque au boot) d'une init lente.

**Pourquoi :** Permet de mesurer, dans daemon.log seul, où passe le temps au démarrage après un redémarrage. Constat : l'ancien run écoutait déjà avant l'init du watcher (spawn_watcher ne faisait que lancer le thread) ; le bind est désormais explicitement en tête pour que cela reste vrai.
