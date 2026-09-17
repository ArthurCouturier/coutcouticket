# Décisions — ticket 0025

<!-- Une entrée par décision, via ticket_decide / « coutcouticket decide ».
     Une décision remplacée n'est jamais supprimée : on ajoute une nouvelle entrée qui la cite. -->

## D1 — Démarrage automatique sous Windows (2026-09-17)

**Décision :** Tâche planifiée créée par « schtasks /Create /XML » (fichier UTF-16), nommée coutcouticket-daemon à la racine du planificateur : déclencheur LogonTrigger limité à l'utilisateur courant, jeton interactif sans élévation (LeastPrivilege), ExecutionTimeLimit PT0S, pas d'arrêt sur batterie, IgnoreNew, RestartOnFailure (1 min, 999 fois), priorité 5 (normale).

**Alternatives écartées :** Clé HKCU\Software\Microsoft\Windows\CurrentVersion\Run : pas de relance, démarrage retardé par l'Explorateur. « schtasks /SC ONLOGON » : déclencheur pour tout utilisateur, exige les droits administrateur. Service Windows : droits administrateur et session 0. Dossier Démarrage : pas de relance. Sous-dossier \coutcouticket\ du planificateur : sa création peut exiger les droits administrateur.

**Pourquoi :** Seule la tâche planifiée avec un déclencheur restreint à l'utilisateur démarre à l'ouverture de session, sans droits administrateur, avec relance et sans la limite de 72 h par défaut. La priorité par défaut (7) est inférieure à la normale : même piège que le bridage launchd du ticket 0009. Le XML expose ces réglages, pas les options de la ligne de commande schtasks.

## D2 — Pas de fenêtre console pour le démon Windows (2026-09-17)

**Décision :** L'action de la tâche lance « conhost.exe --headless <exe> daemon run --log-file <journal> » (conhost de %SystemRoot%\System32). Si conhost.exe est absent, le binaire est lancé directement et daemon install signale la fenêtre visible.

**Alternatives écartées :** #![windows_subsystem = "windows"] : impossible pour une CLI (cmd n'attend plus le processus, stdout perdu pour les hooks et le MCP stdio). Second binaire GUI : deux fichiers à distribuer et à garder alignés. Script VBS via wscript : VBScript est en cours de retrait. PowerShell -WindowStyle Hidden ou FreeConsole : fenêtre visible un instant à chaque ouverture de session. Tâche « exécuter même si l'utilisateur n'est pas connecté » : mot de passe stocké ou droits administrateur.

**Pourquoi :** conhost --headless (Windows 10 1809 et suivants) héberge la console sans l'afficher, sans dépendance ni second binaire. Non vérifiable visuellement en CI : à confirmer par l'utilisateur à l'ouverture de session.

## D3 — Journal, pid et arrêt du démon sous Windows (2026-09-17)

**Décision :** « daemon run --log-file F » (option cachée) écrit le journal dans F au lieu de stderr et note le pid dans daemon.pid à côté, une fois le port obtenu. daemon install et uninstall arrêtent l'instance en cours : schtasks /End, puis taskkill du pid noté si tasklist confirme qu'il s'agit de coutcouticket, puis attente de la libération du port (5 s max). Journal, pid et XML de la tâche dans %LOCALAPPDATA%\coutcouticket.

**Alternatives écartées :** Redirection par cmd /c : une fenêtre de plus et des guillemets fragiles. taskkill /IM coutcouticket.exe : tuerait aussi les hooks et serveurs MCP stdio des autres sessions. schtasks /End seul : ne garantit pas l'arrêt du processus lancé par conhost.

**Pourquoi :** La tâche planifiée ne redirige pas les sorties. Le pid n'est écrit qu'après le bind, pour qu'une instance refusée n'écrase pas celui de l'instance active. Une erreur fatale de run est aussi écrite au journal (« arrêt sur erreur »).

## D4 — Chemins et permissions sous Windows (2026-09-17)

**Décision :** Configuration globale dans %APPDATA%\coutcouticket (COUTCOUTICKET_HOME reste prioritaire). daemon.toml garde les ACL héritées du profil (utilisateur, SYSTEM, administrateurs) : pas d'équivalent de 0600 dans le code. Jeton tiré par getrandom (ProcessPrng sous Windows, /dev/urandom ailleurs). Chemins canoniques sans préfixe \\?\ (fsutil::canonicalize). Chemins relatifs affichés avec des / sur toutes les plateformes (Project::rel).

**Alternatives écartées :** Crates dirs ou dunce : dépendances de plus pour quelques lignes. ACL explicites via windows-sys : complexité sans gain, %APPDATA% étant déjà privé.

**Pourquoi :** getrandom était déjà dans Cargo.lock (via rand et tempfile). Le préfixe verbatim casse git -C, les hooks et la comparaison avec le registre. Les / gardent BOARD.md et les sorties CLI et MCP identiques entre plateformes et alignées sur git.

## D5 — Hooks git et fins de ligne sous Windows (2026-09-17)

**Décision :** Les hooks restent des scripts sh, exécutés par le sh de Git for Windows ; le binaire y est noté au format C:/…/coutcouticket.exe. .gitattributes impose eol=lf à tout le dépôt. Le parseur de frontmatter accepte les fins de ligne CRLF. Sous Windows, setup-claude cherche claude.exe, claude.cmd puis claude.bat dans le PATH.

**Alternatives écartées :** Format MSYS /c/Users/… : illisible hors de Git Bash, et MSYS accepte C:/…. Hooks .bat ou .ps1 : git for Windows exécute les hooks avec sh. Laisser autocrlf convertir : gabarits include_str! et hooks en CRLF.

**Pourquoi :** Git for Windows exécute toujours les hooks avec son sh, qui accepte C:/… et lance les .exe. Command::new("claude") ne cherche que claude.exe alors que l'installation npm fournit claude.cmd.

## D6 — Distribution Windows (2026-09-17)

**Décision :** Release : job build-windows (windows-latest) qui teste, compile x86_64-pc-windows-msvc avec le CRT statique et publie coutcouticket-<version>-x86_64-pc-windows-msvc.zip et .zip.sha256 (format shasum), inclus dans SHA256SUMS. install.ps1 (UTF-8 avec BOM, compatible Windows PowerShell 5.1) : télécharge, vérifie, renomme l'ancien .exe en .old, ajoute le dossier au PATH utilisateur, relance daemon install si la tâche existe.

**Alternatives écartées :** CRT dynamique : exige le redistribuable Visual C++. Archive .tar.gz : peu naturelle sous Windows. aarch64-pc-windows-msvc : pas de runner Windows ARM pour tester ; le binaire x64 tourne en émulation.

**Pourquoi :** Un .exe en cours d'exécution ne peut pas être remplacé mais peut être renommé : la mise à jour ne dépend pas de l'arrêt préalable du démon. Sans BOM, PowerShell 5.1 lit un .ps1 en ANSI et casse les accents.
