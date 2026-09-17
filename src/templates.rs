//! Templates embarqués dans le binaire (aucun fichier externe à installer).

pub const GLOBAL_README: &str = include_str!("../templates/global-readme.md");
pub const DOC_INDEX: &str = include_str!("../templates/doc-index.md");
pub const CLAUDE_BLOCK: &str = include_str!("../templates/claude-md-block.md");
pub const CLAUDE_BLOCK_START: &str = "<!-- coutcouticket:start";
pub const CLAUDE_BLOCK_END: &str = "<!-- coutcouticket:end -->";
const TICKET_BODY: &str = include_str!("../templates/ticket-body.md");
const JOURNAL: &str = include_str!("../templates/journal.md");
const DECISIONS: &str = include_str!("../templates/decisions.md");

pub fn with_notes_dir(template: &str, notes_dir: &str) -> String {
    template.replace("{{notes_dir}}", notes_dir)
}

pub fn ticket_body(description: Option<&str>, acceptance: &[String]) -> String {
    let description = description
        .map(str::trim)
        .filter(|d| !d.is_empty())
        .unwrap_or("_À rédiger._");
    let acceptance = if acceptance.is_empty() {
        "- [ ] _À définir._".to_string()
    } else {
        acceptance
            .iter()
            .map(|a| format!("- [ ] {}", a.trim()))
            .collect::<Vec<_>>()
            .join("\n")
    };
    TICKET_BODY
        .replace("{{description}}", description)
        .replace("{{acceptance}}", &acceptance)
}

pub fn journal(id: &str) -> String {
    JOURNAL.replace("{{id}}", id)
}

pub fn decisions(id: &str) -> String {
    DECISIONS.replace("{{id}}", id)
}
