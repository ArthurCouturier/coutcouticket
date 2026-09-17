mod board;
mod claude;
mod config;
mod daemon;
mod fsutil;
mod git;
mod hooks;
mod init;
mod mcp;
mod model;
mod naming;
mod store;
mod templates;

use std::path::PathBuf;
use std::process::ExitCode;

use anyhow::{Result, bail};
use clap::{Parser, Subcommand};

use crate::config::{DaemonConfig, Registry, find_root};
use crate::model::{Priority, Status, TicketId};
use crate::store::{CreateInput, Project, resolve_project};

#[derive(Parser)]
#[command(name = "coutcouticket", version, about = "Tickets et notes Markdown pour Claude Code")]
struct Cli {
    /// Racine du projet (par défaut : remonte depuis le dossier courant).
    #[arg(long, short = 'C', global = true)]
    project: Option<PathBuf>,
    #[command(subcommand)]
    command: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Crée ou répare l'arborescence du projet (idempotent).
    Init {
        /// Dossier du projet (défaut : dossier courant).
        path: Option<PathBuf>,
        /// Ne pas installer les hooks git.
        #[arg(long)]
        no_git_hooks: bool,
        /// Ne pas écrire le bloc dans CLAUDE.md.
        #[arg(long)]
        no_claude_md: bool,
        /// Ne pas enregistrer le projet auprès du démon.
        #[arg(long)]
        no_register: bool,
        /// Ne pas ajouter le dossier de notes au .gitignore.
        #[arg(long)]
        no_gitignore: bool,
    },
    /// Crée un ticket.
    New {
        #[arg(long, short)]
        title: String,
        #[arg(long = "type", short = 'k')]
        kind: Option<String>,
        #[arg(long, short)]
        priority: Option<String>,
        /// Projets de dev, séparés par des virgules.
        #[arg(long, value_delimiter = ',')]
        projects: Vec<String>,
        #[arg(long, short)]
        description: Option<String>,
        /// Critère d'acceptation (répétable).
        #[arg(long = "accept", short = 'a')]
        acceptance: Vec<String>,
        /// Tickets dont celui-ci dépend, séparés par des virgules (ex. 3,7).
        #[arg(long = "blocked-by", value_delimiter = ',')]
        blocked_by: Vec<String>,
    },
    /// Ajoute (ou retire) des dépendances : <id> est bloqué par les tickets --on.
    Depend {
        id: String,
        /// Tickets dont <id> dépend, séparés par des virgules (ex. 3,7).
        #[arg(long, value_delimiter = ',', required = true)]
        on: Vec<String>,
        /// Retire ces dépendances au lieu de les ajouter.
        #[arg(long)]
        remove: bool,
    },
    /// Démarre un ticket : branche <type>/<id>-<slug> + statut in-progress.
    Start { id: String },
    /// Change le statut d'un ticket.
    Status {
        id: String,
        status: String,
        #[arg(long, short)]
        note: Option<String>,
    },
    /// Ajoute une entrée d'avancement au journal.
    Log {
        id: String,
        #[arg(long, short)]
        text: String,
        #[arg(long, short)]
        next: String,
    },
    /// Consigne une décision.
    Decide {
        id: String,
        #[arg(long, short)]
        title: String,
        #[arg(long, short)]
        decision: String,
        #[arg(long, short, default_value = "")]
        alternatives: String,
        #[arg(long, short)]
        why: String,
    },
    /// Liste les tickets.
    List {
        #[arg(long, short)]
        status: Option<String>,
        /// Filtre par projet de dev.
        #[arg(long = "dev-project")]
        dev_project: Option<String>,
        #[arg(long)]
        json: bool,
    },
    /// Affiche le contexte de reprise d'un ticket.
    Show {
        id: String,
        #[arg(long)]
        json: bool,
    },
    /// Fichiers modifiés par un ticket (dérivés de git).
    Files { id: String },
    /// Régénère BOARD.md.
    Board,
    /// Valide l'arborescence et les tickets.
    Validate {
        #[arg(long)]
        json: bool,
    },
    /// Serveur MCP en stdio (secours si le démon est arrêté).
    Mcp,
    /// Démon : serveur MCP HTTP + surveillance des notes.
    Daemon {
        #[command(subcommand)]
        action: DaemonCmd,
    },
    /// Registre des projets suivis par le démon.
    Projects {
        #[command(subcommand)]
        action: ProjectsCmd,
    },
    /// Enregistre le serveur MCP du démon dans Claude Code.
    SetupClaude {
        /// Enregistre (ou met à jour) le serveur au lieu d'afficher la commande. Sans danger si déjà fait.
        #[arg(long)]
        apply: bool,
    },
    /// Points d'entrée des hooks (appelés par git et Claude Code).
    #[command(hide = true)]
    Hook {
        #[command(subcommand)]
        which: HookCmd,
    },
}

#[derive(Subcommand)]
enum DaemonCmd {
    /// Lance le démon au premier plan.
    Run,
    /// Installe et démarre le LaunchAgent macOS.
    Install,
    /// Arrête et retire le LaunchAgent.
    Uninstall,
    /// Vérifie que le démon répond.
    Status,
}

#[derive(Subcommand)]
enum ProjectsCmd {
    /// Liste les projets enregistrés.
    List,
    /// Enregistre un projet.
    Add { path: Option<PathBuf> },
    /// Retire un projet.
    Remove { path: PathBuf },
}

#[derive(Subcommand)]
enum HookCmd {
    PreCommit,
    PrepareCommitMsg {
        file: PathBuf,
        source: Option<String>,
        sha: Option<String>,
    },
    SessionStart,
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match run(cli) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("coutcouticket : {e:#}");
            ExitCode::FAILURE
        }
    }
}

fn project(cli_path: &Option<PathBuf>) -> Result<Project> {
    let s = cli_path.as_ref().map(|p| p.to_string_lossy().to_string());
    resolve_project(s.as_deref())
}

fn parse_ids(ids: &[String]) -> Result<Vec<TicketId>> {
    ids.iter().map(|s| TicketId::parse(s)).collect()
}

fn runtime() -> Result<tokio::runtime::Runtime> {
    Ok(tokio::runtime::Builder::new_current_thread().enable_all().build()?)
}

fn run(cli: Cli) -> Result<()> {
    let p = &cli.project;
    match cli.command {
        Cmd::Init { path, no_git_hooks, no_claude_md, no_register, no_gitignore } => {
            let target = match path.or_else(|| p.clone()) {
                Some(t) => t,
                None => std::env::current_dir()?,
            };
            let report = init::init(
                &target,
                init::InitOptions {
                    git_hooks: !no_git_hooks,
                    claude_md: !no_claude_md,
                    register: !no_register,
                    gitignore: !no_gitignore,
                },
            )?;
            print!("{}", report.render());
        }
        Cmd::New { title, kind, priority, projects, description, acceptance, blocked_by } => {
            let s = project(p)?.create(CreateInput {
                title,
                kind,
                priority: priority.map(|x| x.parse::<Priority>()).transpose()?,
                projects,
                description,
                acceptance,
                blocked_by: parse_ids(&blocked_by)?,
            })?;
            println!("Ticket {} créé : {}\nBranche à utiliser : {}", s.id, s.dir, s.branch);
            if !s.open_blockers.is_empty() {
                println!("Bloqué par : {}", s.open_blockers.join(", "));
            }
        }
        Cmd::Depend { id, on, remove } => {
            let s = project(p)?.depend(TicketId::parse(&id)?, &parse_ids(&on)?, remove)?;
            let deps = if s.blocked_by.is_empty() { "aucune".to_string() } else { s.blocked_by.join(", ") };
            let open = if s.open_blockers.is_empty() { "aucune".to_string() } else { s.open_blockers.join(", ") };
            println!("Ticket {} : dépendances {deps} (encore ouvertes : {open}).", s.id);
        }
        Cmd::Start { id } => {
            let (s, created) = project(p)?.start(TicketId::parse(&id)?)?;
            println!(
                "Ticket {} en cours sur la branche {} ({}).",
                s.id,
                s.branch,
                if created { "créée" } else { "existante" }
            );
            if !s.open_blockers.is_empty() {
                println!("Attention : ce ticket est bloqué par des tickets encore ouverts : {}.", s.open_blockers.join(", "));
            }
        }
        Cmd::Status { id, status, note } => {
            let s = project(p)?.set_status(TicketId::parse(&id)?, status.parse::<Status>()?, note.as_deref())?;
            println!("Ticket {} → {}", s.id, s.status);
        }
        Cmd::Log { id, text, next } => {
            project(p)?.log(TicketId::parse(&id)?, &text, &next)?;
            println!("Entrée ajoutée au journal.");
        }
        Cmd::Decide { id, title, decision, alternatives, why } => {
            let n = project(p)?.decide(TicketId::parse(&id)?, &title, &decision, &alternatives, &why)?;
            println!("Décision {n} consignée.");
        }
        Cmd::List { status, dev_project, json } => {
            let status = status.map(|s| s.parse::<Status>()).transpose()?;
            let list = project(p)?.list(status, dev_project.as_deref())?;
            if json {
                println!("{}", serde_json::to_string_pretty(&list)?);
            } else if list.is_empty() {
                println!("Aucun ticket.");
            } else {
                for t in list {
                    let projects = if t.projects.is_empty() { String::new() } else { format!(" [{}]", t.projects.join(", ")) };
                    let blockers = if t.open_blockers.is_empty() { String::new() } else { format!("  (bloqué par {})", t.open_blockers.join(", ")) };
                    println!("{}  {:<11} {} {:<7} {}{}{}", t.id, t.status.as_str(), t.priority, t.kind, t.title, projects, blockers);
                }
            }
        }
        Cmd::Show { id, json } => {
            let ctx = project(p)?.context(TicketId::parse(&id)?)?;
            if json {
                println!("{}", serde_json::to_string_pretty(&ctx)?);
            } else {
                let s = &ctx.summary;
                println!("{} « {} »", s.id, s.title);
                println!("  statut    {}   priorité {}   type {}", s.status, s.priority, s.kind);
                println!("  projets   {}", if s.projects.is_empty() { "—".into() } else { s.projects.join(", ") });
                if !s.blocked_by.is_empty() {
                    let open = if s.open_blockers.is_empty() { "aucune ouverte".to_string() } else { format!("ouvertes : {}", s.open_blockers.join(", ")) };
                    println!("  dépend de {}   ({open})", s.blocked_by.join(", "));
                }
                println!("  branche   {}{}", s.branch, if ctx.on_ticket_branch { " (courante)" } else { "" });
                if let Some(e) = &ctx.git_error {
                    println!("  ⚠️ git en échec, branche courante inconnue : {e}");
                }
                println!("  fichiers  {}  {}  {}", ctx.ticket_file, ctx.journal_file, ctx.decisions_file);
                println!("  journal   {} entrée(s)   décisions {}", ctx.journal_entries, ctx.decisions);
                println!("  prochaine étape : {}", ctx.last_next_step.as_deref().unwrap_or("—"));
            }
        }
        Cmd::Files { id } => {
            let (committed, pending) = project(p)?.files(TicketId::parse(&id)?)?;
            println!("Commités ({}) :", committed.len());
            committed.iter().for_each(|f| println!("  {f}"));
            if !pending.is_empty() {
                println!("Non commités sur la branche du ticket ({}) :", pending.len());
                pending.iter().for_each(|f| println!("  {f}"));
            }
        }
        Cmd::Board => {
            let proj = project(p)?;
            let changed = proj.regenerate_board()?;
            println!("{} {}", proj.rel(&proj.board_path()), if changed { "régénéré" } else { "déjà à jour" });
        }
        Cmd::Validate { json } => {
            let problems = project(p)?.validate()?;
            if json {
                println!("{}", serde_json::to_string_pretty(&problems)?);
            } else {
                for pb in &problems {
                    println!("✗ {} : {}", pb.path, pb.message);
                }
            }
            if !problems.is_empty() {
                bail!("{} problème(s)", problems.len());
            }
            if !json {
                println!("✓ notes valides");
            }
        }
        Cmd::Mcp => runtime()?.block_on(mcp::serve_stdio())?,
        Cmd::Daemon { action } => match action {
            DaemonCmd::Run => runtime()?.block_on(daemon::run())?,
            DaemonCmd::Install => println!("{}", daemon::install()?),
            DaemonCmd::Uninstall => println!("{}", daemon::uninstall()?),
            DaemonCmd::Status => println!("{}", daemon::health()?),
        },
        Cmd::Projects { action } => {
            let mut reg = Registry::load()?;
            match action {
                ProjectsCmd::List => {
                    if reg.projects.is_empty() {
                        println!("Aucun projet enregistré.");
                    }
                    for path in &reg.projects {
                        let ok = if find_root(path).is_ok() { "" } else { "  (introuvable ou non initialisé)" };
                        println!("{}{ok}", path.display());
                    }
                }
                ProjectsCmd::Add { path } => {
                    let base = match path.or_else(|| p.clone()) {
                        Some(x) => x,
                        None => std::env::current_dir()?,
                    };
                    let root = find_root(&base)?;
                    if reg.add(&root)? {
                        reg.save()?;
                        println!("Projet enregistré : {}", root.display());
                    } else {
                        println!("Déjà enregistré : {}", root.display());
                    }
                }
                ProjectsCmd::Remove { path } => {
                    if reg.remove(&path) {
                        reg.save()?;
                        println!("Projet retiré : {}", path.display());
                    } else {
                        bail!("projet non enregistré : {}", path.display());
                    }
                }
            }
        }
        Cmd::SetupClaude { apply } => {
            if apply {
                match claude::apply()? {
                    claude::Outcome::UpToDate { scope } => println!(
                        "Serveur MCP coutcouticket déjà enregistré dans Claude Code (portée {}) : enregistrement à jour, rien à faire.",
                        scope.as_deref().unwrap_or("inconnue")
                    ),
                    claude::Outcome::Registered { removed } if removed.is_empty() => {
                        println!("Serveur MCP coutcouticket enregistré dans Claude Code (portée user).")
                    }
                    claude::Outcome::Registered { removed } => println!(
                        "Serveur MCP coutcouticket mis à jour dans Claude Code : ancien enregistrement retiré (portée {}), nouveau en portée user.\nRedémarrer les sessions Claude Code ouvertes (ou /mcp) pour qu'elles le prennent en compte.",
                        removed.join(", ")
                    ),
                }
            } else {
                let cfg = DaemonConfig::load_or_create()?;
                let args = claude::Registration::wanted()?.add_args();
                let display = format!(
                    "claude {}",
                    args.iter().map(|a| if a.contains(' ') { format!("\"{a}\"") } else { a.clone() }).collect::<Vec<_>>().join(" ")
                );
                println!("Commande pour enregistrer le démon dans Claude Code (portée utilisateur) :\n\n{display}\n");
                println!("Si « coutcouticket » est déjà enregistré avec une autre valeur, le retirer d'abord : claude mcp remove coutcouticket");
                println!("Ou relancer avec --apply : vérifie l'existant et ne le remplace que s'il diffère. Port : {}.", cfg.port);
                println!("Secours sans démon : claude mcp add --scope user coutcouticket-stdio -- coutcouticket mcp");
            }
        }
        Cmd::Hook { which } => match which {
            HookCmd::PreCommit => hooks::pre_commit()?,
            HookCmd::PrepareCommitMsg { file, source, .. } => hooks::prepare_commit_msg(&file, source.as_deref())?,
            HookCmd::SessionStart => hooks::session_start()?,
        },
    }
    Ok(())
}
