//! Cœur métier : toutes les opérations déterministes sur les tickets.
//! La CLI, le serveur MCP, les hooks et le démon passent tous par ici.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow, bail};
use chrono::{Local, NaiveDate};
use serde::Serialize;

use crate::board;
use crate::config::{Config, find_root};
use crate::fsutil::{self, ProjectLock};
use crate::git;
use crate::model::{Frontmatter, Priority, Status, TicketDoc, TicketId};
use crate::naming;
use crate::templates;

pub const TICKET_FILE: &str = "ticket.md";
pub const JOURNAL_FILE: &str = "journal.md";
pub const DECISIONS_FILE: &str = "decisions.md";
pub const NEXT_STEP_MARK: &str = "**Prochaine étape :**";

#[derive(Debug, Clone)]
pub struct Project {
    pub root: PathBuf,
    pub cfg: Config,
}

#[derive(Debug, Clone)]
pub struct Ticket {
    pub dir_name: String,
    pub path: PathBuf,
    pub doc: TicketDoc,
}

#[derive(Debug, Clone, Serialize)]
pub struct Problem {
    pub path: String,
    pub message: String,
}

#[derive(Debug, Default)]
pub struct Scan {
    pub tickets: Vec<Ticket>,
    pub problems: Vec<Problem>,
}

#[derive(Debug, Clone, Default)]
pub struct CreateInput {
    pub title: String,
    pub kind: Option<String>,
    pub priority: Option<Priority>,
    pub projects: Vec<String>,
    pub description: Option<String>,
    pub acceptance: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TicketSummary {
    pub id: String,
    pub title: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub status: Status,
    pub priority: Priority,
    pub projects: Vec<String>,
    pub created: String,
    pub updated: String,
    pub dir: String,
    pub branch: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct TicketContext {
    #[serde(flatten)]
    pub summary: TicketSummary,
    pub ticket_file: String,
    pub journal_file: String,
    pub decisions_file: String,
    pub last_next_step: Option<String>,
    pub journal_entries: usize,
    pub decisions: usize,
    pub on_ticket_branch: bool,
    pub current_branch: Option<String>,
}

fn today() -> NaiveDate {
    Local::now().date_naive()
}

fn now_stamp() -> String {
    Local::now().format("%Y-%m-%d %H:%M").to_string()
}

impl Project {
    /// Ouvre le projet contenant `path` (remonte jusqu'à `.coutcouticket.toml`).
    pub fn open(path: &Path) -> Result<Self> {
        let root = find_root(path)?;
        let cfg = Config::load(&root)?;
        Ok(Project { root, cfg })
    }

    pub fn name(&self) -> String {
        self.root.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| "projet".into())
    }

    pub fn notes(&self) -> PathBuf {
        self.root.join(&self.cfg.notes_dir)
    }
    pub fn global_dir(&self) -> PathBuf {
        self.notes().join("0-global")
    }
    pub fn tickets_dir(&self) -> PathBuf {
        self.notes().join("tickets")
    }
    pub fn doc_dir(&self) -> PathBuf {
        self.notes().join("doc")
    }
    pub fn board_path(&self) -> PathBuf {
        self.global_dir().join("BOARD.md")
    }

    pub fn rel(&self, path: &Path) -> String {
        path.strip_prefix(&self.root).unwrap_or(path).to_string_lossy().to_string()
    }

    // -----------------------------------------------------------------------
    // Lecture
    // -----------------------------------------------------------------------

    pub fn scan(&self) -> Result<Scan> {
        let mut scan = Scan::default();
        let dir = self.tickets_dir();
        if !dir.is_dir() {
            return Ok(scan);
        }
        let mut entries: Vec<_> = fs::read_dir(&dir)?.filter_map(|e| e.ok()).collect();
        entries.sort_by_key(|e| e.file_name());
        for entry in entries {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with('.') {
                continue;
            }
            let path = entry.path();
            let rel = self.rel(&path);
            if !path.is_dir() {
                scan.problems.push(Problem { path: rel, message: "fichier inattendu dans tickets/ (seuls des dossiers de ticket sont attendus)".into() });
                continue;
            }
            let Some((id, _)) = naming::parse_dir_name(&name, &self.cfg) else {
                scan.problems.push(Problem {
                    path: rel,
                    message: format!(
                        "nom de dossier non conforme : attendu <{} chiffres>-<slug>, ex. {}",
                        self.cfg.id_width,
                        naming::dir_name(TicketId(13), "add-thing-to-etc", &self.cfg)
                    ),
                });
                continue;
            };
            let ticket_path = path.join(TICKET_FILE);
            let content = match fs::read_to_string(&ticket_path) {
                Ok(c) => c,
                Err(_) => {
                    scan.problems.push(Problem { path: self.rel(&ticket_path), message: "ticket.md manquant".into() });
                    continue;
                }
            };
            match TicketDoc::parse(&content) {
                Ok(doc) => {
                    if doc.front.id != id {
                        scan.problems.push(Problem {
                            path: self.rel(&ticket_path),
                            message: format!(
                                "l'id du frontmatter ({}) ne correspond pas au dossier ({})",
                                doc.front.id.format(self.cfg.id_width),
                                id.format(self.cfg.id_width)
                            ),
                        });
                        continue;
                    }
                    scan.tickets.push(Ticket { dir_name: name, path, doc });
                }
                Err(e) => scan.problems.push(Problem { path: self.rel(&ticket_path), message: format!("{e:#}") }),
            }
        }
        Ok(scan)
    }

    pub fn find(&self, id: TicketId) -> Result<Ticket> {
        let scan = self.scan()?;
        let mut found: Vec<Ticket> = scan.tickets.into_iter().filter(|t| t.doc.front.id == id).collect();
        match found.len() {
            1 => Ok(found.remove(0)),
            0 => {
                let prefix = format!("{}-", id.format(self.cfg.id_width));
                if let Some(p) = scan.problems.iter().find(|p| p.path.contains(&prefix)) {
                    bail!("ticket {} invalide : {} ({})", id.format(self.cfg.id_width), p.message, p.path)
                }
                bail!("ticket {} introuvable dans {}", id.format(self.cfg.id_width), self.rel(&self.tickets_dir()))
            }
            _ => bail!("plusieurs dossiers portent l'id {} : corriger à la main", id.format(self.cfg.id_width)),
        }
    }

    pub fn summary(&self, t: &Ticket) -> TicketSummary {
        let f = &t.doc.front;
        TicketSummary {
            id: f.id.format(self.cfg.id_width),
            title: f.title.clone(),
            kind: f.kind.clone(),
            status: f.status,
            priority: f.priority,
            projects: f.projects.clone(),
            created: f.created.format("%Y-%m-%d").to_string(),
            updated: f.updated.format("%Y-%m-%d").to_string(),
            dir: self.rel(&t.path),
            branch: naming::branch_name(&f.kind, &t.dir_name),
        }
    }

    pub fn list(&self, status: Option<Status>, project: Option<&str>) -> Result<Vec<TicketSummary>> {
        let mut tickets: Vec<Ticket> = self
            .scan()?
            .tickets
            .into_iter()
            .filter(|t| status.is_none_or(|s| t.doc.front.status == s))
            .filter(|t| project.is_none_or(|p| t.doc.front.projects.iter().any(|x| x == p)))
            .collect();
        tickets.sort_by(|a, b| {
            (a.doc.front.priority, a.doc.front.id).cmp(&(b.doc.front.priority, b.doc.front.id))
        });
        Ok(tickets.iter().map(|t| self.summary(t)).collect())
    }

    pub fn context(&self, id: TicketId) -> Result<TicketContext> {
        let t = self.find(id)?;
        let journal_path = t.path.join(JOURNAL_FILE);
        let decisions_path = t.path.join(DECISIONS_FILE);
        let journal = fs::read_to_string(&journal_path).unwrap_or_default();
        let decisions = fs::read_to_string(&decisions_path).unwrap_or_default();
        let summary = self.summary(&t);
        let current_branch = if git::is_repo(&self.root) { git::current_branch(&self.root)? } else { None };
        Ok(TicketContext {
            on_ticket_branch: current_branch.as_deref() == Some(summary.branch.as_str()),
            current_branch,
            ticket_file: self.rel(&t.path.join(TICKET_FILE)),
            journal_file: self.rel(&journal_path),
            decisions_file: self.rel(&decisions_path),
            last_next_step: last_next_step(&journal),
            journal_entries: journal.lines().filter(|l| l.starts_with("## ")).count(),
            decisions: count_decisions(&decisions),
            summary,
        })
    }

    pub fn files(&self, id: TicketId) -> Result<(Vec<String>, Vec<String>)> {
        let t = self.find(id)?;
        if !git::is_repo(&self.root) {
            bail!("le projet n'est pas un dépôt git");
        }
        let id_str = t.doc.front.id.format(self.cfg.id_width);
        let committed = git::files_for_ticket(&self.root, &id_str)?;
        let branch = naming::branch_name(&t.doc.front.kind, &t.dir_name);
        let pending = if git::current_branch(&self.root)?.as_deref() == Some(branch.as_str()) {
            git::uncommitted_files(&self.root)?
        } else {
            vec![]
        };
        Ok((committed, pending))
    }

    // -----------------------------------------------------------------------
    // Écriture (toujours sous verrou, board régénéré à la fin)
    // -----------------------------------------------------------------------

    fn save_ticket(&self, t: &mut Ticket) -> Result<()> {
        t.doc.front.updated = today();
        fsutil::write_atomic(&t.path.join(TICKET_FILE), &t.doc.render(self.cfg.id_width))
    }

    fn check_kind(&self, kind: &str) -> Result<()> {
        if !self.cfg.branch_types.iter().any(|k| k == kind) {
            bail!("type « {kind} » non autorisé (types : {})", self.cfg.branch_types.join(", "));
        }
        Ok(())
    }

    fn check_projects(&self, projects: &[String]) -> Result<()> {
        for p in projects {
            if p.is_empty() || p.contains(',') || p.contains(']') || p.contains('[') {
                bail!("nom de projet invalide « {p} »");
            }
            if !self.cfg.projects.is_empty() && !self.cfg.projects.contains(p) {
                bail!("projet « {p} » non autorisé (projets : {})", self.cfg.projects.join(", "));
            }
        }
        Ok(())
    }

    fn next_id(&self) -> Result<TicketId> {
        let dir = self.tickets_dir();
        let mut max = 0u32;
        if dir.is_dir() {
            for entry in fs::read_dir(&dir)?.filter_map(|e| e.ok()) {
                let name = entry.file_name().to_string_lossy().to_string();
                let digits: String = name.chars().take_while(|c| c.is_ascii_digit()).collect();
                if let Ok(n) = digits.parse::<u32>() {
                    max = max.max(n);
                }
            }
        }
        let next = max + 1;
        if next > self.cfg.max_id() {
            bail!("plus d'identifiant disponible (maximum {})", self.cfg.max_id());
        }
        Ok(TicketId(next))
    }

    pub fn create(&self, input: CreateInput) -> Result<TicketSummary> {
        self.ensure_initialized()?;
        let _lock = ProjectLock::acquire(&self.root)?;
        let title = input.title.trim().to_string();
        if title.is_empty() {
            bail!("le titre ne peut pas être vide");
        }
        let kind = input.kind.unwrap_or_else(|| self.cfg.default_type.clone());
        self.check_kind(&kind)?;
        self.check_projects(&input.projects)?;
        let slug = naming::slugify(&title, self.cfg.slug_max_len);
        if slug.is_empty() {
            bail!("le titre doit contenir au moins une lettre ou un chiffre");
        }
        let id = self.next_id()?;
        let dir_name = naming::dir_name(id, &slug, &self.cfg);
        let path = self.tickets_dir().join(&dir_name);
        if path.exists() {
            bail!("le dossier {} existe déjà", self.rel(&path));
        }
        let date = today();
        let doc = TicketDoc {
            front: Frontmatter {
                id,
                title,
                kind,
                status: Status::Todo,
                priority: input.priority.unwrap_or(Priority::P2),
                projects: input.projects,
                created: date,
                updated: date,
            },
            body: templates::ticket_body(input.description.as_deref(), &input.acceptance),
        };
        fs::create_dir_all(&path)?;
        let id_str = id.format(self.cfg.id_width);
        fsutil::write_atomic(&path.join(TICKET_FILE), &doc.render(self.cfg.id_width))?;
        fsutil::write_atomic(&path.join(JOURNAL_FILE), &templates::journal(&id_str))?;
        fsutil::write_atomic(&path.join(DECISIONS_FILE), &templates::decisions(&id_str))?;
        let ticket = Ticket { dir_name, path, doc };
        self.regenerate_board_locked()?;
        Ok(self.summary(&ticket))
    }

    /// Passe le ticket en cours et bascule sur sa branche (créée si besoin).
    pub fn start(&self, id: TicketId) -> Result<(TicketSummary, bool)> {
        let _lock = ProjectLock::acquire(&self.root)?;
        let mut t = self.find(id)?;
        if !t.doc.front.status.is_open() {
            bail!(
                "le ticket {} est « {} » : le rouvrir d'abord avec ticket_set_status",
                t.doc.front.id.format(self.cfg.id_width),
                t.doc.front.status
            );
        }
        if !git::is_repo(&self.root) {
            bail!("le projet n'est pas un dépôt git : impossible de créer la branche");
        }
        let branch = naming::branch_name(&t.doc.front.kind, &t.dir_name);
        naming::classify_branch(&branch, &self.cfg)?;
        // Bascule d'abord : la modification du statut atterrit sur la branche du ticket.
        let created = git::switch_or_create(&self.root, &branch)?;
        let previous = t.doc.front.status;
        if previous != Status::InProgress {
            t.doc.front.status = Status::InProgress;
            self.save_ticket(&mut t)?;
            let entry = format!(
                "\n## {} — statut : {} → {}\n\nDémarrage sur la branche `{branch}`.\n",
                now_stamp(),
                previous,
                Status::InProgress
            );
            fsutil::append(&t.path.join(JOURNAL_FILE), &entry)?;
        }
        self.regenerate_board_locked()?;
        Ok((self.summary(&t), created))
    }

    pub fn set_status(&self, id: TicketId, status: Status, note: Option<&str>) -> Result<TicketSummary> {
        let _lock = ProjectLock::acquire(&self.root)?;
        let mut t = self.find(id)?;
        let previous = t.doc.front.status;
        if previous == status {
            bail!("le ticket est déjà « {status} »");
        }
        t.doc.front.status = status;
        self.save_ticket(&mut t)?;
        let note = note.map(str::trim).filter(|n| !n.is_empty());
        let entry = match note {
            Some(n) => format!("\n## {} — statut : {previous} → {status}\n\n{n}\n", now_stamp()),
            None => format!("\n## {} — statut : {previous} → {status}\n", now_stamp()),
        };
        fsutil::append(&t.path.join(JOURNAL_FILE), &entry)?;
        self.regenerate_board_locked()?;
        Ok(self.summary(&t))
    }

    pub fn log(&self, id: TicketId, text: &str, next_step: &str) -> Result<TicketSummary> {
        let (text, next_step) = (text.trim(), next_step.trim());
        if text.is_empty() {
            bail!("le texte de l'entrée ne peut pas être vide");
        }
        if next_step.is_empty() {
            bail!("« prochaine étape » est obligatoire (écrire « Aucune » si le ticket est terminé)");
        }
        let _lock = ProjectLock::acquire(&self.root)?;
        let mut t = self.find(id)?;
        let entry = format!("\n## {} — avancement\n\n{text}\n\n{NEXT_STEP_MARK} {next_step}\n", now_stamp());
        fsutil::append(&t.path.join(JOURNAL_FILE), &entry)?;
        self.save_ticket(&mut t)?;
        self.regenerate_board_locked()?;
        Ok(self.summary(&t))
    }

    pub fn decide(&self, id: TicketId, title: &str, decision: &str, alternatives: &str, why: &str) -> Result<String> {
        for (name, v) in [("titre", title), ("décision", decision), ("pourquoi", why)] {
            if v.trim().is_empty() {
                bail!("le champ « {name} » est obligatoire");
            }
        }
        let _lock = ProjectLock::acquire(&self.root)?;
        let mut t = self.find(id)?;
        let path = t.path.join(DECISIONS_FILE);
        let existing = fs::read_to_string(&path).unwrap_or_default();
        let n = count_decisions(&existing) + 1;
        let alternatives = if alternatives.trim().is_empty() { "Aucune envisagée." } else { alternatives.trim() };
        let entry = format!(
            "\n## D{n} — {} ({})\n\n**Décision :** {}\n\n**Alternatives écartées :** {}\n\n**Pourquoi :** {}\n",
            title.trim(),
            today().format("%Y-%m-%d"),
            decision.trim(),
            alternatives,
            why.trim()
        );
        if !path.exists() {
            fsutil::write_atomic(&path, &templates::decisions(&t.doc.front.id.format(self.cfg.id_width)))?;
        }
        fsutil::append(&path, &entry)?;
        self.save_ticket(&mut t)?;
        self.regenerate_board_locked()?;
        Ok(format!("D{n}"))
    }

    // -----------------------------------------------------------------------
    // Board et validation
    // -----------------------------------------------------------------------

    pub fn render_board(&self) -> Result<String> {
        let scan = self.scan()?;
        Ok(board::render(self, &scan))
    }

    /// Régénère BOARD.md (sous verrou). Retourne true si le fichier a changé.
    pub fn regenerate_board(&self) -> Result<bool> {
        let _lock = ProjectLock::acquire(&self.root)?;
        self.regenerate_board_locked()
    }

    fn regenerate_board_locked(&self) -> Result<bool> {
        let content = self.render_board()?;
        fsutil::write_if_changed(&self.board_path(), &content)
    }

    fn ensure_initialized(&self) -> Result<()> {
        if !self.tickets_dir().is_dir() {
            bail!("{} absent : lancer « coutcouticket init »", self.rel(&self.tickets_dir()));
        }
        Ok(())
    }

    pub fn validate(&self) -> Result<Vec<Problem>> {
        let mut problems = Vec::new();
        let required_dirs = [self.notes(), self.global_dir(), self.tickets_dir(), self.doc_dir()];
        for d in &required_dirs {
            if !d.is_dir() {
                problems.push(Problem { path: self.rel(d), message: "dossier manquant (réparer avec « coutcouticket init »)".into() });
            }
        }
        for f in [self.global_dir().join("README.md"), self.doc_dir().join("INDEX.md")] {
            if !f.is_file() {
                problems.push(Problem { path: self.rel(&f), message: "fichier manquant (réparer avec « coutcouticket init »)".into() });
            }
        }
        let scan = self.scan()?;
        problems.extend(scan.problems.iter().cloned());
        let mut ids: BTreeMap<TicketId, Vec<String>> = BTreeMap::new();
        for t in &scan.tickets {
            let f = &t.doc.front;
            let rel = self.rel(&t.path.join(TICKET_FILE));
            ids.entry(f.id).or_default().push(t.dir_name.clone());
            if let Err(e) = self.check_kind(&f.kind) {
                problems.push(Problem { path: rel.clone(), message: format!("{e:#}") });
            }
            if let Err(e) = self.check_projects(&f.projects) {
                problems.push(Problem { path: rel.clone(), message: format!("{e:#}") });
            }
            if f.created > f.updated {
                problems.push(Problem { path: rel.clone(), message: "« created » est postérieur à « updated »".into() });
            }
            for file in [JOURNAL_FILE, DECISIONS_FILE] {
                if !t.path.join(file).is_file() {
                    problems.push(Problem {
                        path: self.rel(&t.path.join(file)),
                        message: "fichier manquant (réparer avec « coutcouticket init »)".into(),
                    });
                }
            }
        }
        for (id, dirs) in ids {
            if dirs.len() > 1 {
                problems.push(Problem {
                    path: self.rel(&self.tickets_dir()),
                    message: format!("id {} utilisé par plusieurs dossiers : {}", id.format(self.cfg.id_width), dirs.join(", ")),
                });
            }
        }
        if self.global_dir().is_dir() {
            let expected = board::render(self, &scan);
            let current = fs::read_to_string(self.board_path()).unwrap_or_default();
            if current != expected {
                problems.push(Problem {
                    path: self.rel(&self.board_path()),
                    message: "BOARD.md obsolète (régénérer avec « coutcouticket board »)".into(),
                });
            }
        }
        Ok(problems)
    }
}

pub fn last_next_step(journal: &str) -> Option<String> {
    journal
        .lines()
        .rev()
        .find_map(|l| l.strip_prefix(NEXT_STEP_MARK))
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

fn count_decisions(text: &str) -> usize {
    text.lines()
        .filter(|l| {
            l.strip_prefix("## D")
                .map(|r| r.chars().next().is_some_and(|c| c.is_ascii_digit()))
                .unwrap_or(false)
        })
        .count()
}

/// Résout le projet à partir d'un chemin optionnel, sinon du dossier courant.
pub fn resolve_project(path: Option<&str>) -> Result<Project> {
    let base = match path {
        Some(p) => PathBuf::from(p),
        None => std::env::current_dir().context("dossier courant inaccessible")?,
    };
    Project::open(&base).map_err(|e| anyhow!("{e:#}"))
}
