//! Démon : serveur MCP en Streamable HTTP sur 127.0.0.1, surveillance des notes
//! de tous les projets enregistrés (régénération de BOARD.md), et installation
//! en LaunchAgent macOS.
//!
//! Le démon n'est jamais la source de vérité : les fichiers le sont. Les hooks
//! git passent par la CLI et fonctionnent même démon arrêté.

use std::collections::{BTreeSet, HashSet};
use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail};
use axum::{
    Router,
    extract::Request,
    http::{StatusCode, header},
    middleware::{self, Next},
    response::IntoResponse,
    routing::get,
};
use notify::{RecursiveMode, Watcher};
use rmcp::transport::streamable_http_server::{
    StreamableHttpServerConfig, StreamableHttpService, session::local::LocalSessionManager,
};
use tokio_util::sync::CancellationToken;

use crate::config::{Config, DaemonConfig, Registry};
use crate::mcp::{Mode, TicketServer};
use crate::store::Project;

#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
pub const LAUNCHD_LABEL: &str = "app.coutcouticket.daemon";

fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

/// Ligne du journal du démon, horodatée à la milliseconde : permet de mesurer le
/// délai entre le lancement du processus et l'écoute (voir `daemon.log`).
macro_rules! log {
    ($($arg:tt)*) => {
        eprintln!("{} coutcouticket : {}", chrono::Local::now().format("%Y-%m-%d %H:%M:%S%.3f"), format_args!($($arg)*))
    };
}

pub async fn run() -> Result<()> {
    let started = Instant::now();
    log!("lancement du démon {} (pid {}){}", env!("CARGO_PKG_VERSION"), std::process::id(), exec_delay());

    // L'écoute passe avant tout le reste : une session Claude Code ouverte juste
    // après la connexion doit trouver le port ouvert. Les connexions arrivées
    // avant `axum::serve` attendent dans la file du noyau au lieu d'être refusées.
    let cfg = DaemonConfig::load_or_create()?;
    let addr = format!("127.0.0.1:{}", cfg.port);
    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .with_context(|| format!("impossible d'écouter sur {addr} (démon déjà lancé ?)"))?;
    log!("démon à l'écoute sur http://{addr}/mcp ({} ms après le début de run)", started.elapsed().as_millis());

    let ct = CancellationToken::new();
    let service: StreamableHttpService<TicketServer, LocalSessionManager> = StreamableHttpService::new(
        || Ok(TicketServer::new(Mode::Daemon)),
        Default::default(),
        StreamableHttpServerConfig::default()
            .with_allowed_hosts(["127.0.0.1", "localhost"])
            // Refuse toute requête portant un en-tête Origin (navigateurs) : protège du DNS rebinding.
            .enforce_origin_validation()
            .with_cancellation_token(ct.child_token()),
    );

    let token = cfg.token.clone();
    let auth = middleware::from_fn(move |req: Request, next: Next| {
        let token = token.clone();
        async move {
            let ok = req
                .headers()
                .get(header::AUTHORIZATION)
                .and_then(|v| v.to_str().ok())
                .and_then(|v| v.strip_prefix("Bearer "))
                .is_some_and(|given| constant_time_eq(given.as_bytes(), token.as_bytes()));
            if ok {
                next.run(req).await
            } else {
                (StatusCode::UNAUTHORIZED, "jeton manquant ou invalide").into_response()
            }
        }
    });

    let app = Router::new()
        .nest_service("/mcp", service)
        .layer(auth)
        .route("/health", get(|| async { format!("coutcouticket {} ok", env!("CARGO_PKG_VERSION")) }));

    // Le watcher (registre, FSEvents, régénération initiale des boards) peut être
    // lent à l'ouverture de session : il s'initialise dans son thread, port déjà ouvert.
    let watcher_stop = spawn_watcher(started)?;

    let shutdown = {
        let ct = ct.clone();
        async move {
            wait_for_signal().await;
            ct.cancel();
        }
    };
    axum::serve(listener, app).with_graceful_shutdown(shutdown).await?;
    let _ = watcher_stop.send(());
    log!("démon arrêté");
    Ok(())
}

/// Délai entre le lancement du processus (exec par launchd) et l'entrée dans
/// `run`, pour distinguer un démarrage lent du système d'une initialisation lente.
#[cfg(target_os = "macos")]
fn exec_delay() -> String {
    let mut info: libc::proc_bsdinfo = unsafe { std::mem::zeroed() };
    let size = std::mem::size_of::<libc::proc_bsdinfo>() as libc::c_int;
    // SAFETY : tampon de la taille exacte attendue pour PROC_PIDTBSDINFO.
    let n = unsafe {
        libc::proc_pidinfo(
            std::process::id() as libc::c_int,
            libc::PROC_PIDTBSDINFO,
            0,
            (&raw mut info).cast(),
            size,
        )
    };
    if n != size {
        return String::new();
    }
    let start = std::time::UNIX_EPOCH
        + Duration::from_secs(info.pbi_start_tvsec)
        + Duration::from_micros(info.pbi_start_tvusec);
    match std::time::SystemTime::now().duration_since(start) {
        Ok(d) => format!(", processus lancé il y a {} ms", d.as_millis()),
        Err(_) => String::new(),
    }
}

#[cfg(not(target_os = "macos"))]
fn exec_delay() -> String {
    String::new()
}

async fn wait_for_signal() {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{SignalKind, signal};
        let mut term = signal(SignalKind::terminate()).expect("gestion de SIGTERM");
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {},
            _ = term.recv() => {},
        }
    }
    #[cfg(not(unix))]
    {
        let _ = tokio::signal::ctrl_c().await;
    }
}

// ---------------------------------------------------------------------------
// Surveillance des fichiers
// ---------------------------------------------------------------------------

struct Watched {
    /// (racine du projet, dossier des notes)
    projects: Vec<(PathBuf, PathBuf)>,
}

fn load_watched() -> Watched {
    let projects = Registry::load()
        .map(|r| r.projects)
        .unwrap_or_default()
        .into_iter()
        .filter_map(|root| {
            let cfg = Config::load(&root).ok()?;
            let notes = root.join(&cfg.notes_dir);
            notes.is_dir().then_some((root, notes))
        })
        .collect();
    Watched { projects }
}

/// Lance le thread de surveillance. Événementiel (FSEvents sur macOS) : aucun
/// polling, aucun CPU consommé quand rien ne change.
fn spawn_watcher(started: Instant) -> Result<mpsc::Sender<()>> {
    let (stop_tx, stop_rx) = mpsc::channel::<()>();
    let (ev_tx, ev_rx) = mpsc::channel::<notify::Result<notify::Event>>();
    let registry_path = Registry::path()?;
    let global_dir = registry_path.parent().unwrap().to_path_buf();
    std::fs::create_dir_all(&global_dir)?;

    std::thread::Builder::new().name("coutcouticket-watcher".into()).spawn(move || {
        let mut watcher = match notify::recommended_watcher(ev_tx) {
            Ok(w) => w,
            Err(e) => {
                log!("surveillance indisponible : {e}");
                return;
            }
        };
        let _ = watcher.watch(&global_dir, RecursiveMode::NonRecursive);
        let mut watched = load_watched();
        let mut active: HashSet<PathBuf> = HashSet::new();
        rewatch(&mut watcher, &watched, &mut active);
        // Mise à jour initiale des boards.
        for (root, _) in &watched.projects {
            regenerate(root);
        }
        log!(
            "surveillance prête ({} projet(s), {} ms après le début de run)",
            watched.projects.len(),
            started.elapsed().as_millis()
        );

        let debounce = Duration::from_millis(300);
        let max_wait = Duration::from_secs(2);
        loop {
            if stop_rx.try_recv().is_ok() {
                return;
            }
            // Attente du premier événement d'écriture (les lectures sont ignorées).
            let mut paths: Vec<PathBuf> = match ev_rx.recv_timeout(Duration::from_secs(1)) {
                Ok(ev) => write_paths(ev),
                Err(mpsc::RecvTimeoutError::Timeout) => continue,
                Err(mpsc::RecvTimeoutError::Disconnected) => return,
            };
            if paths.is_empty() {
                continue;
            }
            // Anti-rebond : seules les écritures prolongent l'attente, plafonnée à max_wait.
            // Sans ce plafond, un processus qui lit les notes en continu (éditeur,
            // indexation, agent) empêcherait toute régénération.
            let batch_start = Instant::now();
            let mut last_write = batch_start;
            loop {
                let now = Instant::now();
                let deadline = (last_write + debounce).min(batch_start + max_wait);
                if now >= deadline {
                    break;
                }
                match ev_rx.recv_timeout(deadline - now) {
                    Ok(ev) => {
                        let more = write_paths(ev);
                        if !more.is_empty() {
                            paths.extend(more);
                            last_write = Instant::now();
                        }
                    }
                    Err(mpsc::RecvTimeoutError::Timeout) => break,
                    Err(mpsc::RecvTimeoutError::Disconnected) => return,
                }
            }
            if std::env::var_os("COUTCOUTICKET_DEBUG").is_some() {
                log!("[debug] écritures : {paths:?}");
            }

            let mut registry_changed = false;
            let mut dirty: BTreeSet<PathBuf> = BTreeSet::new();
            for p in &paths {
                if p == &registry_path {
                    registry_changed = true;
                    continue;
                }
                if is_ignored(p) {
                    continue;
                }
                if let Some((root, _)) = watched.projects.iter().find(|(_, notes)| p.starts_with(notes)) {
                    dirty.insert(root.clone());
                }
            }
            if registry_changed {
                watched = load_watched();
                rewatch(&mut watcher, &watched, &mut active);
                for (root, _) in &watched.projects {
                    dirty.insert(root.clone());
                }
            }
            for root in dirty {
                regenerate(&root);
            }
        }
    })?;
    Ok(stop_tx)
}

/// Chemins d'un événement s'il s'agit d'une écriture (création, modification de
/// contenu, suppression, renommage). Les lectures et changements de métadonnées
/// sont ignorés : sinon régénérer le board relirait les tickets et se redéclencherait.
fn write_paths(ev: notify::Result<notify::Event>) -> Vec<PathBuf> {
    use notify::EventKind;
    use notify::event::ModifyKind;
    match ev {
        Ok(ev) => match ev.kind {
            EventKind::Modify(ModifyKind::Metadata(_)) => vec![],
            EventKind::Create(_) | EventKind::Modify(_) | EventKind::Remove(_) => ev.paths,
            _ => vec![],
        },
        Err(_) => vec![],
    }
}

fn is_ignored(p: &Path) -> bool {
    let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
    name == "BOARD.md" || name.starts_with('.') || name.ends_with('~')
}

fn rewatch(watcher: &mut impl Watcher, watched: &Watched, active: &mut HashSet<PathBuf>) {
    let wanted: HashSet<PathBuf> = watched.projects.iter().map(|(_, n)| n.clone()).collect();
    for old in active.difference(&wanted).cloned().collect::<Vec<_>>() {
        let _ = watcher.unwatch(&old);
        active.remove(&old);
    }
    for new in wanted.difference(&active.clone()).cloned().collect::<Vec<_>>() {
        match watcher.watch(&new, RecursiveMode::Recursive) {
            Ok(()) => {
                log!("surveillance de {}", new.display());
                active.insert(new);
            }
            Err(e) => log!("impossible de surveiller {} : {e}", new.display()),
        }
    }
}

fn regenerate(root: &Path) {
    match Project::open(root).and_then(|p| p.regenerate_board()) {
        Ok(true) => log!("BOARD.md régénéré pour {}", root.display()),
        Ok(false) => {}
        Err(e) => log!("échec de régénération pour {} : {e:#}", root.display()),
    }
}

// ---------------------------------------------------------------------------
// État et installation
// ---------------------------------------------------------------------------

/// Interroge /health sans client HTTP (pas de dépendance supplémentaire).
pub fn health() -> Result<String> {
    let cfg = DaemonConfig::load()?.context("démon jamais configuré (lancer « coutcouticket daemon install »)")?;
    let mut stream = TcpStream::connect_timeout(&format!("127.0.0.1:{}", cfg.port).parse()?, Duration::from_secs(2))
        .with_context(|| format!("démon injoignable sur le port {}", cfg.port))?;
    stream.set_read_timeout(Some(Duration::from_secs(2)))?;
    write!(stream, "GET /health HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n")?;
    let mut resp = String::new();
    stream.read_to_string(&mut resp)?;
    let body = resp.split("\r\n\r\n").nth(1).unwrap_or("").trim().to_string();
    if !resp.starts_with("HTTP/1.1 200") {
        bail!("réponse inattendue du démon : {}", resp.lines().next().unwrap_or(""));
    }
    Ok(body)
}

#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
pub fn launchd_plist(exe: &Path, log_dir: &Path) -> String {
    let log = log_dir.join("daemon.log");
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>Label</key>
  <string>{LAUNCHD_LABEL}</string>
  <key>ProgramArguments</key>
  <array>
    <string>{exe}</string>
    <string>daemon</string>
    <string>run</string>
  </array>
  <key>RunAtLoad</key>
  <true/>
  <key>KeepAlive</key>
  <true/>
  <key>ThrottleInterval</key>
  <integer>10</integer>
  <!-- Interactive : aucun bridage CPU ni disque, pour que le port s'ouvre dès
       l'ouverture de session. Au repos, le démon ne consomme rien (événementiel). -->
  <key>ProcessType</key>
  <string>Interactive</string>
  <key>EnvironmentVariables</key>
  <dict>
    <key>PATH</key>
    <string>/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin</string>
  </dict>
  <key>StandardOutPath</key>
  <string>{log}</string>
  <key>StandardErrorPath</key>
  <string>{log}</string>
</dict>
</plist>
"#,
        exe = exe.display(),
        log = log.display()
    )
}

#[cfg(target_os = "macos")]
fn home() -> Result<PathBuf> {
    Ok(PathBuf::from(std::env::var("HOME").context("HOME absent")?))
}

#[cfg(target_os = "macos")]
fn plist_path() -> Result<PathBuf> {
    Ok(home()?.join("Library/LaunchAgents").join(format!("{LAUNCHD_LABEL}.plist")))
}

#[cfg(target_os = "macos")]
fn gui_domain() -> Result<String> {
    let out = std::process::Command::new("id").arg("-u").output()?;
    Ok(format!("gui/{}", String::from_utf8_lossy(&out.stdout).trim()))
}

#[cfg(target_os = "macos")]
pub fn install() -> Result<String> {
    use std::process::Command;
    let cfg = DaemonConfig::load_or_create()?;
    let exe = std::env::current_exe()?.canonicalize()?;
    let log_dir = home()?.join("Library/Logs/coutcouticket");
    std::fs::create_dir_all(&log_dir)?;
    let plist = plist_path()?;
    std::fs::create_dir_all(plist.parent().unwrap())?;
    crate::fsutil::write_atomic(&plist, &launchd_plist(&exe, &log_dir))?;
    let domain = gui_domain()?;
    let _ = Command::new("launchctl").args(["bootout", &format!("{domain}/{LAUNCHD_LABEL}")]).output();
    let out = Command::new("launchctl").args(["bootstrap", &domain]).arg(&plist).output()?;
    if !out.status.success() {
        bail!("launchctl bootstrap a échoué : {}", String::from_utf8_lossy(&out.stderr).trim());
    }
    Ok(format!(
        "Démon installé ({}) et démarré sur le port {}.\nLogs : {}\nÉtape suivante : « coutcouticket setup-claude »",
        plist.display(),
        cfg.port,
        log_dir.join("daemon.log").display()
    ))
}

#[cfg(target_os = "macos")]
pub fn uninstall() -> Result<String> {
    use std::process::Command;
    let plist = plist_path()?;
    let domain = gui_domain()?;
    let _ = Command::new("launchctl").args(["bootout", &format!("{domain}/{LAUNCHD_LABEL}")]).output();
    if plist.exists() {
        std::fs::remove_file(&plist)?;
    }
    Ok("Démon arrêté et désinstallé. La configuration (~/.config/coutcouticket) est conservée.".into())
}

#[cfg(not(target_os = "macos"))]
pub fn install() -> Result<String> {
    bail!("l'installation en service n'est prise en charge que sur macOS (launchd). Lancer « coutcouticket daemon run » à la main.")
}

#[cfg(not(target_os = "macos"))]
pub fn uninstall() -> Result<String> {
    bail!("désinstallation prise en charge uniquement sur macOS (launchd)")
}

/// Commande d'enregistrement du serveur MCP dans Claude Code (portée utilisateur).
pub fn claude_add_args() -> Result<Vec<String>> {
    let cfg = DaemonConfig::load_or_create()?;
    Ok(vec![
        "mcp".into(),
        "add".into(),
        "--transport".into(),
        "http".into(),
        "--scope".into(),
        "user".into(),
        "coutcouticket".into(),
        format!("http://127.0.0.1:{}/mcp", cfg.port),
        "--header".into(),
        format!("Authorization: Bearer {}", cfg.token),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plist_is_well_formed() {
        let p = launchd_plist(Path::new("/usr/local/bin/coutcouticket"), Path::new("/tmp/logs"));
        assert!(p.contains("<string>/usr/local/bin/coutcouticket</string>"));
        assert!(p.contains(LAUNCHD_LABEL));
        assert_eq!(p.matches("<dict>").count(), p.matches("</dict>").count());
        assert!(p.contains("<string>/tmp/logs/daemon.log</string>"));
    }

    /// Pas de bridage launchd : il retardait l'écoute d'environ 1 min après
    /// l'ouverture de session (ticket 0009).
    #[test]
    fn plist_is_not_throttled() {
        let p = launchd_plist(Path::new("/usr/local/bin/coutcouticket"), Path::new("/tmp/logs"));
        assert!(p.contains("<key>ProcessType</key>\n  <string>Interactive</string>"));
        for key in ["Background", "LowPriorityIO", "LowPriorityBackgroundIO", "<key>Nice</key>"] {
            assert!(!p.contains(key), "clé de bridage inattendue : {key}");
        }
        assert!(p.contains("<key>RunAtLoad</key>\n  <true/>"));
        assert!(p.contains("<key>KeepAlive</key>\n  <true/>"));
    }
}
