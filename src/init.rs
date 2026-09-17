//! `coutcouticket init` : crée ou répare l'arborescence d'un projet.
//! Idempotent : ne réécrit jamais un contenu utilisateur, complète ce qui manque.

use std::fs;
use std::path::Path;

use anyhow::{Context, Result, bail};
use serde::Serialize;

use crate::config::{CONFIG_FILE, Config, Registry};
use crate::fsutil;
use crate::git;
use crate::naming;
use crate::store::{DECISIONS_FILE, JOURNAL_FILE, Project, TICKET_FILE};
use crate::templates;

pub const HOOK_MARKER: &str = "# coutcouticket-hook";
pub const GIT_HOOKS: [&str; 2] = ["pre-commit", "prepare-commit-msg"];

#[derive(Debug, Clone, Copy)]
pub struct InitOptions {
    pub git_hooks: bool,
    pub claude_md: bool,
    pub register: bool,
}

impl Default for InitOptions {
    fn default() -> Self {
        InitOptions { git_hooks: true, claude_md: true, register: true }
    }
}

#[derive(Debug, Default, Serialize)]
pub struct InitReport {
    pub root: String,
    pub created: Vec<String>,
    pub updated: Vec<String>,
    pub warnings: Vec<String>,
}

impl InitReport {
    pub fn render(&self) -> String {
        let mut out = format!("Projet : {}\n", self.root);
        if self.created.is_empty() && self.updated.is_empty() {
            out.push_str("Arborescence déjà complète, rien à faire.\n");
        }
        for c in &self.created {
            out.push_str(&format!("  + créé      {c}\n"));
        }
        for u in &self.updated {
            out.push_str(&format!("  ~ mis à jour {u}\n"));
        }
        for w in &self.warnings {
            out.push_str(&format!("  ! attention {w}\n"));
        }
        out
    }
}

fn hook_script(name: &str) -> String {
    let call = match name {
        "prepare-commit-msg" => "exec coutcouticket hook prepare-commit-msg \"$@\"",
        _ => "exec coutcouticket hook pre-commit",
    };
    format!(
        "#!/bin/sh\n{HOOK_MARKER} (installé par « coutcouticket init », ne pas éditer)\n\
         if ! command -v coutcouticket >/dev/null 2>&1; then\n\
         \x20 echo \"coutcouticket introuvable dans le PATH. Installer le binaire (voir README) ou contourner ponctuellement avec --no-verify.\" >&2\n\
         \x20 exit 1\n\
         fi\n\
         {call}\n"
    )
}

pub fn init(path: &Path, opts: InitOptions) -> Result<InitReport> {
    if !path.is_dir() {
        bail!("{} n'est pas un dossier", path.display());
    }
    let root = path.canonicalize()?;
    let mut report = InitReport { root: root.display().to_string(), ..Default::default() };

    // 1. Configuration
    let cfg_path = root.join(CONFIG_FILE);
    if fsutil::create_if_missing(&cfg_path, &Config::default_file_content())? {
        report.created.push(CONFIG_FILE.into());
    }
    let cfg = Config::load(&root)?;
    let project = Project { root: root.clone(), cfg };
    let rel = |p: &Path| project.rel(p);

    // 2. Dossiers
    for dir in [project.notes(), project.global_dir(), project.tickets_dir(), project.doc_dir()] {
        if !dir.is_dir() {
            fs::create_dir_all(&dir).with_context(|| format!("création de {}", dir.display()))?;
            report.created.push(format!("{}/", rel(&dir)));
        }
    }
    let keep = project.tickets_dir().join(".gitkeep");
    fsutil::create_if_missing(&keep, "")?;

    // 3. Fichiers globaux (jamais écrasés)
    let readme = project.global_dir().join("README.md");
    if fsutil::create_if_missing(&readme, &templates::with_notes_dir(templates::GLOBAL_README, &project.cfg.notes_dir))? {
        report.created.push(rel(&readme));
    }
    let index = project.doc_dir().join("INDEX.md");
    if fsutil::create_if_missing(&index, templates::DOC_INDEX)? {
        report.created.push(rel(&index));
    }

    // 4. Réparation des tickets incomplets
    for entry in fs::read_dir(project.tickets_dir())?.filter_map(|e| e.ok()) {
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with('.') || !entry.path().is_dir() {
            continue;
        }
        let Some((id, _)) = naming::parse_dir_name(&name, &project.cfg) else {
            report.warnings.push(format!("{} : nom de dossier non conforme, ignoré", rel(&entry.path())));
            continue;
        };
        let id_str = id.format(project.cfg.id_width);
        let dir = entry.path();
        if !dir.join(TICKET_FILE).is_file() {
            report.warnings.push(format!(
                "{} : ticket.md manquant, impossible à reconstituer automatiquement",
                rel(&dir)
            ));
        }
        for (file, content) in [
            (JOURNAL_FILE, templates::journal(&id_str)),
            (DECISIONS_FILE, templates::decisions(&id_str)),
        ] {
            let p = dir.join(file);
            if fsutil::create_if_missing(&p, &content)? {
                report.created.push(rel(&p));
            }
        }
    }

    // 5. Board
    let board_existed = project.board_path().exists();
    if project.regenerate_board()? {
        let r = rel(&project.board_path());
        if board_existed { report.updated.push(r) } else { report.created.push(r) }
    }

    // 6. Bloc CLAUDE.md
    if opts.claude_md {
        let path = root.join("CLAUDE.md");
        let block = templates::with_notes_dir(templates::CLAUDE_BLOCK, &project.cfg.notes_dir);
        match upsert_claude_block(&path, &block)? {
            Upsert::Created => report.created.push("CLAUDE.md".into()),
            Upsert::Updated => report.updated.push("CLAUDE.md (bloc coutcouticket)".into()),
            Upsert::Unchanged => {}
        }
    }

    // 7. Hooks git
    if opts.git_hooks {
        if git::is_repo(&root) {
            let hooks = git::hooks_dir(&root)?;
            fs::create_dir_all(&hooks)?;
            for name in GIT_HOOKS {
                let path = hooks.join(name);
                let script = hook_script(name);
                if path.exists() {
                    let existing = fs::read_to_string(&path).unwrap_or_default();
                    if !existing.contains(HOOK_MARKER) {
                        report.warnings.push(format!(
                            "hook git {name} déjà présent et non géré par coutcouticket : non modifié. Ajouter à la main : coutcouticket hook {name}{}",
                            if name == "prepare-commit-msg" { " \"$@\"" } else { "" }
                        ));
                        continue;
                    }
                    if existing == script {
                        continue;
                    }
                    fsutil::write_atomic(&path, &script)?;
                    report.updated.push(format!("hook git {name}"));
                } else {
                    fsutil::write_atomic(&path, &script)?;
                    report.created.push(format!("hook git {name}"));
                }
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    fs::set_permissions(&path, fs::Permissions::from_mode(0o755))?;
                }
            }
        } else {
            report.warnings.push("pas de dépôt git : hooks non installés (relancer init après git init)".into());
        }
    }

    // 8. Registre (pour le démon)
    if opts.register {
        match Registry::load().and_then(|mut r| {
            let added = r.add(&root)?;
            if added {
                r.save()?;
            }
            Ok(added)
        }) {
            Ok(true) => report.created.push("enregistrement dans le registre des projets".into()),
            Ok(false) => {}
            Err(e) => report.warnings.push(format!("enregistrement impossible : {e:#}")),
        }
    }

    Ok(report)
}

enum Upsert {
    Created,
    Updated,
    Unchanged,
}

fn upsert_claude_block(path: &Path, block: &str) -> Result<Upsert> {
    let Ok(existing) = fs::read_to_string(path) else {
        fsutil::write_atomic(path, &format!("# CLAUDE.md\n\n{block}"))?;
        return Ok(Upsert::Created);
    };
    let new = match (existing.find(templates::CLAUDE_BLOCK_START), existing.find(templates::CLAUDE_BLOCK_END)) {
        (Some(start), Some(end)) if end > start => {
            let end = end + templates::CLAUDE_BLOCK_END.len();
            let end = if existing[end..].starts_with('\n') { end + 1 } else { end };
            format!("{}{}{}", &existing[..start], block, &existing[end..])
        }
        (None, None) => {
            let sep = if existing.ends_with("\n\n") { "" } else if existing.ends_with('\n') { "\n" } else { "\n\n" };
            format!("{existing}{sep}{block}")
        }
        _ => bail!("CLAUDE.md contient un bloc coutcouticket incomplet (marqueur start/end manquant) : corriger à la main"),
    };
    if fsutil::write_if_changed(path, &new)? { Ok(Upsert::Updated) } else { Ok(Upsert::Unchanged) }
}
