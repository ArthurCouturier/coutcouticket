//! Conventions de nommage : slug, dossier de ticket, branche git.
//!
//! Dossier : `0013-add-thing-to-etc`
//! Branche : `feat/0013-add-thing-to-etc`
//! Le suffixe de la branche est STRICTEMENT le nom du dossier.

use anyhow::{Result, bail};

use crate::config::Config;
use crate::model::TicketId;

fn fold_char(c: char) -> &'static str {
    match c {
        'à' | 'á' | 'â' | 'ä' | 'ã' | 'å' => "a",
        'ç' => "c",
        'è' | 'é' | 'ê' | 'ë' => "e",
        'ì' | 'í' | 'î' | 'ï' => "i",
        'ñ' => "n",
        'ò' | 'ó' | 'ô' | 'ö' | 'õ' => "o",
        'ù' | 'ú' | 'û' | 'ü' => "u",
        'ý' | 'ÿ' => "y",
        'œ' => "oe",
        'æ' => "ae",
        _ => "",
    }
}

/// Transforme un titre en slug ASCII `[a-z0-9-]`, tronqué proprement sur un tiret.
pub fn slugify(title: &str, max_len: usize) -> String {
    let mut out = String::new();
    let mut last_dash = true;
    for c in title.to_lowercase().chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c);
            last_dash = false;
        } else {
            let folded = fold_char(c);
            if !folded.is_empty() {
                out.push_str(folded);
                last_dash = false;
            } else if !last_dash {
                out.push('-');
                last_dash = true;
            }
        }
    }
    let mut slug = out.trim_matches('-').to_string();
    if slug.len() > max_len {
        let cut = &slug[..max_len];
        slug = match cut.rfind('-') {
            Some(i) if i > 0 => cut[..i].to_string(),
            _ => cut.to_string(),
        };
    }
    slug
}

pub fn is_valid_slug(s: &str) -> bool {
    !s.is_empty()
        && !s.starts_with('-')
        && !s.ends_with('-')
        && !s.contains("--")
        && s.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

pub fn dir_name(id: TicketId, slug: &str, cfg: &Config) -> String {
    format!("{}-{}", id.format(cfg.id_width), slug)
}

/// Découpe un nom de dossier `0013-slug` ; retourne None s'il ne respecte pas le format.
pub fn parse_dir_name(name: &str, cfg: &Config) -> Option<(TicketId, String)> {
    let (num, slug) = name.split_once('-')?;
    if num.len() != cfg.id_width || !num.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    if !is_valid_slug(slug) {
        return None;
    }
    let id = TicketId::parse(num).ok()?;
    Some((id, slug.to_string()))
}

pub fn branch_name(kind: &str, dir: &str) -> String {
    format!("{kind}/{dir}")
}

#[derive(Debug, PartialEq, Eq)]
pub enum BranchKind {
    /// Branche exemptée (main, develop, ...).
    Exempt,
    /// Branche de ticket conforme.
    Ticket { kind: String, id: TicketId, dir: String },
}

/// Analyse une branche selon la convention stricte. Erreur explicite si non conforme.
pub fn classify_branch(branch: &str, cfg: &Config) -> Result<BranchKind> {
    if cfg.exempt_branches.iter().any(|b| b == branch) {
        return Ok(BranchKind::Exempt);
    }
    let example = format!("{}/{}-add-thing-to-etc", cfg.branch_types[0], TicketId(13).format(cfg.id_width));
    let Some((kind, dir)) = branch.split_once('/') else {
        bail!(
            "branche « {branch} » non conforme : format attendu <type>/<id>-<slug>, ex. {example} (types : {})",
            cfg.branch_types.join(", ")
        );
    };
    if !cfg.branch_types.iter().any(|t| t == kind) {
        bail!(
            "branche « {branch} » : type « {kind} » non autorisé (types : {})",
            cfg.branch_types.join(", ")
        );
    }
    let Some((id, _)) = parse_dir_name(dir, cfg) else {
        bail!(
            "branche « {branch} » non conforme : après « {kind}/ » il faut {} chiffres, un tiret et un slug en minuscules, ex. {example}",
            cfg.id_width
        );
    };
    Ok(BranchKind::Ticket { kind: kind.to_string(), id, dir: dir.to_string() })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slug_french() {
        assert_eq!(slugify("Gérer l'écran « Mes plantes » !", 50), "gerer-l-ecran-mes-plantes");
        assert_eq!(slugify("  Add thing to etc  ", 50), "add-thing-to-etc");
    }

    #[test]
    fn slug_truncates_on_dash() {
        assert_eq!(slugify("alpha beta gamma delta", 13), "alpha-beta");
    }

    #[test]
    fn branches() {
        let cfg = Config::default();
        assert_eq!(classify_branch("main", &cfg).unwrap(), BranchKind::Exempt);
        assert_eq!(
            classify_branch("feat/0013-add-thing-to-etc", &cfg).unwrap(),
            BranchKind::Ticket { kind: "feat".into(), id: TicketId(13), dir: "0013-add-thing-to-etc".into() }
        );
        assert!(classify_branch("feat/13-add-thing", &cfg).is_err());
        assert!(classify_branch("feature/0013-add-thing", &cfg).is_err());
        assert!(classify_branch("feat/0013-Add_Thing", &cfg).is_err());
        assert!(classify_branch("wip", &cfg).is_err());
    }
}
