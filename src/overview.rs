//! Vue des tickets ouverts de tous les projets enregistrés (`overview`,
//! `tickets_overview`, `OVERVIEW.md`). Lecture seule, sans verrou : chaque projet
//! est lu par `Project::scan`, comme pour `list`.
//!
//! Un projet illisible (introuvable, config ou notes invalides) ne fait jamais
//! échouer la vue : il est signalé dans `warnings`, avec la correction.

use std::fmt::Write;
use std::path::{Path, PathBuf};

use anyhow::{Result, bail};
use serde::Serialize;

use crate::config::{CONFIG_FILE, global_dir};
use crate::fsutil;
use crate::model::{Priority, Status};
use crate::store::{Project, TICKET_FILE, TicketSummary};

pub const OVERVIEW_FILE: &str = "OVERVIEW.md";

#[derive(Debug, Clone, Copy, Default)]
pub struct Filter {
    /// Statut ouvert uniquement (la vue ne montre jamais les tickets fermés).
    pub status: Option<Status>,
    pub priority: Option<Priority>,
}

#[derive(Debug, Clone, Serialize)]
pub struct OverviewTicket {
    /// Nom du projet (dossier racine).
    pub project: String,
    /// Chemin absolu de la racine du projet (paramètre `project` des outils `ticket_*`).
    pub project_path: String,
    #[serde(flatten)]
    pub ticket: TicketSummary,
}

#[derive(Debug, Clone, Serialize)]
pub struct Warning {
    pub project_path: String,
    pub message: String,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct Overview {
    /// Projets enregistrés.
    pub projects: usize,
    /// Tickets ouverts, triés par statut (en cours, en revue, bloqué, à faire),
    /// priorité, projet puis id.
    pub tickets: Vec<OverviewTicket>,
    pub warnings: Vec<Warning>,
}

/// Rang d'un statut dans la vue : l'ordre de `Status::ALL` (en cours, en revue, bloqué, à faire).
fn rank(s: Status) -> usize {
    Status::ALL.iter().position(|x| *x == s).unwrap_or(usize::MAX)
}

/// Agrège les tickets ouverts des projets `roots` (le registre).
pub fn build(roots: &[PathBuf], filter: Filter) -> Result<Overview> {
    if let Some(s) = filter.status
        && !s.is_open()
    {
        bail!("la vue globale ne montre que les tickets ouverts : choisir un statut parmi in-progress, review, blocked, todo");
    }
    let mut out = Overview { projects: roots.len(), ..Default::default() };
    for root in roots {
        let path = root.display().to_string();
        let warn = |message: String| Warning { project_path: path.clone(), message };
        if !root.join(CONFIG_FILE).is_file() {
            out.warnings.push(warn(format!(
                "projet introuvable ({CONFIG_FILE} absent) : le retirer avec « coutcouticket projects remove {path} », ou relancer « coutcouticket init » s'il a été déplacé"
            )));
            continue;
        }
        let project = match Project::open(root) {
            Ok(p) => p,
            Err(e) => {
                out.warnings.push(warn(format!("configuration illisible : {e:#}")));
                continue;
            }
        };
        if !project.tickets_dir().is_dir() {
            out.warnings.push(warn(format!(
                "{} absent : lancer « coutcouticket -C {path} init »",
                project.rel(&project.tickets_dir())
            )));
            continue;
        }
        let scan = match project.scan() {
            Ok(s) => s,
            Err(e) => {
                out.warnings.push(warn(format!("notes illisibles : {e:#}")));
                continue;
            }
        };
        if !scan.problems.is_empty() {
            out.warnings.push(warn(format!(
                "{} problème(s) dans les notes, tickets concernés absents de la vue : lancer « coutcouticket -C {path} validate »",
                scan.problems.len()
            )));
        }
        let name = project.name();
        for t in &scan.tickets {
            let f = &t.doc.front;
            if !f.status.is_open()
                || filter.status.is_some_and(|s| s != f.status)
                || filter.priority.is_some_and(|p| p != f.priority)
            {
                continue;
            }
            out.tickets.push(OverviewTicket {
                project: name.clone(),
                project_path: path.clone(),
                ticket: project.summary(t, &scan.tickets),
            });
        }
    }
    out.tickets.sort_by(|a, b| {
        let key = |t: &OverviewTicket| {
            (rank(t.ticket.status), t.ticket.priority, t.project.clone(), t.project_path.clone(), t.ticket.id.clone())
        };
        key(a).cmp(&key(b))
    });
    Ok(out)
}

fn cell(s: &str) -> String {
    s.replace('|', "\\|").replace('\n', " ")
}

fn open_statuses() -> impl Iterator<Item = Status> {
    Status::ALL.into_iter().filter(|s| s.is_open())
}

/// Rendu texte de la CLI, groupé par statut.
pub fn render_text(o: &Overview) -> String {
    let mut out = String::new();
    if o.projects == 0 {
        out.push_str("Aucun projet enregistré : lancer « coutcouticket init » dans un projet.\n");
    } else if o.tickets.is_empty() {
        let _ = writeln!(out, "Aucun ticket ouvert dans {} projet(s).", o.projects);
    } else {
        let width = o.tickets.iter().map(|t| t.project.chars().count()).max().unwrap_or(0);
        let _ = writeln!(out, "{} ticket(s) ouvert(s) dans {} projet(s).", o.tickets.len(), o.projects);
        for status in open_statuses() {
            let list: Vec<&OverviewTicket> = o.tickets.iter().filter(|t| t.ticket.status == status).collect();
            if list.is_empty() {
                continue;
            }
            let _ = writeln!(out, "\n{} ({})", status.label(), list.len());
            for t in list {
                let s = &t.ticket;
                let blockers =
                    if s.open_blockers.is_empty() { String::new() } else { format!("  (bloqué par {})", s.open_blockers.join(", ")) };
                let _ = writeln!(out, "  {:<width$}  {}  {} {:<7} {}{}", t.project, s.id, s.priority, s.kind, s.title, blockers);
            }
        }
    }
    if !o.warnings.is_empty() {
        out.push('\n');
        for w in &o.warnings {
            let _ = writeln!(out, "⚠️ {} : {}", w.project_path, w.message);
        }
    }
    out
}

/// Lien Markdown vers un chemin absolu (chevrons : les espaces restent valides).
/// Sous Windows, séparateurs `/` (`C:/Users/…`) : les visionneuses Markdown les suivent.
fn link(text: &str, path: &Path) -> String {
    format!("[{text}](<{}>)", link_path(path))
}

fn link_path(path: &Path) -> String {
    let p = path.display().to_string();
    if cfg!(windows) { p.replace('\\', "/") } else { p }
}

/// Rendu de `OVERVIEW.md`. Strictement déterministe (aucun horodatage) : le
/// fichier ne change que lorsque les tickets ou le registre changent.
pub fn render_markdown(o: &Overview) -> String {
    let mut out = String::new();
    out.push_str("<!-- GÉNÉRÉ par le démon coutcouticket — ne pas éditer. Vue en direct : coutcouticket overview -->\n");
    out.push_str("# Tickets ouverts — tous les projets\n\n");
    let count = |s: Status| o.tickets.iter().filter(|t| t.ticket.status == s).count();
    let counts: Vec<String> = open_statuses().map(|s| format!("{} {}", count(s), s.label().to_lowercase())).collect();
    let _ = writeln!(out, "**{} ouvert(s)** dans {} projet(s) · {}\n", o.tickets.len(), o.projects, counts.join(" · "));

    if !o.warnings.is_empty() {
        out.push_str("## ⚠️ Projets à vérifier\n\n");
        for w in &o.warnings {
            let _ = writeln!(out, "- `{}` : {}", w.project_path, cell(&w.message));
        }
        out.push('\n');
    }

    for status in open_statuses() {
        let list: Vec<&OverviewTicket> = o.tickets.iter().filter(|t| t.ticket.status == status).collect();
        let _ = writeln!(out, "## {} ({})\n", status.label(), list.len());
        if list.is_empty() {
            out.push_str("_Aucun ticket._\n\n");
            continue;
        }
        out.push_str("| Projet | ID | Titre | Type | Prio | Bloqué par |\n|--------|----|-------|------|------|------------|\n");
        for t in list {
            let s = &t.ticket;
            let ticket = Path::new(&t.project_path).join(&s.dir).join(TICKET_FILE);
            let blockers = if s.open_blockers.is_empty() { "—".to_string() } else { s.open_blockers.join(", ") };
            let _ = writeln!(
                out,
                "| {} | {} | {} | {} | {} | {} |",
                cell(&t.project),
                link(&s.id, &ticket),
                cell(&s.title),
                s.kind,
                s.priority,
                blockers
            );
        }
        out.push('\n');
    }
    out
}

pub fn file_path() -> Result<PathBuf> {
    Ok(global_dir()?.join(OVERVIEW_FILE))
}

/// Régénère `OVERVIEW.md` dans le dossier de config globale. Retourne true s'il a changé.
pub fn regenerate_file(roots: &[PathBuf]) -> Result<bool> {
    let overview = build(roots, Filter::default())?;
    fsutil::write_if_changed(&file_path()?, &render_markdown(&overview))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use crate::model::TicketId;
    use crate::store::CreateInput;
    use std::fs;

    fn project(base: &Path, name: &str) -> Project {
        let root = base.join(name);
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join(CONFIG_FILE), Config::default_file_content()).unwrap();
        let p = Project::open(&root).unwrap();
        fs::create_dir_all(p.tickets_dir()).unwrap();
        fs::create_dir_all(p.global_dir()).unwrap();
        p
    }

    fn new(p: &Project, title: &str, priority: Priority, deps: &[u32]) {
        p.create(CreateInput {
            title: title.into(),
            priority: Some(priority),
            blocked_by: deps.iter().map(|n| TicketId(*n)).collect(),
            ..Default::default()
        })
        .unwrap();
    }

    #[test]
    fn agrege_trie_et_signale_les_projets_illisibles() {
        let tmp = tempfile::tempdir().unwrap();
        let a = project(tmp.path(), "alpha");
        let b = project(tmp.path(), "beta");
        new(&a, "A un", Priority::P2, &[]);
        new(&a, "A deux", Priority::P0, &[1]);
        new(&a, "A fini", Priority::P0, &[]);
        a.set_status(TicketId(3), Status::Done, None).unwrap();
        new(&b, "B un", Priority::P1, &[]);
        new(&b, "B deux", Priority::P3, &[]);
        new(&b, "B trois", Priority::P2, &[]);
        b.set_status(TicketId(2), Status::InProgress, None).unwrap();
        b.set_status(TicketId(3), Status::Review, None).unwrap();
        a.set_status(TicketId(1), Status::Blocked, None).unwrap();
        // Ticket invalide dans beta : signalé, le reste du projet reste visible.
        fs::create_dir_all(b.tickets_dir().join("brouillon")).unwrap();
        let broken = project(tmp.path(), "casse");
        fs::write(broken.root.join(CONFIG_FILE), "cle_inconnue = 1\n").unwrap();
        let missing = tmp.path().join("disparu");

        let roots = vec![a.root.clone(), missing.clone(), b.root.clone(), broken.root.clone()];
        let o = build(&roots, Filter::default()).unwrap();
        assert_eq!(o.projects, 4);
        let got: Vec<(String, String)> = o.tickets.iter().map(|t| (t.project.clone(), t.ticket.id.clone())).collect();
        let want = [("beta", "0002"), ("beta", "0003"), ("alpha", "0001"), ("alpha", "0002"), ("beta", "0001")];
        assert_eq!(got, want.map(|(p, i)| (p.to_string(), i.to_string())));
        let a2 = &o.tickets[3];
        assert_eq!(a2.ticket.open_blockers, vec!["0001"]);
        assert_eq!(a2.project_path, a.root.display().to_string());

        let warnings: Vec<(String, String)> = o.warnings.iter().map(|w| (w.project_path.clone(), w.message.clone())).collect();
        assert_eq!(warnings.len(), 3, "{warnings:?}");
        assert!(warnings[0].0.ends_with("disparu") && warnings[0].1.contains("introuvable") && warnings[0].1.contains("projects remove"), "{warnings:?}");
        assert!(warnings[1].1.contains("1 problème(s)") && warnings[1].1.contains("validate"), "{warnings:?}");
        assert!(warnings[2].0.ends_with("casse") && warnings[2].1.contains("configuration illisible"), "{warnings:?}");

        let text = render_text(&o);
        assert!(text.contains("5 ticket(s) ouvert(s) dans 4 projet(s)."), "{text}");
        assert!(text.find("En cours (1)").unwrap() < text.find("À faire (2)").unwrap(), "{text}");
        assert!(text.contains("alpha  0002  p0 feat    A deux  (bloqué par 0001)"), "{text}");
        assert!(text.contains("⚠️ ") && text.contains("disparu"), "{text}");
    }

    #[test]
    fn filtres_et_statut_ferme_refuse() {
        let tmp = tempfile::tempdir().unwrap();
        let a = project(tmp.path(), "alpha");
        new(&a, "Un", Priority::P1, &[]);
        new(&a, "Deux", Priority::P2, &[]);
        a.set_status(TicketId(2), Status::InProgress, None).unwrap();
        let roots = vec![a.root.clone()];
        let only = |filter| build(&roots, filter).unwrap().tickets.iter().map(|t| t.ticket.id.clone()).collect::<Vec<_>>();
        assert_eq!(only(Filter { status: Some(Status::Todo), priority: None }), vec!["0001"]);
        assert_eq!(only(Filter { status: None, priority: Some(Priority::P2) }), vec!["0002"]);
        assert!(only(Filter { status: Some(Status::Review), priority: None }).is_empty());
        let err = build(&roots, Filter { status: Some(Status::Done), priority: None }).unwrap_err().to_string();
        assert!(err.contains("tickets ouverts"), "{err}");
        assert!(render_text(&build(&[], Filter::default()).unwrap()).contains("Aucun projet enregistré"));
    }

    #[test]
    fn markdown_deterministe() {
        let tmp = tempfile::tempdir().unwrap();
        let a = project(tmp.path(), "al pha");
        new(&a, "Titre | avec barre", Priority::P1, &[]);
        let roots = vec![a.root.clone(), tmp.path().join("disparu")];
        let md = render_markdown(&build(&roots, Filter::default()).unwrap());
        assert_eq!(md, render_markdown(&build(&roots, Filter::default()).unwrap()));
        assert!(md.contains("**1 ouvert(s)** dans 2 projet(s)"), "{md}");
        assert!(md.contains("## ⚠️ Projets à vérifier"), "{md}");
        let ticket = a.root.join("0-notes/tickets/0001-titre-avec-barre/ticket.md");
        let row = md.lines().find(|l| l.contains("Titre \\| avec barre")).expect(&md);
        assert!(row.contains(&format!("[0001](<{}>)", link_path(&ticket))), "{row}");
        assert!(row.starts_with("| al pha | [0001]"), "{row}");
        assert!(md.contains("## En cours (0)\n\n_Aucun ticket._"), "{md}");
    }
}
