# Décisions — ticket 0003

<!-- Une entrée par décision, via ticket_decide / « coutcouticket decide ».
     Une décision remplacée n'est jamais supprimée : on ajoute une nouvelle entrée qui la cite. -->

## D1 — Cibles et runners de release (2026-09-17)

**Décision :** Un seul runner macos-15 (Apple Silicon) : aarch64-apple-darwin natif avec cargo test complet, x86_64-apple-darwin compilé en croisé sur le même runner, sans tests (--version exécuté seulement si Rosetta est présente). Publication par un job ubuntu avec gh release create --verify-tag une fois les deux builds réussis.

**Alternatives écartées :** arm64 seul : écarte les Mac Intel alors que la compilation croisée ne coûte presque rien. Runner Intel natif (macos-13 retiré, macos-15-intel en fin de vie) : dépendance à une image vouée à disparaître. Binaire universel (lipo) : archive deux fois plus lourde pour chaque utilisateur. Actions tierces de release : gh suffit.

**Pourquoi :** Pas de dépendance C hors frameworks système : la compilation croisée est fiable. Le code testé est le même pour les deux architectures ; le risque résiduel (binaire x86_64 jamais exécuté en CI) est accepté.

## D2 — Workflow CI séparé (2026-09-17)

**Décision :** Ajouter ci.yml : cargo test --locked et sh -n install.sh sur push main et sur chaque PR, runner macos-15, cache Swatinem/rust-cache.

**Alternatives écartées :** Tests uniquement au tag : une régression n'est vue qu'au moment de publier. Runner Linux : moins cher mais ne couvre ni FSEvents ni le comportement macOS réel.

**Pourquoi :** Dépôt public : minutes macOS gratuites. Le projet cible macOS ; le tag de release ne doit pas être le premier endroit où les tests tournent en CI.

## D3 — Dossier d'installation du script (2026-09-17)

**Décision :** install.sh remplace sur place le coutcouticket trouvé dans le PATH (même s'il vient de cargo install) ; sinon ~/.local/bin. --dir / COUTCOUTICKET_INSTALL_DIR pour forcer. Remplacement atomique (copie puis mv). Aucun sudo.

**Alternatives écartées :** Toujours ~/.local/bin : laisse l'ancien binaire en place, qui masque le nouveau dans le PATH et reste référencé par les hooks git. /usr/local/bin : exige sudo. Homebrew : un tap à maintenir, reporté.

**Pourquoi :** Les hooks git mémorisent le chemin de current_exe lors d'init et le plist du démon celui de daemon install : garder le même chemin évite de relancer init dans chaque projet. Le script avertit si le dossier change, s'il n'est pas dans le PATH ou si un autre binaire le masque.

## D4 — Pas de commande self-update pour l'instant (2026-09-17)

**Décision :** Ne pas implémenter coutcouticket self-update dans ce ticket ; la mise à jour passe par install.sh. Idée de suite : self-update qui délègue à curl + install.sh avec --dir = dossier de current_exe.

**Alternatives écartées :** Client HTTP embarqué (reqwest + rustls) : plusieurs Mo et des dizaines de dépendances, contraire à la sobriété du binaire. Délégation à curl immédiate : exécute un script distant depuis le binaire, sans test possible avant la première release.

**Pourquoi :** Le script couvre déjà le besoin (une ligne) ; une commande intégrée se justifie une fois le circuit de release éprouvé.
