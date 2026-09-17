//! Génération de BOARD.md. Sortie strictement déterministe (aucun horodatage),
//! pour que le fichier ne change que lorsque les tickets changent.

use std::collections::BTreeMap;
use std::fmt::Write;

use crate::model::Status;
use crate::store::{Project, Scan, Ticket};

fn cell(s: &str) -> String {
    s.replace('|', "\\|").replace('\n', " ")
}

/// `blockers` : `Some` pour les tickets ouverts (colonne « Bloqué par »), `None` sinon.
fn row(p: &Project, t: &Ticket, blockers: Option<String>) -> String {
    let f = &t.doc.front;
    let id = f.id.format(p.cfg.id_width);
    let projects = if f.projects.is_empty() { "—".to_string() } else { f.projects.join(", ") };
    let blockers = blockers.map(|b| format!(" {b} |")).unwrap_or_default();
    format!(
        "| [{id}](../tickets/{}/ticket.md) | {} | {} | {} | {} |{blockers} {} |\n",
        t.dir_name,
        cell(&f.title),
        f.kind,
        f.priority,
        cell(&projects),
        f.updated.format("%Y-%m-%d")
    )
}

/// Dépendances encore ouvertes, en liens vers leur ticket. Une dépendance
/// terminée ou annulée ne bloque plus et n'apparaît pas.
fn blockers_cell(p: &Project, t: &Ticket, scan: &Scan) -> String {
    let links: Vec<String> = p
        .open_blockers(t, &scan.tickets)
        .iter()
        .filter_map(|id| scan.tickets.iter().find(|o| o.doc.front.id == *id))
        .map(|o| format!("[{}](../tickets/{}/ticket.md)", o.doc.front.id.format(p.cfg.id_width), o.dir_name))
        .collect();
    if links.is_empty() { "—".into() } else { links.join(", ") }
}

pub fn render(p: &Project, scan: &Scan) -> String {
    let mut out = String::new();
    out.push_str("<!-- GÉNÉRÉ par coutcouticket — ne pas éditer. Régénérer : coutcouticket board -->\n");
    let _ = writeln!(out, "# Tableau des tickets — {}\n", p.name());

    let mut by_status: BTreeMap<usize, Vec<&Ticket>> = BTreeMap::new();
    for t in &scan.tickets {
        let idx = Status::ALL.iter().position(|s| *s == t.doc.front.status).unwrap_or(0);
        by_status.entry(idx).or_default().push(t);
    }
    let count = |s: Status| scan.tickets.iter().filter(|t| t.doc.front.status == s).count();
    let open = scan.tickets.iter().filter(|t| t.doc.front.status.is_open()).count();
    let counts: Vec<String> = Status::ALL.iter().map(|s| format!("{} {}", count(*s), s.label().to_lowercase())).collect();
    let _ = writeln!(out, "**{open} ouvert(s)** sur {} · {}\n", scan.tickets.len(), counts.join(" · "));

    if !scan.problems.is_empty() {
        out.push_str("## ⚠️ Problèmes détectés\n\n");
        for pb in &scan.problems {
            let _ = writeln!(out, "- `{}` : {}", pb.path, cell(&pb.message));
        }
        out.push_str("\nVérifier avec `coutcouticket validate`.\n\n");
    }

    for (idx, status) in Status::ALL.iter().enumerate() {
        let mut list: Vec<&Ticket> = by_status.get(&idx).cloned().unwrap_or_default();
        if status.is_open() {
            list.sort_by_key(|a| (a.doc.front.priority, a.doc.front.id));
        } else {
            list.sort_by_key(|a| std::cmp::Reverse((a.doc.front.updated, a.doc.front.id)));
        }
        let _ = writeln!(out, "## {} ({})\n", status.label(), list.len());
        if list.is_empty() {
            out.push_str("_Aucun ticket._\n\n");
            continue;
        }
        if status.is_open() {
            out.push_str("| ID | Titre | Type | Prio | Projets | Bloqué par | MAJ |\n|----|-------|------|------|---------|------------|-----|\n");
        } else {
            out.push_str("| ID | Titre | Type | Prio | Projets | MAJ |\n|----|-------|------|------|---------|-----|\n");
        }
        for t in list {
            let blockers = status.is_open().then(|| blockers_cell(p, t, scan));
            out.push_str(&row(p, t, blockers));
        }
        out.push('\n');
    }

    let mut projects: BTreeMap<String, [usize; 6]> = BTreeMap::new();
    for t in &scan.tickets {
        let idx = Status::ALL.iter().position(|s| *s == t.doc.front.status).unwrap_or(0);
        let names = if t.doc.front.projects.is_empty() { vec!["(sans projet)".to_string()] } else { t.doc.front.projects.clone() };
        for name in names {
            projects.entry(name).or_default()[idx] += 1;
        }
    }
    out.push_str("## Par projet de dev\n\n");
    if projects.is_empty() {
        out.push_str("_Aucun ticket._\n");
    } else {
        let header: Vec<&str> = Status::ALL.iter().map(|s| s.label()).collect();
        let _ = writeln!(out, "| Projet | {} |", header.join(" | "));
        let _ = writeln!(out, "|--------|{}", "---|".repeat(header.len()));
        for (name, c) in projects {
            let cells: Vec<String> = c.iter().map(|n| n.to_string()).collect();
            let _ = writeln!(out, "| {} | {} |", cell(&name), cells.join(" | "));
        }
    }
    out
}
