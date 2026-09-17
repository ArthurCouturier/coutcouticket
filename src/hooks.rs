//! Hooks : git (pre-commit, prepare-commit-msg) et Claude Code (SessionStart).

use std::io::Read;
use std::path::{Path, PathBuf};

use anyhow::{Result, bail};

use crate::config::{Registry, find_root};
use crate::git;
use crate::model::Status;
use crate::naming::{self, BranchKind};
use crate::store::{Project, last_next_step, JOURNAL_FILE};

fn open_here() -> Result<Option<Project>> {
    let cwd = std::env::current_dir()?;
    if find_root(&cwd).is_err() {
        return Ok(None);
    }
    Ok(Some(Project::open(&cwd)?))
}

/// pre-commit : convention de branche stricte, ticket existant, notes valides.
pub fn pre_commit() -> Result<()> {
    let Some(project) = open_here()? else {
        eprintln!("coutcouticket : aucun .coutcouticket.toml trouvé, vérifications ignorées");
        return Ok(());
    };
    if let Some(branch) = git::current_branch(&project.root)?
        && let BranchKind::Ticket { kind, id, dir } = naming::classify_branch(&branch, &project.cfg)?
    {
        let ticket = project.find(id).map_err(|e| {
            anyhow::anyhow!("branche « {branch} » : {e:#}. Créer le ticket d'abord (ticket_create) puis utiliser ticket_start.")
        })?;
        if ticket.dir_name != dir {
            bail!(
                "branche « {branch} » : le dossier du ticket {} est « {} ». Branche attendue : {}",
                id.format(project.cfg.id_width),
                ticket.dir_name,
                naming::branch_name(&ticket.doc.front.kind, &ticket.dir_name)
            );
        }
        if ticket.doc.front.kind != kind {
            bail!(
                "branche « {branch} » : le ticket est de type « {} », branche attendue : {}",
                ticket.doc.front.kind,
                naming::branch_name(&ticket.doc.front.kind, &ticket.dir_name)
            );
        }
    }
    if project.regenerate_board()? {
        // Notes hors dépôt (.gitignore) : git add échouerait et bloquerait le commit.
        if git::is_ignored(&project.root, &project.board_path())? {
            eprintln!("coutcouticket : BOARD.md régénéré (notes ignorées par git, non ajouté au commit)");
        } else {
            git::add(&project.root, &project.board_path())?;
            eprintln!("coutcouticket : BOARD.md régénéré et ajouté au commit");
        }
    }
    let problems = project.validate()?;
    if !problems.is_empty() {
        let list: Vec<String> = problems.iter().map(|p| format!("  - {} : {}", p.path, p.message)).collect();
        bail!("notes invalides, commit refusé :\n{}", list.join("\n"));
    }
    Ok(())
}

/// prepare-commit-msg : ajoute le trailer `Ticket: <id>` depuis le nom de branche,
/// ou ceux des tickets touchés si le commit ne contient que leurs notes.
pub fn prepare_commit_msg(msg_file: &Path, source: Option<&str>) -> Result<()> {
    if source == Some("merge") {
        return Ok(());
    }
    let Some(project) = open_here()? else { return Ok(()) };
    let Some(branch) = git::current_branch(&project.root)? else { return Ok(()) };
    if let BranchKind::Ticket { id, .. } = naming::classify_branch(&branch, &project.cfg)? {
        let msg_path: PathBuf = if msg_file.is_absolute() { msg_file.to_path_buf() } else { std::env::current_dir()?.join(msg_file) };
        let staged = git::staged_files(&project.root)?;
        let prefix = git::show_prefix(&project.root)?;
        let ids: Vec<String> =
            project.commit_tickets(id, &staged, &prefix).iter().map(|t| t.format(project.cfg.id_width)).collect();
        git::add_trailers(&project.root, &msg_path, &ids)?;
    }
    Ok(())
}

/// SessionStart (Claude Code) : injecte le contexte du projet et du ticket courant.
pub fn session_start() -> Result<()> {
    let mut input = String::new();
    let _ = std::io::stdin().read_to_string(&mut input);
    let cwd = serde_json::from_str::<serde_json::Value>(&input)
        .ok()
        .and_then(|v| v.get("cwd").and_then(|c| c.as_str()).map(PathBuf::from))
        .map(Ok)
        .unwrap_or_else(std::env::current_dir)?;
    let Ok(root) = find_root(&cwd) else { return Ok(()) };
    let project = Project::open(&root)?;
    let text = session_context(&project)?;
    let out = serde_json::json!({
        "hookSpecificOutput": { "hookEventName": "SessionStart", "additionalContext": text }
    });
    println!("{out}");
    Ok(())
}

pub fn session_context(project: &Project) -> Result<String> {
    let scan = project.scan()?;
    let count = |s: Status| scan.tickets.iter().filter(|t| t.doc.front.status == s).count();
    let mut lines = vec![
        format!("[coutcouticket] Projet : {} (notes dans {}/)", project.root.display(), project.cfg.notes_dir),
        format!("Paramètre « project » à passer aux outils MCP ticket_* : {}", project.root.display()),
        format!(
            "Tickets : {} en cours, {} en revue, {} bloqué(s), {} à faire. Vue complète : {}/0-global/BOARD.md",
            count(Status::InProgress),
            count(Status::Review),
            count(Status::Blocked),
            count(Status::Todo),
            project.cfg.notes_dir
        ),
    ];
    if !scan.problems.is_empty() {
        lines.push(format!("⚠️ {} problème(s) dans les notes : lancer notes_validate.", scan.problems.len()));
    }
    match git::current_branch_if_repo(&project.root) {
        Err(e) => lines.push(format!("⚠️ git en échec, branche courante inconnue : {e:#}")),
        Ok(None) => {}
        Ok(Some(branch)) => {
            match naming::classify_branch(&branch, &project.cfg) {
                Ok(BranchKind::Ticket { id, .. }) => match scan.tickets.iter().find(|t| t.doc.front.id == id) {
                    Some(t) => {
                        let journal = std::fs::read_to_string(t.path.join(JOURNAL_FILE)).unwrap_or_default();
                        lines.push(format!(
                            "Branche courante {branch} → ticket {} « {} » ({}).",
                            id.format(project.cfg.id_width),
                            t.doc.front.title,
                            t.doc.front.status
                        ));
                        let blockers = project.open_blockers(t, &scan.tickets);
                        if !blockers.is_empty() {
                            let ids: Vec<String> = blockers.iter().map(|b| b.format(project.cfg.id_width)).collect();
                            lines.push(format!("⚠️ Ticket bloqué par des tickets encore ouverts : {}.", ids.join(", ")));
                        }
                        if let Some(next) = last_next_step(&journal) {
                            lines.push(format!("Prochaine étape notée : {next}"));
                        }
                    }
                    None => lines.push(format!("⚠️ Branche {branch} : ticket {} introuvable.", id.format(project.cfg.id_width))),
                },
                Ok(BranchKind::Exempt) => lines.push(format!("Branche courante : {branch} (hors ticket).")),
                Err(e) => lines.push(format!("⚠️ {e:#}")),
            }
        }
    }
    if !Registry::load().map(|r| r.contains(&project.root)).unwrap_or(false) {
        lines.push("⚠️ Projet non enregistré auprès du démon : lancer « coutcouticket init » ou « coutcouticket projects add ».".into());
    }
    lines.push("Pour toute action sur un ticket, suivre le skill « ticket ».".into());
    Ok(lines.join("\n"))
}
