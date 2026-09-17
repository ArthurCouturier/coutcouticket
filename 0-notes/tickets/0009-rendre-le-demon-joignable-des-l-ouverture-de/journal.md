# Journal — ticket 0009

<!-- Journal append-only, alimenté par ticket_log / « coutcouticket log ».
     Ne jamais réécrire une entrée passée. Chaque entrée finit par « Prochaine étape ». -->

## 2026-09-17 15:25 — statut : todo → in-progress

Démarrage sur la branche `fix/0009-rendre-le-demon-joignable-des-l-ouverture-de`.

## 2026-09-17 15:30 — avancement

Démarrage réordonné (bind avant tout, watcher ensuite), journal horodaté (lancement avec délai exec→run, écoute, surveillance prête), plist en ProcessType=Interactive sans Nice ni LowPriorityIO. Constat : l'ancien run écoutait déjà avant l'init du watcher, donc le bridage launchd reste la cause retenue. Mesure locale (binaire release, COUTCOUTICKET_HOME temporaire, 1 projet) : port ouvert 5-6 ms après le spawn (exec→run 4 ms, run→écoute 0 ms, watcher prêt à 2-3 ms) ; sous taskpolicy -b (bridage background simulé, machine peu chargée) 20 ms. 0 CPU au repos (ps 0:00.00 après 12 s). Tests : plist_is_not_throttled (unitaire), daemon_ecoute_avant_le_watcher (e2e) ; cargo test passe hors hooks_hors_du_path (échec environnemental connu).

**Prochaine étape :** Vérification utilisateur après installation et redémarrage (voir entrée suivante)

## 2026-09-17 15:30 — avancement

Reste à vérifier par l'utilisateur (critères 2 et 3). 1) Réinstaller depuis la branche fusionnée : « cargo install --path . » puis « coutcouticket daemon install » (réécrit le plist). Contrôle : « grep -A1 ProcessType ~/Library/LaunchAgents/app.coutcouticket.daemon.plist » affiche Interactive, et « grep -c -E 'Nice|LowPriorityIO' » sur le même fichier donne 0. 2) Redémarrer macOS, se connecter et ouvrir aussitôt une session Claude Code : « /mcp » doit montrer coutcouticket connecté et les outils ticket_* disponibles sans reconnexion. 3) Mesures : « sysctl -n kern.boottime » ; « grep -E 'lancement du démon|à l.écoute|surveillance prête' ~/Library/Logs/coutcouticket/daemon.log | tail -3 » : l'écart entre les horodatages « lancement » et « à l'écoute » doit être < 5 s, et « processus lancé il y a N ms » doit rester petit (sinon le délai est avant run : exec, disque, Gatekeeper) ; « ps -o lstart=,time=,%cpu= -p 3795 ». 4) CPU au repos : relancer la commande ps une minute plus tard, TIME doit rester à 0:00.0x. 5) Si le délai persiste : « log show --last boot --predicate 'eventMessage CONTAINS "app.coutcouticket.daemon"' | head -50 » pour voir quand launchd a réellement lancé le job.

**Prochaine étape :** L'utilisateur applique la procédure ci-dessus, puis coche les critères 2 et 3 et passe le ticket en done (ou rouvre avec les horodatages relevés)

## 2026-09-17 15:30 — statut : in-progress → review

Code, plist, journal horodaté et tests faits ; à valider par l'utilisateur après réinstallation et redémarrage macOS (procédure dans le journal).
