# Décisions — ticket 0008

<!-- Une entrée par décision, via ticket_decide / « coutcouticket decide ».
     Une décision remplacée n'est jamais supprimée : on ajoute une nouvelle entrée qui la cite. -->

## D1 — Notes déjà suivies par git (2026-09-17)

**Décision :** Si au moins un fichier du dossier de notes est suivi par git, `init` n'ajoute pas la règle au .gitignore. Il avertit que les notes sont versionnées et donne la marche à suivre pour les ignorer quand même : ajouter `/<notes_dir>/` au .gitignore, puis `git rm -r --cached <notes_dir>`. Sinon, la règle est ajoutée par défaut, sauf avec `--no-gitignore`.

**Alternatives écartées :** Ignorer quand même en avertissant : produit un état mixte (anciens tickets suivis, nouveaux ignorés) sur tout projet existant dès qu'on relance init.

**Pourquoi :** Choix de l'utilisateur. Relancer `init`, qui sert aussi de réparation, ne doit jamais changer à l'insu de l'utilisateur le fait que les notes d'un projet soient versionnées ou non.
