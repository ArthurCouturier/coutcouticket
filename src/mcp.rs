//! Serveur MCP. Les outils ne font que valider les paramètres et appeler le cœur :
//! aucune logique métier ici.

use anyhow::{Result, bail};
use rmcp::{
    ErrorData as McpError, ServerHandler,
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::{CallToolResult, ContentBlock, Implementation, ServerCapabilities, ServerConfig},
    schemars, tool, tool_handler, tool_router,
};
use serde::{Deserialize, Serialize};

use crate::config::Registry;
use crate::model::{Priority, Status, TicketId};
use crate::store::{CreateInput, Project, resolve_project};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// Lancé par le client (stdio) : le projet par défaut est le dossier courant.
    Stdio,
    /// Démon HTTP partagé : le paramètre `project` est obligatoire et doit être enregistré.
    Daemon,
}

#[derive(Clone)]
pub struct TicketServer {
    mode: Mode,
    tool_router: ToolRouter<Self>,
}

const INSTRUCTIONS: &str = "Outils de gestion des tickets coutcouticket. Toutes les modifications de \
tickets passent par ces outils (jamais d'édition manuelle du frontmatter ni de BOARD.md). \
Passer le chemin absolu de la racine du projet dans « project » (indiqué au démarrage de session). \
Lire les fichiers Markdown directement avec les outils de lecture habituels.";

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ProjectOnly {
    /// Chemin absolu de la racine du projet (dossier contenant .coutcouticket.toml).
    pub project: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct IdParams {
    /// Chemin absolu de la racine du projet.
    pub project: Option<String>,
    /// Identifiant du ticket, ex. "13" ou "0013".
    pub id: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct CreateParams {
    /// Chemin absolu de la racine du projet.
    pub project: Option<String>,
    /// Titre court et explicite ; sert à générer le slug du dossier et de la branche.
    pub title: String,
    /// Type du ticket = préfixe de branche (valeurs définies dans .coutcouticket.toml, ex. feat, fix, refacto, design, ci).
    #[serde(rename = "type")]
    pub kind: Option<String>,
    /// Priorité (p0 = critique … p3 = faible). Défaut : p2.
    pub priority: Option<Priority>,
    /// Projets de dev concernés, ex. ["backend", "app"].
    #[serde(default)]
    pub projects: Vec<String>,
    /// Description en Markdown : contexte, problème, objectif.
    pub description: Option<String>,
    /// Critères d'acceptation vérifiables, un par élément.
    #[serde(default)]
    pub acceptance: Vec<String>,
    /// Tickets dont celui-ci dépend (doivent exister), ex. ["3", "0007"].
    #[serde(default)]
    pub blocked_by: Vec<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct DependParams {
    /// Chemin absolu de la racine du projet.
    pub project: Option<String>,
    /// Identifiant du ticket dépendant (celui qui attend).
    pub id: String,
    /// Tickets dont il dépend, ex. ["3", "0007"].
    pub on: Vec<String>,
    /// true pour retirer ces dépendances au lieu de les ajouter.
    #[serde(default)]
    pub remove: bool,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct StatusParams {
    /// Chemin absolu de la racine du projet.
    pub project: Option<String>,
    /// Identifiant du ticket.
    pub id: String,
    /// Nouveau statut.
    pub status: Status,
    /// Note consignée dans le journal (raison du blocage, résumé de clôture…).
    pub note: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct LogParams {
    /// Chemin absolu de la racine du projet.
    pub project: Option<String>,
    /// Identifiant du ticket.
    pub id: String,
    /// Ce qui a été fait, constaté ou appris (Markdown, concis et factuel).
    pub text: String,
    /// Prochaine action concrète pour reprendre le travail. « Aucune » si terminé.
    pub next_step: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct DecideParams {
    /// Chemin absolu de la racine du projet.
    pub project: Option<String>,
    /// Identifiant du ticket.
    pub id: String,
    /// Intitulé court de la décision.
    pub title: String,
    /// La décision prise.
    pub decision: String,
    /// Alternatives envisagées et écartées.
    #[serde(default)]
    pub alternatives: String,
    /// Justification : contraintes, compromis, conséquences.
    pub why: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ListParams {
    /// Chemin absolu de la racine du projet.
    pub project: Option<String>,
    /// Filtrer par statut.
    pub status: Option<Status>,
    /// Filtrer par projet de dev (valeur du champ projects).
    pub dev_project: Option<String>,
}

#[derive(Serialize)]
struct FilesResult {
    committed: Vec<String>,
    uncommitted_on_ticket_branch: Vec<String>,
}

fn json<T: Serialize>(value: &T) -> Result<CallToolResult, McpError> {
    let text = serde_json::to_string_pretty(value).unwrap_or_else(|e| format!("erreur de sérialisation : {e}"));
    Ok(CallToolResult::success(vec![ContentBlock::text(text)]))
}

fn text(msg: impl Into<String>) -> Result<CallToolResult, McpError> {
    Ok(CallToolResult::success(vec![ContentBlock::text(msg.into())]))
}

fn parse_ids(ids: &[String]) -> Result<Vec<TicketId>> {
    ids.iter().map(|s| TicketId::parse(s)).collect()
}

fn fail(e: anyhow::Error) -> Result<CallToolResult, McpError> {
    Ok(CallToolResult::error(vec![ContentBlock::text(format!("Erreur : {e:#}"))]))
}

macro_rules! tryt {
    ($e:expr) => {
        match $e {
            Ok(v) => v,
            Err(e) => return fail(e.into()),
        }
    };
}

impl TicketServer {
    pub fn new(mode: Mode) -> Self {
        TicketServer { mode, tool_router: Self::tool_router() }
    }

    fn project(&self, path: Option<&str>) -> Result<Project> {
        match (self.mode, path) {
            (Mode::Daemon, None) => {
                bail!("paramètre « project » obligatoire : chemin absolu de la racine du projet")
            }
            (Mode::Daemon, Some(p)) => {
                let project = resolve_project(Some(p))?;
                if !Registry::load()?.contains(&project.root) {
                    bail!(
                        "projet {} non enregistré : lancer « coutcouticket init » dans ce projet",
                        project.root.display()
                    );
                }
                Ok(project)
            }
            (Mode::Stdio, p) => resolve_project(p),
        }
    }
}

#[tool_router]
impl TicketServer {
    #[tool(description = "Crée un ticket (id suivant, dossier, fichiers, board). Retourne l'id, le dossier et la branche à utiliser.")]
    fn ticket_create(&self, Parameters(p): Parameters<CreateParams>) -> Result<CallToolResult, McpError> {
        let project = tryt!(self.project(p.project.as_deref()));
        let blocked_by = tryt!(parse_ids(&p.blocked_by));
        let summary = tryt!(project.create(CreateInput {
            title: p.title,
            kind: p.kind,
            priority: p.priority,
            projects: p.projects,
            description: p.description,
            acceptance: p.acceptance,
            blocked_by,
        }));
        json(&summary)
    }

    #[tool(description = "Démarre un ticket : bascule (ou crée) sa branche <type>/<id>-<slug> puis passe le statut à in-progress. Seule façon autorisée de créer une branche de ticket.")]
    fn ticket_start(&self, Parameters(p): Parameters<IdParams>) -> Result<CallToolResult, McpError> {
        let project = tryt!(self.project(p.project.as_deref()));
        let id = tryt!(TicketId::parse(&p.id));
        let (summary, created) = tryt!(project.start(id));
        json(&serde_json::json!({ "ticket": summary, "branch_created": created }))
    }

    #[tool(description = "Change le statut d'un ticket (todo, in-progress, blocked, review, done, cancelled) et le consigne dans le journal.")]
    fn ticket_set_status(&self, Parameters(p): Parameters<StatusParams>) -> Result<CallToolResult, McpError> {
        let project = tryt!(self.project(p.project.as_deref()));
        let id = tryt!(TicketId::parse(&p.id));
        let summary = tryt!(project.set_status(id, p.status, p.note.as_deref()));
        if p.status == Status::Done {
            return json(&serde_json::json!({
                "ticket": summary,
                "rappel_cloture": "Vérifier : critères d'acceptation cochés dans ticket.md, doc/ mise à jour, doc/INDEX.md à jour, dernière entrée de journal avec « Prochaine étape : Aucune »."
            }));
        }
        json(&summary)
    }

    #[tool(description = "Ajoute (ou retire avec remove=true) des dépendances : le ticket « id » est bloqué par les tickets « on » tant qu'ils ne sont pas done ou cancelled. Refuse les ids inconnus, l'auto-référence et les cycles. Consigné dans le journal.")]
    fn ticket_depend(&self, Parameters(p): Parameters<DependParams>) -> Result<CallToolResult, McpError> {
        let project = tryt!(self.project(p.project.as_deref()));
        let id = tryt!(TicketId::parse(&p.id));
        let on = tryt!(parse_ids(&p.on));
        let summary = tryt!(project.depend(id, &on, p.remove));
        json(&summary)
    }

    #[tool(description = "Ajoute une entrée d'avancement datée au journal du ticket. « next_step » est obligatoire : c'est le point de reprise.")]
    fn ticket_log(&self, Parameters(p): Parameters<LogParams>) -> Result<CallToolResult, McpError> {
        let project = tryt!(self.project(p.project.as_deref()));
        let id = tryt!(TicketId::parse(&p.id));
        tryt!(project.log(id, &p.text, &p.next_step));
        text("Entrée ajoutée au journal.")
    }

    #[tool(description = "Consigne une décision structurée (décision, alternatives écartées, pourquoi) dans decisions.md du ticket.")]
    fn ticket_decide(&self, Parameters(p): Parameters<DecideParams>) -> Result<CallToolResult, McpError> {
        let project = tryt!(self.project(p.project.as_deref()));
        let id = tryt!(TicketId::parse(&p.id));
        let n = tryt!(project.decide(id, &p.title, &p.decision, &p.alternatives, &p.why));
        text(format!("Décision {n} consignée."))
    }

    #[tool(description = "Liste les tickets, triés par priorité puis id, avec filtres optionnels par statut et projet de dev. Chaque ticket expose blocked_by (dépendances déclarées) et open_blockers (celles encore ouvertes).")]
    fn ticket_list(&self, Parameters(p): Parameters<ListParams>) -> Result<CallToolResult, McpError> {
        let project = tryt!(self.project(p.project.as_deref()));
        let list = tryt!(project.list(p.status, p.dev_project.as_deref()));
        json(&list)
    }

    #[tool(description = "Point d'entrée pour reprendre un ticket : métadonnées (dont blocked_by et open_blockers), chemins des fichiers, dernière « prochaine étape », branche attendue et branche courante.")]
    fn ticket_context(&self, Parameters(p): Parameters<IdParams>) -> Result<CallToolResult, McpError> {
        let project = tryt!(self.project(p.project.as_deref()));
        let id = tryt!(TicketId::parse(&p.id));
        let ctx = tryt!(project.context(id));
        json(&ctx)
    }

    #[tool(description = "Fichiers modifiés par un ticket, dérivés de git (commits portant le trailer Ticket: <id>, plus changements non commités si on est sur sa branche).")]
    fn ticket_files(&self, Parameters(p): Parameters<IdParams>) -> Result<CallToolResult, McpError> {
        let project = tryt!(self.project(p.project.as_deref()));
        let id = tryt!(TicketId::parse(&p.id));
        let (committed, pending) = tryt!(project.files(id));
        json(&FilesResult { committed, uncommitted_on_ticket_branch: pending })
    }

    #[tool(description = "Valide toute l'arborescence des notes (dossiers, frontmatter, ids, fichiers, board). Retourne la liste des problèmes.")]
    fn notes_validate(&self, Parameters(p): Parameters<ProjectOnly>) -> Result<CallToolResult, McpError> {
        let project = tryt!(self.project(p.project.as_deref()));
        let problems = tryt!(project.validate());
        if problems.is_empty() {
            return text("Aucun problème : notes valides.");
        }
        json(&problems)
    }
}

#[tool_handler(router = self.tool_router)]
impl ServerHandler for TicketServer {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::new("coutcouticket", env!("CARGO_PKG_VERSION")))
            .with_instructions(INSTRUCTIONS)
    }
}

pub async fn serve_stdio() -> Result<()> {
    use rmcp::ServiceExt;
    let service = TicketServer::new(Mode::Stdio).serve(rmcp::transport::stdio()).await?;
    service.waiting().await?;
    Ok(())
}
