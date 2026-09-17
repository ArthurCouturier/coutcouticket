//! Modèle de ticket et frontmatter.
//!
//! Le frontmatter est volontairement un sous-ensemble strict de YAML, parsé à la
//! main : clés connues uniquement, dans un ordre fixe à l'écriture. Cela évite une
//! dépendance YAML et rend toute dérive (clé inconnue, valeur invalide) détectable.

use std::fmt;
use std::str::FromStr;

use anyhow::{Context, Result, anyhow, bail};
use chrono::NaiveDate;
use rmcp::schemars;
use serde::{Deserialize, Serialize};

pub const FRONTMATTER_KEYS: [&str; 8] = [
    "id", "title", "type", "status", "priority", "projects", "created", "updated",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum Status {
    Todo,
    InProgress,
    Blocked,
    Review,
    Done,
    Cancelled,
}

impl Status {
    pub const ALL: [Status; 6] = [
        Status::InProgress,
        Status::Review,
        Status::Blocked,
        Status::Todo,
        Status::Done,
        Status::Cancelled,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Status::Todo => "todo",
            Status::InProgress => "in-progress",
            Status::Blocked => "blocked",
            Status::Review => "review",
            Status::Done => "done",
            Status::Cancelled => "cancelled",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Status::Todo => "À faire",
            Status::InProgress => "En cours",
            Status::Blocked => "Bloqué",
            Status::Review => "En revue",
            Status::Done => "Terminé",
            Status::Cancelled => "Annulé",
        }
    }

    pub fn is_open(self) -> bool {
        !matches!(self, Status::Done | Status::Cancelled)
    }
}

impl fmt::Display for Status {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for Status {
    type Err = anyhow::Error;
    fn from_str(s: &str) -> Result<Self> {
        Status::ALL
            .into_iter()
            .find(|st| st.as_str() == s)
            .ok_or_else(|| {
                anyhow!(
                    "statut invalide « {s} » (valeurs : todo, in-progress, blocked, review, done, cancelled)"
                )
            })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum Priority {
    P0,
    P1,
    P2,
    P3,
}

impl Priority {
    pub fn as_str(self) -> &'static str {
        match self {
            Priority::P0 => "p0",
            Priority::P1 => "p1",
            Priority::P2 => "p2",
            Priority::P3 => "p3",
        }
    }
}

impl fmt::Display for Priority {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for Priority {
    type Err = anyhow::Error;
    fn from_str(s: &str) -> Result<Self> {
        match s {
            "p0" => Ok(Priority::P0),
            "p1" => Ok(Priority::P1),
            "p2" => Ok(Priority::P2),
            "p3" => Ok(Priority::P3),
            _ => bail!("priorité invalide « {s} » (valeurs : p0, p1, p2, p3)"),
        }
    }
}

/// Identifiant numérique d'un ticket (1..=9999 avec la largeur par défaut).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TicketId(pub u32);

impl TicketId {
    /// Accepte « 13 », « 0013 » ou « #13 ».
    pub fn parse(s: &str) -> Result<Self> {
        let t = s.trim().trim_start_matches('#');
        if t.is_empty() || !t.chars().all(|c| c.is_ascii_digit()) {
            bail!("identifiant de ticket invalide « {s} » (attendu : un nombre, ex. 13 ou 0013)");
        }
        let n: u32 = t.parse().context("identifiant trop grand")?;
        if n == 0 {
            bail!("l'identifiant 0 n'existe pas, les tickets commencent à 1");
        }
        Ok(TicketId(n))
    }

    pub fn format(self, width: usize) -> String {
        format!("{:0width$}", self.0, width = width)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Frontmatter {
    pub id: TicketId,
    pub title: String,
    pub kind: String,
    pub status: Status,
    pub priority: Priority,
    pub projects: Vec<String>,
    pub created: NaiveDate,
    pub updated: NaiveDate,
}

/// Document ticket.md découpé en frontmatter + corps.
#[derive(Debug, Clone)]
pub struct TicketDoc {
    pub front: Frontmatter,
    pub body: String,
}

fn quote(s: &str) -> String {
    serde_json::to_string(s).expect("sérialisation d'une chaîne")
}

impl Frontmatter {
    pub fn render(&self, id_width: usize) -> String {
        let projects = self.projects.join(", ");
        format!(
            "---\nid: \"{}\"\ntitle: {}\ntype: {}\nstatus: {}\npriority: {}\nprojects: [{}]\ncreated: {}\nupdated: {}\n---\n",
            self.id.format(id_width),
            quote(&self.title),
            self.kind,
            self.status,
            self.priority,
            projects,
            self.created.format("%Y-%m-%d"),
            self.updated.format("%Y-%m-%d"),
        )
    }
}

impl TicketDoc {
    pub fn render(&self, id_width: usize) -> String {
        format!("{}{}", self.front.render(id_width), self.body)
    }

    pub fn parse(content: &str) -> Result<Self> {
        let content = content.strip_prefix('\u{feff}').unwrap_or(content);
        let rest = content
            .strip_prefix("---\n")
            .ok_or_else(|| anyhow!("ticket.md doit commencer par une ligne « --- »"))?;
        let end = rest
            .find("\n---\n")
            .map(|i| (i, i + 5))
            .or_else(|| rest.strip_suffix("\n---").map(|r| (r.len(), rest.len())))
            .ok_or_else(|| anyhow!("frontmatter non terminé (ligne « --- » manquante)"))?;
        let (block, body) = (&rest[..end.0], &rest[end.1..]);
        let front = parse_frontmatter_block(block)?;
        Ok(TicketDoc { front, body: body.to_string() })
    }
}

fn parse_scalar(raw: &str) -> Result<String> {
    let raw = raw.trim();
    if raw.starts_with('"') {
        serde_json::from_str::<String>(raw).map_err(|e| anyhow!("chaîne entre guillemets invalide : {e}"))
    } else if raw.starts_with('\'') && raw.ends_with('\'') && raw.len() >= 2 {
        Ok(raw[1..raw.len() - 1].replace("''", "'"))
    } else {
        Ok(raw.to_string())
    }
}

fn parse_list(raw: &str) -> Result<Vec<String>> {
    let raw = raw.trim();
    let inner = raw
        .strip_prefix('[')
        .and_then(|r| r.strip_suffix(']'))
        .ok_or_else(|| anyhow!("« projects » doit être une liste entre crochets, ex. [backend, app]"))?;
    Ok(inner
        .split(',')
        .map(|s| s.trim().trim_matches('"').trim_matches('\'').to_string())
        .filter(|s| !s.is_empty())
        .collect())
}

fn parse_date(key: &str, raw: &str) -> Result<NaiveDate> {
    let v = parse_scalar(raw)?;
    NaiveDate::parse_from_str(&v, "%Y-%m-%d")
        .map_err(|_| anyhow!("« {key} » doit être une date AAAA-MM-JJ, reçu « {v} »"))
}

fn parse_frontmatter_block(block: &str) -> Result<Frontmatter> {
    let mut map: Vec<(String, String)> = Vec::new();
    for (i, line) in block.lines().enumerate() {
        if line.trim().is_empty() || line.trim_start().starts_with('#') {
            continue;
        }
        let (k, v) = line
            .split_once(':')
            .ok_or_else(|| anyhow!("ligne {} du frontmatter invalide : « {line} »", i + 2))?;
        let k = k.trim().to_string();
        if !FRONTMATTER_KEYS.contains(&k.as_str()) {
            bail!(
                "clé inconnue « {k} » dans le frontmatter (clés autorisées : {})",
                FRONTMATTER_KEYS.join(", ")
            );
        }
        if map.iter().any(|(ek, _)| *ek == k) {
            bail!("clé « {k} » présente deux fois dans le frontmatter");
        }
        map.push((k, v.to_string()));
    }
    let get = |key: &str| -> Result<&str> {
        map.iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
            .ok_or_else(|| anyhow!("clé obligatoire « {key} » absente du frontmatter"))
    };
    let title = parse_scalar(get("title")?)?;
    if title.trim().is_empty() {
        bail!("« title » ne peut pas être vide");
    }
    let kind = parse_scalar(get("type")?)?;
    if kind.is_empty() {
        bail!("« type » ne peut pas être vide");
    }
    Ok(Frontmatter {
        id: TicketId::parse(&parse_scalar(get("id")?)?)?,
        title,
        kind,
        status: parse_scalar(get("status")?)?.parse()?,
        priority: parse_scalar(get("priority")?)?.parse()?,
        projects: parse_list(get("projects")?)?,
        created: parse_date("created", get("created")?)?,
        updated: parse_date("updated", get("updated")?)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> TicketDoc {
        TicketDoc {
            front: Frontmatter {
                id: TicketId(13),
                title: "Ajouter le refresh: \"silencieux\"".into(),
                kind: "feat".into(),
                status: Status::InProgress,
                priority: Priority::P1,
                projects: vec!["backend".into(), "app".into()],
                created: NaiveDate::from_ymd_opt(2026, 9, 17).unwrap(),
                updated: NaiveDate::from_ymd_opt(2026, 9, 18).unwrap(),
            },
            body: "\n# Description\n\nTexte.\n".into(),
        }
    }

    #[test]
    fn roundtrip() {
        let doc = sample();
        let text = doc.render(4);
        assert!(text.contains("id: \"0013\""));
        let parsed = TicketDoc::parse(&text).unwrap();
        assert_eq!(parsed.front, doc.front);
        assert_eq!(parsed.body, doc.body);
    }

    #[test]
    fn rejects_unknown_key() {
        let text = sample().render(4).replace("priority: p1", "priority: p1\nowner: moi");
        let err = TicketDoc::parse(&text).unwrap_err().to_string();
        assert!(err.contains("clé inconnue"), "{err}");
    }

    #[test]
    fn rejects_bad_status() {
        let text = sample().render(4).replace("in-progress", "en cours");
        assert!(TicketDoc::parse(&text).is_err());
    }

    #[test]
    fn parses_ids() {
        assert_eq!(TicketId::parse("0013").unwrap(), TicketId(13));
        assert_eq!(TicketId::parse("#13").unwrap(), TicketId(13));
        assert!(TicketId::parse("0").is_err());
        assert!(TicketId::parse("abc").is_err());
    }
}
