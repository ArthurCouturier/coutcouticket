//! Génération de BOARD.md. Sortie strictement déterministe (aucun horodatage),
//! pour que le fichier ne change que lorsque les tickets changent.

use std::collections::BTreeMap;
use std::fmt::Write;

use crate::model::Status;
use crate::store::{Project, Scan, Ticket};

fn cell(s: &str) -> String {
    s.replace('|', "\\|").replace('\n', " ")
}

fn row(p: &Project, t: &Ticket) -> String {
    let f = &t.doc.front;
    let id = f.id.format(p.cfg.id_width);
    let projects = if f.projects.is_empty() { "—".to_string() } else { f.projects.join(", ") };
    format!(
        "| [{id}](../tickets/{}/ticket.md) | {} | {} | {} | {} | {} |\n",
        t.dir_name,
        cell(&f.title),
        f.kind,
        f.priority,
        cell(&projects),
        f.updated.format("%Y-%m-%d")
    )
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
            list.sort_by(|a, b| (a.doc.front.priority, a.doc.front.id).cmp(&(b.doc.front.priority, b.doc.front.id)));
        } else {
            list.sort_by(|a, b| (b.doc.front.updated, b.doc.front.id).cmp(&(a.doc.front.updated, a.doc.front.id)));
        }
        let _ = writeln!(out, "## {} ({})\n", status.label(), list.len());
        if list.is_empty() {
            out.push_str("_Aucun ticket._\n\n");
            continue;
        }
        out.push_str("| ID | Titre | Type | Prio | Projets | MAJ |\n|----|-------|------|------|---------|-----|\n");
        for t in list {
            out.push_str(&row(p, t));
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
