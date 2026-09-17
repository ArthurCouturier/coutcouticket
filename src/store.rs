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
use crate::model::{Frontmatter, Priority, Status, TicketDoc, TicketId, normalize_ids};
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

/// Rattachement d'un fichier du dépôt, pour les trailers et `files`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PathOwner {
    /// Hors du dossier de notes (ou hors du projet).
    Code,
    /// Dans le dossier d'un ticket.
    Ticket(TicketId),
    /// `BOARD.md`, régénéré par le hook pre-commit.
    Board,
    /// Autres notes : doc, README de 0-global, fichiers mal placés.
    Notes,
}

#[derive(Debug, Clone, Default)]
pub struct CreateInput {
    pub title: String,
    pub kind: Option<String>,
    pub priority: Option<Priority>,
    pub projects: Vec<String>,
    pub description: Option<String>,
    pub acceptance: Vec<String>,
    pub blocked_by: Vec<TicketId>,
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
    /// Dépendances déclarées (frontmatter).
    pub blocked_by: Vec<String>,
    /// Dépendances encore ouvertes : celles qui bloquent réellement le ticket.
    pub open_blockers: Vec<String>,
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
        let deps = self.dependency_problems(&scan.tickets);
        scan.problems.extend(deps);
        Ok(scan)
    }

    /// Dépendances incohérentes : id inconnu, auto-référence, cycle.
    fn dependency_problems(&self, tickets: &[Ticket]) -> Vec<Problem> {
        let w = self.cfg.id_width;
        let mut problems = Vec::new();
        let known: BTreeMap<TicketId, &Ticket> = tickets.iter().map(|t| (t.doc.front.id, t)).collect();
        for t in tickets {
            let f = &t.doc.front;
            let id = f.id.format(w);
            let path = self.rel(&t.path.join(TICKET_FILE));
            for dep in &f.blocked_by {
                let d = dep.format(w);
                if *dep == f.id {
                    problems.push(Problem {
                        path: path.clone(),
                        message: format!(
                            "le ticket {id} dépend de lui-même : retirer la dépendance avec « coutcouticket depend {id} --on {d} --remove »"
                        ),
                    });
                } else if !known.contains_key(dep) {
                    problems.push(Problem {
                        path: path.clone(),
                        message: format!(
                            "« blocked_by » cite le ticket {d}, introuvable : retirer la dépendance avec « coutcouticket depend {id} --on {d} --remove »"
                        ),
                    });
                }
            }
        }
        for cycle in find_cycles(&dependency_graph(tickets)) {
            let first = cycle[0];
            let t = known[&first];
            let text: Vec<String> = cycle.iter().chain(std::iter::once(&first)).map(|i| i.format(w)).collect();
            let (a, b) = (cycle[0].format(w), cycle[1 % cycle.len()].format(w));
            problems.push(Problem {
                path: self.rel(&t.path.join(TICKET_FILE)),
                message: format!(
                    "cycle de dépendances : {} ; le casser, par exemple avec « coutcouticket depend {a} --on {b} --remove »",
                    text.join(" → ")
                ),
            });
        }
        problems
    }

    pub fn find(&self, id: TicketId) -> Result<Ticket> {
        Ok(self.find_with_scan(id)?.0)
    }

    /// Comme `find`, en rendant aussi le scan (statuts des dépendances).
    fn find_with_scan(&self, id: TicketId) -> Result<(Ticket, Scan)> {
        let scan = self.scan()?;
        let found: Vec<&Ticket> = scan.tickets.iter().filter(|t| t.doc.front.id == id).collect();
        match found.len() {
            1 => {
                let t = found[0].clone();
                Ok((t, scan))
            }
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

    /// Dépendances de `t` encore ouvertes parmi `all`. Un id inconnu n'est pas compté
    /// (il est signalé par la validation).
    pub fn open_blockers(&self, t: &Ticket, all: &[Ticket]) -> Vec<TicketId> {
        t.doc
            .front
            .blocked_by
            .iter()
            .copied()
            .filter(|dep| all.iter().any(|o| o.doc.front.id == *dep && o.doc.front.status.is_open()))
            .collect()
    }

    /// `all` : tous les tickets du projet, pour savoir quelles dépendances sont encore ouvertes.
    pub fn summary(&self, t: &Ticket, all: &[Ticket]) -> TicketSummary {
        let f = &t.doc.front;
        let w = self.cfg.id_width;
        TicketSummary {
            blocked_by: f.blocked_by.iter().map(|i| i.format(w)).collect(),
            open_blockers: self.open_blockers(t, all).iter().map(|i| i.format(w)).collect(),
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
        let scan = self.scan()?;
        let mut tickets: Vec<&Ticket> = scan
            .tickets
            .iter()
            .filter(|t| status.is_none_or(|s| t.doc.front.status == s))
            .filter(|t| project.is_none_or(|p| t.doc.front.projects.iter().any(|x| x == p)))
            .collect();
        tickets.sort_by(|a, b| {
            (a.doc.front.priority, a.doc.front.id).cmp(&(b.doc.front.priority, b.doc.front.id))
        });
        Ok(tickets.iter().map(|t| self.summary(t, &scan.tickets)).collect())
    }

    pub fn context(&self, id: TicketId) -> Result<TicketContext> {
        let (t, scan) = self.find_with_scan(id)?;
        let journal_path = t.path.join(JOURNAL_FILE);
        let decisions_path = t.path.join(DECISIONS_FILE);
        let journal = fs::read_to_string(&journal_path).unwrap_or_default();
        let decisions = fs::read_to_string(&decisions_path).unwrap_or_default();
        let summary = self.summary(&t, &scan.tickets);
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
        let prefix = git::show_prefix(&self.root)?;
        // Notes d'autres tickets et BOARD.md : présents dans des commits du ticket
        // sans le concerner (commit de notes groupé, board régénéré à chaque commit).
        let keep = |f: &String| match self.path_owner(&prefix, f) {
            PathOwner::Ticket(other) => other == id,
            PathOwner::Board => false,
            PathOwner::Notes | PathOwner::Code => true,
        };
        let committed = git::files_for_ticket(&self.root, &id_str)?.into_iter().filter(keep).collect();
        let branch = naming::branch_name(&t.doc.front.kind, &t.dir_name);
        let pending = if git::current_branch(&self.root)?.as_deref() == Some(branch.as_str()) {
            git::uncommitted_files(&self.root)?.into_iter().filter(keep).collect()
        } else {
            vec![]
        };
        Ok((committed, pending))
    }

    /// Rattache un chemin git (relatif à la racine du dépôt, `prefix` = chemin du
    /// projet dans le dépôt) au code, au dossier d'un ticket, au board ou aux autres notes.
    pub fn path_owner(&self, prefix: &str, path: &str) -> PathOwner {
        let notes = format!("{}/", self.cfg.notes_dir.trim_end_matches('/'));
        let Some(rest) = path.strip_prefix(prefix).and_then(|p| p.strip_prefix(notes.as_str())) else {
            return PathOwner::Code;
        };
        if rest == "0-global/BOARD.md" {
            return PathOwner::Board;
        }
        if let Some((dir, file)) = rest.strip_prefix("tickets/").and_then(|r| r.split_once('/')) {
            if let Some((id, _)) = naming::parse_dir_name(dir, &self.cfg).filter(|_| !file.is_empty()) {
                return PathOwner::Ticket(id);
            }
        }
        PathOwner::Notes
    }

    /// Tickets à inscrire en trailer d'un commit fait sur la branche de `branch_id`.
    /// Un commit qui ne touche que des notes d'autres tickets leur revient ; tout autre
    /// commit (code, notes du ticket de la branche, doc seule, index vide) revient à la branche.
    pub fn commit_tickets(&self, branch_id: TicketId, staged: &[String], prefix: &str) -> Vec<TicketId> {
        let mut touched = Vec::new();
        for f in staged {
            match self.path_owner(prefix, f) {
                PathOwner::Code => return vec![branch_id],
                PathOwner::Ticket(id) if id == branch_id => return vec![branch_id],
                PathOwner::Ticket(id) => touched.push(id),
                PathOwner::Board | PathOwner::Notes => {}
            }
        }
        if touched.is_empty() {
            return vec![branch_id];
        }
        normalize_ids(&mut touched);
        touched
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
        let scan = self.scan()?;
        let mut blocked_by = input.blocked_by;
        normalize_ids(&mut blocked_by);
        self.check_new_dependencies(&scan.tickets, id, &blocked_by)?;
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
                blocked_by,
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
        Ok(self.summary(&ticket, &scan.tickets))
    }

    /// Passe le ticket en cours et bascule sur sa branche (créée si besoin).
    pub fn start(&self, id: TicketId) -> Result<(TicketSummary, bool)> {
        let _lock = ProjectLock::acquire(&self.root)?;
        let (mut t, scan) = self.find_with_scan(id)?;
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
        Ok((self.summary(&t, &scan.tickets), created))
    }

    pub fn set_status(&self, id: TicketId, status: Status, note: Option<&str>) -> Result<TicketSummary> {
        let _lock = ProjectLock::acquire(&self.root)?;
        let (mut t, scan) = self.find_with_scan(id)?;
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
        Ok(self.summary(&t, &scan.tickets))
    }

    /// Vérifie que `id` peut dépendre de `deps` : ids existants, pas d'auto-référence,
    /// pas de cycle une fois ces dépendances en place.
    fn check_new_dependencies(&self, tickets: &[Ticket], id: TicketId, deps: &[TicketId]) -> Result<()> {
        let w = self.cfg.id_width;
        let me = id.format(w);
        for dep in deps {
            let d = dep.format(w);
            if *dep == id {
                bail!("le ticket {me} ne peut pas dépendre de lui-même : retirer {d} de la liste");
            }
            if !tickets.iter().any(|t| t.doc.front.id == *dep) {
                bail!("ticket {d} introuvable : vérifier l'id avec « coutcouticket list » (dépendance refusée pour {me})");
            }
        }
        let mut graph = dependency_graph(tickets);
        graph.insert(id, deps.to_vec());
        for dep in deps {
            if let Some(path) = find_path(&graph, *dep, id) {
                let text: Vec<String> = std::iter::once(id).chain(path).map(|i| i.format(w)).collect();
                bail!(
                    "{me} ne peut pas dépendre de {} : cela créerait le cycle {}. Retirer d'abord une dépendance de ce cycle (« coutcouticket depend <id> --on <id> --remove »)",
                    dep.format(w),
                    text.join(" → ")
                );
            }
        }
        Ok(())
    }

    /// Ajoute (ou retire si `remove`) des dépendances à un ticket. Consigné dans le journal.
    pub fn depend(&self, id: TicketId, on: &[TicketId], remove: bool) -> Result<TicketSummary> {
        if on.is_empty() {
            bail!("aucune dépendance indiquée : préciser au moins un id (ex. --on 3)");
        }
        let _lock = ProjectLock::acquire(&self.root)?;
        let (mut t, scan) = self.find_with_scan(id)?;
        let w = self.cfg.id_width;
        let me = id.format(w);
        let mut on = on.to_vec();
        normalize_ids(&mut on);
        let mut deps = t.doc.front.blocked_by.clone();
        if remove {
            for dep in &on {
                if !deps.contains(dep) {
                    bail!("le ticket {me} ne dépend pas de {} (dépendances : {})", dep.format(w), ids_text(&deps, w));
                }
            }
            deps.retain(|d| !on.contains(d));
            // Pas de contrôle d'existence au retrait : il doit permettre de réparer un id inconnu.
        } else {
            for dep in &on {
                if deps.contains(dep) {
                    bail!("le ticket {me} dépend déjà de {}", dep.format(w));
                }
            }
            self.check_new_dependencies(&scan.tickets, id, &on)?;
            deps.extend(on.iter().copied());
            normalize_ids(&mut deps);
        }
        t.doc.front.blocked_by = deps;
        self.save_ticket(&mut t)?;
        let entry = format!(
            "\n## {} — dépendances\n\n{} : {}. Bloqué par : {}.\n",
            now_stamp(),
            if remove { "Retrait" } else { "Ajout" },
            ids_text(&on, w),
            ids_text(&t.doc.front.blocked_by, w)
        );
        fsutil::append(&t.path.join(JOURNAL_FILE), &entry)?;
        self.regenerate_board_locked()?;
        Ok(self.summary(&t, &scan.tickets))
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
        let (mut t, scan) = self.find_with_scan(id)?;
        let entry = format!("\n## {} — avancement\n\n{text}\n\n{NEXT_STEP_MARK} {next_step}\n", now_stamp());
        fsutil::append(&t.path.join(JOURNAL_FILE), &entry)?;
        self.save_ticket(&mut t)?;
        self.regenerate_board_locked()?;
        Ok(self.summary(&t, &scan.tickets))
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

fn ids_text(ids: &[TicketId], width: usize) -> String {
    if ids.is_empty() {
        return "aucun".into();
    }
    ids.iter().map(|i| i.format(width)).collect::<Vec<_>>().join(", ")
}

/// Graphe id → dépendances. Les auto-références et ids inconnus sont gardés :
/// `find_cycles` et `find_path` ne suivent que les nœuds présents.
fn dependency_graph(tickets: &[Ticket]) -> BTreeMap<TicketId, Vec<TicketId>> {
    tickets.iter().map(|t| (t.doc.front.id, t.doc.front.blocked_by.clone())).collect()
}

/// Chemin `from` → … → `to` en suivant les dépendances (sans `from` au début).
fn find_path(graph: &BTreeMap<TicketId, Vec<TicketId>>, from: TicketId, to: TicketId) -> Option<Vec<TicketId>> {
    fn walk(
        graph: &BTreeMap<TicketId, Vec<TicketId>>,
        node: TicketId,
        to: TicketId,
        seen: &mut Vec<TicketId>,
        path: &mut Vec<TicketId>,
    ) -> bool {
        path.push(node);
        if node == to {
            return true;
        }
        if !seen.contains(&node) {
            seen.push(node);
            for next in graph.get(&node).into_iter().flatten() {
                if walk(graph, *next, to, seen, path) {
                    return true;
                }
            }
        }
        path.pop();
        false
    }
    let mut path = Vec::new();
    walk(graph, from, to, &mut Vec::new(), &mut path).then_some(path)
}

/// Cycles du graphe (hors auto-références, signalées à part), chacun une seule fois,
/// commençant par son plus petit id. Ordre déterministe.
fn find_cycles(graph: &BTreeMap<TicketId, Vec<TicketId>>) -> Vec<Vec<TicketId>> {
    #[derive(Clone, Copy, PartialEq)]
    enum Mark {
        Open,
        Done,
    }
    fn visit(
        graph: &BTreeMap<TicketId, Vec<TicketId>>,
        node: TicketId,
        marks: &mut BTreeMap<TicketId, Mark>,
        stack: &mut Vec<TicketId>,
        cycles: &mut std::collections::BTreeSet<Vec<TicketId>>,
    ) {
        marks.insert(node, Mark::Open);
        stack.push(node);
        for next in graph.get(&node).into_iter().flatten().copied() {
            if next == node || !graph.contains_key(&next) {
                continue;
            }
            match marks.get(&next) {
                None => visit(graph, next, marks, stack, cycles),
                Some(Mark::Open) => {
                    let start = stack.iter().position(|n| *n == next).expect("nœud ouvert dans la pile");
                    let mut cycle = stack[start..].to_vec();
                    let min = cycle.iter().enumerate().min_by_key(|(_, id)| **id).map(|(i, _)| i).unwrap_or(0);
                    cycle.rotate_left(min);
                    cycles.insert(cycle);
                }
                Some(Mark::Done) => {}
            }
        }
        stack.pop();
        marks.insert(node, Mark::Done);
    }
    let mut marks = BTreeMap::new();
    let mut cycles = std::collections::BTreeSet::new();
    for node in graph.keys() {
        if !marks.contains_key(node) {
            visit(graph, *node, &mut marks, &mut Vec::new(), &mut cycles);
        }
    }
    cycles.into_iter().collect()
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

#[cfg(test)]
mod tests {
    use super::*;

    fn project() -> (tempfile::TempDir, Project) {
        let tmp = tempfile::tempdir().unwrap();
        let p = Project { root: tmp.path().to_path_buf(), cfg: Config::default() };
        fs::create_dir_all(p.tickets_dir()).unwrap();
        fs::create_dir_all(p.global_dir()).unwrap();
        (tmp, p)
    }

    fn new(p: &Project, title: &str, deps: &[u32]) -> Result<TicketSummary> {
        p.create(CreateInput {
            title: title.into(),
            blocked_by: deps.iter().map(|n| TicketId(*n)).collect(),
            ..Default::default()
        })
    }

    /// Écrit directement un blocked_by dans le frontmatter (simule une édition manuelle).
    fn force_deps(p: &Project, id: u32, deps: &[u32]) {
        let t = p.find(TicketId(id)).unwrap();
        let mut doc = t.doc.clone();
        doc.front.blocked_by = deps.iter().map(|n| TicketId(*n)).collect();
        fs::write(t.path.join(TICKET_FILE), doc.render(p.cfg.id_width)).unwrap();
    }

    fn messages(p: &Project) -> Vec<String> {
        p.scan().unwrap().problems.into_iter().map(|pb| pb.message).collect()
    }

    #[test]
    fn creation_avec_dependances_normalisees() {
        let (_tmp, p) = project();
        new(&p, "Un", &[]).unwrap();
        new(&p, "Deux", &[]).unwrap();
        let s = new(&p, "Trois", &[2, 1, 2]).unwrap();
        assert_eq!(s.blocked_by, vec!["0001", "0002"]);
        assert_eq!(s.open_blockers, vec!["0001", "0002"]);
        let text = fs::read_to_string(p.find(TicketId(3)).unwrap().path.join(TICKET_FILE)).unwrap();
        assert!(text.contains("blocked_by: [\"0001\", \"0002\"]"), "{text}");
        assert!(messages(&p).is_empty());
    }

    #[test]
    fn creation_refuse_id_inconnu() {
        let (_tmp, p) = project();
        new(&p, "Un", &[]).unwrap();
        let err = new(&p, "Deux", &[7]).unwrap_err().to_string();
        assert!(err.contains("0007 introuvable"), "{err}");
        assert_eq!(p.scan().unwrap().tickets.len(), 1, "ticket créé malgré l'erreur");
        let err = new(&p, "Deux", &[2]).unwrap_err().to_string();
        assert!(err.contains("lui-même"), "{err}");
    }

    #[test]
    fn depend_ajoute_retire_et_refuse_les_cycles() {
        let (_tmp, p) = project();
        for t in ["Un", "Deux", "Trois"] {
            new(&p, t, &[]).unwrap();
        }
        p.depend(TicketId(2), &[TicketId(1)], false).unwrap();
        p.depend(TicketId(3), &[TicketId(2)], false).unwrap();
        let err = p.depend(TicketId(1), &[TicketId(3)], false).unwrap_err().to_string();
        assert!(err.contains("cycle 0001 → 0003 → 0002 → 0001"), "{err}");
        let err = p.depend(TicketId(2), &[TicketId(2)], false).unwrap_err().to_string();
        assert!(err.contains("lui-même"), "{err}");
        let err = p.depend(TicketId(2), &[TicketId(1)], false).unwrap_err().to_string();
        assert!(err.contains("dépend déjà"), "{err}");
        let err = p.depend(TicketId(2), &[TicketId(9)], false).unwrap_err().to_string();
        assert!(err.contains("introuvable"), "{err}");
        let err = p.depend(TicketId(1), &[TicketId(2)], true).unwrap_err().to_string();
        assert!(err.contains("ne dépend pas"), "{err}");

        let s = p.depend(TicketId(3), &[TicketId(1)], false).unwrap();
        assert_eq!(s.blocked_by, vec!["0001", "0002"]);
        let s = p.depend(TicketId(3), &[TicketId(2)], true).unwrap();
        assert_eq!(s.blocked_by, vec!["0001"]);
        let journal = fs::read_to_string(p.find(TicketId(3)).unwrap().path.join(JOURNAL_FILE)).unwrap();
        assert!(journal.contains("Retrait : 0002. Bloqué par : 0001."), "{journal}");
        assert!(messages(&p).is_empty());
        // Après retrait, le cycle n'existe plus : 0001 peut dépendre de 0003.
        p.depend(TicketId(1), &[TicketId(3)], false).unwrap_err();
        p.depend(TicketId(3), &[TicketId(1)], true).unwrap();
        p.depend(TicketId(1), &[TicketId(3)], false).unwrap();
    }

    #[test]
    fn dependance_fermee_ne_bloque_plus() {
        let (_tmp, p) = project();
        new(&p, "Un", &[]).unwrap();
        new(&p, "Deux", &[]).unwrap();
        new(&p, "Trois", &[1, 2]).unwrap();
        p.set_status(TicketId(1), Status::Done, None).unwrap();
        p.set_status(TicketId(2), Status::Cancelled, None).unwrap();
        let ctx = p.context(TicketId(3)).unwrap();
        assert_eq!(ctx.summary.blocked_by, vec!["0001", "0002"]);
        assert!(ctx.summary.open_blockers.is_empty());
        p.set_status(TicketId(2), Status::Todo, None).unwrap();
        let list = p.list(None, None).unwrap();
        let t3 = list.iter().find(|t| t.id == "0003").unwrap();
        assert_eq!(t3.open_blockers, vec!["0002"]);
    }

    #[test]
    fn validation_signale_inconnus_auto_references_et_cycles() {
        let (_tmp, p) = project();
        for t in ["Un", "Deux", "Trois", "Quatre"] {
            new(&p, t, &[]).unwrap();
        }
        force_deps(&p, 1, &[2]);
        force_deps(&p, 2, &[3]);
        force_deps(&p, 3, &[1, 8]);
        force_deps(&p, 4, &[4]);
        let msgs = messages(&p);
        assert_eq!(msgs.len(), 3, "{msgs:?}");
        assert!(msgs.iter().any(|m| m.contains("ticket 0008, introuvable") && m.contains("depend 0003 --on 0008 --remove")), "{msgs:?}");
        assert!(msgs.iter().any(|m| m.contains("0004 dépend de lui-même")), "{msgs:?}");
        assert!(msgs.iter().any(|m| m.contains("cycle de dépendances : 0001 → 0002 → 0003 → 0001")), "{msgs:?}");
        // Le retrait fonctionne même pour un id inconnu, et casse le cycle.
        p.depend(TicketId(3), &[TicketId(8), TicketId(1)], true).unwrap();
        p.depend(TicketId(4), &[TicketId(4)], true).unwrap();
        assert!(messages(&p).is_empty(), "{:?}", messages(&p));
    }

    #[test]
    fn cycles_detectes_une_seule_fois() {
        let g: BTreeMap<TicketId, Vec<TicketId>> = [
            (TicketId(1), vec![TicketId(2)]),
            (TicketId(2), vec![TicketId(1), TicketId(3)]),
            (TicketId(3), vec![TicketId(2), TicketId(3)]),
            (TicketId(4), vec![TicketId(9)]),
        ]
        .into();
        let cycles = find_cycles(&g);
        assert_eq!(cycles, vec![vec![TicketId(1), TicketId(2)], vec![TicketId(2), TicketId(3)]]);
        assert_eq!(find_path(&g, TicketId(1), TicketId(3)), Some(vec![TicketId(1), TicketId(2), TicketId(3)]));
        assert_eq!(find_path(&g, TicketId(4), TicketId(1)), None);
    }

    #[test]
    fn board_indique_les_dependances_ouvertes() {
        let (_tmp, p) = project();
        new(&p, "Un", &[]).unwrap();
        new(&p, "Deux", &[]).unwrap();
        new(&p, "Trois", &[1, 2]).unwrap();
        p.set_status(TicketId(1), Status::Done, None).unwrap();
        let board = fs::read_to_string(p.board_path()).unwrap();
        assert!(board.contains("| Bloqué par |"), "{board}");
        let row = board.lines().find(|l| l.contains("| Trois |")).unwrap();
        assert!(row.contains("[0002](../tickets/0002-deux/ticket.md)"), "{row}");
        assert!(!row.contains("0001"), "dépendance terminée affichée : {row}");
        let row = board.lines().find(|l| l.contains("| Deux |")).unwrap();
        assert!(row.contains("| — |"), "{row}");
        let done = board.lines().find(|l| l.contains("| Un |")).unwrap();
        assert_eq!(done.matches('|').count(), 7, "table des tickets fermés sans colonne Bloqué par : {done}");
    }
    #[test]
    fn rattachement_des_chemins() {
        let (_tmp, p) = project();
        let t = |n| PathOwner::Ticket(TicketId(n));
        assert_eq!(p.path_owner("", "src/main.rs"), PathOwner::Code);
        assert_eq!(p.path_owner("", "0-notes-bis/tickets/0001-un/ticket.md"), PathOwner::Code);
        assert_eq!(p.path_owner("", "0-notes/tickets/0003-trois/ticket.md"), t(3));
        assert_eq!(p.path_owner("", "0-notes/tickets/0003-trois/sous/fichier.md"), t(3));
        assert_eq!(p.path_owner("", "0-notes/0-global/BOARD.md"), PathOwner::Board);
        assert_eq!(p.path_owner("", "0-notes/0-global/README.md"), PathOwner::Notes);
        assert_eq!(p.path_owner("", "0-notes/doc/INDEX.md"), PathOwner::Notes);
        assert_eq!(p.path_owner("", "0-notes/tickets/brouillon/ticket.md"), PathOwner::Notes);
        assert_eq!(p.path_owner("", "0-notes/tickets/0003-trois"), PathOwner::Notes);
        // Projet dans un sous-dossier du dépôt : chemins git relatifs à la racine du dépôt.
        assert_eq!(p.path_owner("app/", "app/0-notes/tickets/0003-trois/journal.md"), t(3));
        assert_eq!(p.path_owner("app/", "0-notes/tickets/0003-trois/journal.md"), PathOwner::Code);
        assert_eq!(p.path_owner("app/", "app/src/lib.rs"), PathOwner::Code);
    }

    #[test]
    fn trailers_selon_le_contenu_du_commit() {
        let (_tmp, p) = project();
        let ids = |staged: &[&str]| {
            let staged: Vec<String> = staged.iter().map(|s| s.to_string()).collect();
            p.commit_tickets(TicketId(8), &staged, "").iter().map(|i| i.0).collect::<Vec<_>>()
        };
        // Notes d'autres tickets seulement : les tickets touchés, pas la branche.
        assert_eq!(
            ids(&[
                "0-notes/tickets/0002-deux/ticket.md",
                "0-notes/tickets/0001-un/journal.md",
                "0-notes/tickets/0002-deux/journal.md",
                "0-notes/0-global/BOARD.md",
                "0-notes/doc/INDEX.md",
            ]),
            vec![1, 2]
        );
        // Tout le reste revient à la branche.
        assert_eq!(ids(&["0-notes/tickets/0001-un/journal.md", "src/main.rs"]), vec![8]);
        assert_eq!(ids(&["0-notes/tickets/0001-un/journal.md", "0-notes/tickets/0008-huit/journal.md"]), vec![8]);
        assert_eq!(ids(&["0-notes/doc/architecture.md", "0-notes/0-global/BOARD.md"]), vec![8]);
        assert_eq!(ids(&["src/main.rs"]), vec![8]);
        assert_eq!(ids(&[]), vec![8]);
    }
}
