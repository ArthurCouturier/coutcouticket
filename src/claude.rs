//! Enregistrement du serveur MCP du démon dans Claude Code (`setup-claude`).
//!
//! `claude` n'offre pas de sortie JSON pour `mcp get` : on lit sa sortie texte.
//! Le binaire est surchargeable par `COUTCOUTICKET_CLAUDE_BIN` (tests).

use std::ffi::OsString;
use std::process::{Command, Output};

use anyhow::{Result, bail};

use crate::config::DaemonConfig;

pub const SERVER_NAME: &str = "coutcouticket";
const MAX_STEPS: usize = 6;

/// Enregistrement d'un serveur MCP tel que vu par Claude Code.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Registration {
    pub transport: String,
    pub url: String,
    pub headers: Vec<(String, String)>,
    /// Portée (`user`, `local`, `project`), si elle a pu être lue.
    pub scope: Option<String>,
}

impl Registration {
    /// Enregistrement attendu, d'après la config du démon.
    pub fn wanted() -> Result<Registration> {
        let cfg = DaemonConfig::load_or_create()?;
        Ok(Registration {
            transport: "http".into(),
            url: format!("http://127.0.0.1:{}/mcp", cfg.port),
            headers: vec![("Authorization".into(), format!("Bearer {}", cfg.token))],
            scope: Some("user".into()),
        })
    }

    /// Même serveur effectif (transport, URL, en-têtes), quelle que soit la portée.
    pub fn same_target(&self, other: &Registration) -> bool {
        let mut a = self.headers.clone();
        let mut b = other.headers.clone();
        a.sort();
        b.sort();
        self.transport.eq_ignore_ascii_case(&other.transport) && self.url == other.url && a == b
    }

    /// Arguments de `claude mcp add` pour cet enregistrement (portée utilisateur).
    pub fn add_args(&self) -> Vec<String> {
        let mut args: Vec<String> = ["mcp", "add", "--transport", &self.transport, "--scope", "user", SERVER_NAME, &self.url]
            .iter()
            .map(|s| s.to_string())
            .collect();
        for (k, v) in &self.headers {
            args.push("--header".into());
            args.push(format!("{k}: {v}"));
        }
        args
    }
}

/// Lit la sortie texte de `claude mcp get <nom>`. `None` si elle n'indique pas de type.
/// Serveur stdio : pas d'URL, `url` reste vide (donc différent de l'attendu).
pub fn parse_get(text: &str) -> Option<Registration> {
    let mut transport = None;
    let mut url = None;
    let mut headers = Vec::new();
    let mut scope_label = None;
    let mut scope_flag = None;
    let mut headers_indent: Option<usize> = None;
    for line in text.lines() {
        let indent = line.len() - line.trim_start().len();
        let t = line.trim();
        if let Some(hi) = headers_indent {
            if !t.is_empty() && indent > hi {
                if let Some((k, v)) = t.split_once(':') {
                    headers.push((k.trim().to_string(), v.trim().to_string()));
                }
                continue;
            }
            headers_indent = None;
        }
        if t == "Headers:" {
            headers_indent = Some(indent);
        } else if let Some(v) = t.strip_prefix("Type:") {
            transport = Some(v.trim().to_string());
        } else if let Some(v) = t.strip_prefix("URL:") {
            url = Some(v.trim().to_string());
        } else if let Some(v) = t.strip_prefix("Scope:") {
            scope_label = Some(v.trim().to_lowercase());
        } else if let Some(pos) = t.find("claude mcp remove") {
            // « To remove this server, run: claude mcp remove <nom> -s <portée> »
            let words: Vec<&str> = t[pos..].split_whitespace().collect();
            scope_flag = words
                .windows(2)
                .find(|w| w[0] == "-s" || w[0] == "--scope")
                .map(|w| w[1].to_string());
        }
    }
    let scope = scope_flag.or_else(|| {
        let label = scope_label?;
        ["user", "local", "project"].into_iter().find(|s| label.starts_with(s)).map(String::from)
    });
    Some(Registration { transport: transport?, url: url.unwrap_or_default(), headers, scope })
}

/// Résultat de `setup-claude --apply`.
#[derive(Debug, PartialEq, Eq)]
pub enum Outcome {
    /// Déjà enregistré à l'identique : rien n'a été lancé.
    UpToDate { scope: Option<String> },
    /// Enregistrement créé ; `removed` liste les portées des anciens enregistrements retirés.
    Registered { removed: Vec<String> },
}

fn claude_bin() -> OsString {
    std::env::var_os("COUTCOUTICKET_CLAUDE_BIN").unwrap_or_else(|| "claude".into())
}

fn run_claude(args: &[String]) -> Result<Output> {
    match Command::new(claude_bin()).args(args).output() {
        Ok(o) => Ok(o),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => bail!(
            "binaire « claude » introuvable. Installer Claude Code (ou définir COUTCOUTICKET_CLAUDE_BIN avec son chemin), puis relancer « coutcouticket setup-claude --apply »."
        ),
        Err(e) => bail!("impossible de lancer « claude » : {e}"),
    }
}

/// Sortie de `claude`, jetons masqués (ancien comme nouveau).
fn claude_output(o: &Output) -> String {
    let text = format!("{}{}", String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr));
    mask_tokens(text.trim())
}

fn mask_tokens(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(pos) = rest.find("Bearer ") {
        let (head, tail) = rest.split_at(pos + "Bearer ".len());
        out.push_str(head);
        out.push_str("<jeton>");
        rest = tail.trim_start_matches(|c: char| !c.is_whitespace() && c != '"' && c != '\'');
    }
    out.push_str(rest);
    out
}

/// Enregistrement actuel du serveur, ou `None` s'il est absent.
fn current() -> Result<Option<Registration>> {
    let out = run_claude(&["mcp".into(), "get".into(), SERVER_NAME.into()])?;
    if !out.status.success() {
        // « No MCP server named … » : code 1.
        return Ok(None);
    }
    match parse_get(&String::from_utf8_lossy(&out.stdout)) {
        Some(reg) => Ok(Some(reg)),
        None => bail!(
            "enregistrement « {SERVER_NAME} » illisible dans la sortie de « claude mcp get {SERVER_NAME} » :\n{}\nLe retirer avec « claude mcp remove {SERVER_NAME} », puis relancer « coutcouticket setup-claude --apply ».",
            claude_output(&out)
        ),
    }
}

/// Enregistre le démon dans Claude Code sans jamais échouer sur un enregistrement
/// existant : identique, il est laissé tel quel ; différent (jeton, port, transport),
/// il est retiré de sa portée puis recréé en portée utilisateur. Une portée `local`
/// ou `project` masque la portée utilisateur : elle est donc retirée aussi.
pub fn apply() -> Result<Outcome> {
    let wanted = Registration::wanted()?;
    let mut removed = Vec::new();
    let mut added = false;
    for _ in 0..MAX_STEPS {
        match current()? {
            Some(cur) if cur.same_target(&wanted) => {
                return Ok(if added || !removed.is_empty() {
                    Outcome::Registered { removed }
                } else {
                    Outcome::UpToDate { scope: cur.scope }
                });
            }
            Some(cur) => {
                let mut args: Vec<String> = vec!["mcp".into(), "remove".into(), SERVER_NAME.into()];
                if let Some(s) = &cur.scope {
                    args.extend(["--scope".into(), s.clone()]);
                }
                let out = run_claude(&args)?;
                if !out.status.success() {
                    bail!(
                        "impossible de retirer l'ancien enregistrement « {SERVER_NAME} » (portée {}) :\n{}\nCorriger la cause ci-dessus, puis relancer « coutcouticket setup-claude --apply ».",
                        cur.scope.as_deref().unwrap_or("inconnue"),
                        claude_output(&out)
                    );
                }
                removed.push(cur.scope.unwrap_or_else(|| "inconnue".into()));
            }
            None if added => bail!(
                "« claude mcp add » a réussi mais « claude mcp get {SERVER_NAME} » ne voit pas le serveur. Vérifier avec « claude mcp list », puis relancer « coutcouticket setup-claude --apply »."
            ),
            None => {
                let out = run_claude(&wanted.add_args())?;
                if !out.status.success() {
                    bail!(
                        "« claude mcp add » a échoué :\n{}\nCorriger la cause ci-dessus (état actuel : « claude mcp get {SERVER_NAME} »), puis relancer « coutcouticket setup-claude --apply ».",
                        claude_output(&out)
                    );
                }
                added = true;
            }
        }
    }
    bail!(
        "l'enregistrement « {SERVER_NAME} » reste différent de l'attendu après {MAX_STEPS} tentatives (plusieurs portées ?). Inspecter avec « claude mcp get {SERVER_NAME} », retirer chaque copie avec « claude mcp remove {SERVER_NAME} --scope <portée> », puis relancer « coutcouticket setup-claude --apply »."
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const GET_USER: &str = "coutcouticket:
  Scope: User config (available in all your projects)
  Status: ✔ Connected
  Type: http
  URL: http://127.0.0.1:47813/mcp
  Headers:
    Authorization: Bearer abc123

To remove this server, run: claude mcp remove coutcouticket -s user
";

    fn wanted(port: u16, token: &str) -> Registration {
        Registration {
            transport: "http".into(),
            url: format!("http://127.0.0.1:{port}/mcp"),
            headers: vec![("Authorization".into(), format!("Bearer {token}"))],
            scope: Some("user".into()),
        }
    }

    #[test]
    fn parses_claude_mcp_get() {
        let reg = parse_get(GET_USER).unwrap();
        assert_eq!(reg, wanted(47813, "abc123"));
        assert!(reg.same_target(&wanted(47813, "abc123")));
        assert!(!reg.same_target(&wanted(47813, "autre")));
        assert!(!reg.same_target(&wanted(47814, "abc123")));
    }

    #[test]
    fn scope_from_label_when_hint_missing() {
        let text = "coutcouticket:\n  Scope: Local config (private to you in this project)\n  Type: http\n  URL: http://x/mcp\n";
        let reg = parse_get(text).unwrap();
        assert_eq!(reg.scope.as_deref(), Some("local"));
        assert!(reg.headers.is_empty());
        assert!(!reg.same_target(&wanted(1, "t")));
    }

    #[test]
    fn stdio_server_is_different() {
        let text = "coutcouticket:\n  Scope: User config\n  Type: stdio\n  Command: coutcouticket\n  Args: mcp\n";
        let reg = parse_get(text).unwrap();
        assert_eq!((reg.transport.as_str(), reg.url.as_str()), ("stdio", ""));
        assert!(!reg.same_target(&wanted(47813, "abc123")));
        assert!(parse_get("No MCP server named \"coutcouticket\".").is_none());
    }

    #[test]
    fn tokens_are_masked() {
        assert_eq!(
            mask_tokens("Authorization: Bearer abc123\n--header \"Authorization: Bearer x9\" fin"),
            "Authorization: Bearer <jeton>\n--header \"Authorization: Bearer <jeton>\" fin"
        );
    }

    #[test]
    fn add_args_are_stable() {
        let args = wanted(47813, "tok").add_args();
        assert_eq!(
            args,
            [
                "mcp", "add", "--transport", "http", "--scope", "user", "coutcouticket",
                "http://127.0.0.1:47813/mcp", "--header", "Authorization: Bearer tok"
            ]
        );
    }
}
