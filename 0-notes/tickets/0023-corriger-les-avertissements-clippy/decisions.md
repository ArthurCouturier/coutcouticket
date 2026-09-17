# Décisions — ticket 0023

<!-- Une entrée par décision, via ticket_decide / « coutcouticket decide ».
     Une décision remplacée n'est jamais supprimée : on ajoute une nouvelle entrée qui la cite. -->

## D1 — Clippy en CI sur la toolchain stable, sans allow (2026-09-17)

**Décision :** La CI lance cargo clippy --all-targets --locked -- -D warnings sur la toolchain stable non épinglée (composant clippy ajouté au profil minimal). Aucun #[allow] : les 6 avertissements sont corrigés (sort_by_key, let chains de l'édition 2024).

**Alternatives écartées :** Épingler une version de Rust : stabilise les lints mais demande des montées de version manuelles. Clippy en simple avertissement (sans -D warnings) : ne protège de rien. Masquer par #[allow] : contraire au ticket.

**Pourquoi :** Coût minimal et cohérent avec les tests déjà lancés sur stable. Compromis accepté : une nouvelle version de clippy peut ajouter un lint et faire échouer la CI sans changement de code ; on corrige alors le code. Les let chains exigent Rust 1.88, compatible avec rust-version = 1.89.
