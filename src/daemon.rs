//! Démon : serveur MCP en Streamable HTTP sur 127.0.0.1, surveillance des notes
//! de tous les projets enregistrés (régénération de BOARD.md), et installation
//! en service de session : LaunchAgent (macOS) ou tâche planifiée (Windows).
//!
//! Le démon n'est jamais la source de vérité : les fichiers le sont. Les hooks
//! git passent par la CLI et fonctionnent même démon arrêté.

use std::collections::{BTreeSet, HashSet};
use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock, mpsc};
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
use crate::overview;
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
        write_log(format_args!($($arg)*))
    };
}

/// Journal ouvert par `daemon run --log-file` (Windows : la tâche planifiée ne
/// redirige pas la sortie d'erreur). Absent : sortie d'erreur (launchd la redirige).
static LOG_FILE: OnceLock<Mutex<std::fs::File>> = OnceLock::new();

fn write_log(args: std::fmt::Arguments) {
    let line = format!("{} coutcouticket : {args}\n", chrono::Local::now().format("%Y-%m-%d %H:%M:%S%.3f"));
    match LOG_FILE.get() {
        Some(file) => {
            if let Ok(mut f) = file.lock() {
                let _ = f.write_all(line.as_bytes());
            }
        }
        None => eprint!("{line}"),
    }
}

/// Fichier contenant le pid du démon lancé en service, à côté de son journal.
fn pid_file(log_file: &Path) -> PathBuf {
    log_file.with_file_name("daemon.pid")
}

pub async fn run(log_file: Option<&Path>) -> Result<()> {
    let started = Instant::now();
    if let Some(path) = log_file {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).with_context(|| format!("création de {}", dir.display()))?;
        }
        let file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .with_context(|| format!("ouverture du journal {}", path.display()))?;
        let _ = LOG_FILE.set(Mutex::new(file));
    }
    log!("lancement du démon {} (pid {}){}", env!("CARGO_PKG_VERSION"), std::process::id(), exec_delay());
    let result = serve(started, log_file.map(pid_file)).await;
    if let Err(e) = &result {
        // En service, la sortie d'erreur n'est lue par personne : l'erreur va au journal.
        log!("arrêt sur erreur : {e:#}");
    }
    result
}

/// `pid` : fichier où noter le pid une fois le port obtenu (une instance refusée
/// ne doit pas écraser celui de l'instance en cours).
async fn serve(started: Instant, pid: Option<PathBuf>) -> Result<()> {
    // L'écoute passe avant tout le reste : une session Claude Code ouverte juste
    // après la connexion doit trouver le port ouvert. Les connexions arrivées
    // avant `axum::serve` attendent dans la file du noyau au lieu d'être refusées.
    let cfg = DaemonConfig::load_or_create()?;
    let addr = format!("127.0.0.1:{}", cfg.port);
    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .with_context(|| format!("impossible d'écouter sur {addr} (démon déjà lancé ?)"))?;
    log!("démon à l'écoute sur http://{addr}/mcp ({} ms après le début de run)", started.elapsed().as_millis());
    if let Some(pid) = &pid {
        std::fs::write(pid, std::process::id().to_string()).with_context(|| format!("écriture de {}", pid.display()))?;
    }

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
    // Les événements portent des chemins canoniques (FSEvents : /tmp → /private/tmp) :
    // sans cela, un dossier de config derrière un lien symbolique masque les changements du registre.
    let global_dir = crate::fsutil::canonicalize(&global_dir).unwrap_or(global_dir);
    let registry_path = global_dir.join(registry_path.file_name().unwrap());

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
        regenerate_overview();
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
            let overview_dirty = registry_changed || !dirty.is_empty();
            for root in dirty {
                regenerate(&root);
            }
            if overview_dirty {
                regenerate_overview();
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

/// `OVERVIEW.md` du dossier de config globale : tous les projets du registre,
/// y compris ceux devenus introuvables (signalés dans le fichier).
fn regenerate_overview() {
    match Registry::load().and_then(|r| overview::regenerate_file(&r.projects)) {
        Ok(true) => log!("{} régénéré", overview::OVERVIEW_FILE),
        Ok(false) => {}
        Err(e) => log!("échec de régénération de {} : {e:#}", overview::OVERVIEW_FILE),
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
    Ok(format!(
        "Démon arrêté et désinstallé. La configuration ({}) est conservée.",
        crate::config::global_dir()?.display()
    ))
}

// ---------------------------------------------------------------------------
// Windows : tâche planifiée à l'ouverture de session
// ---------------------------------------------------------------------------

/// Nom de la tâche planifiée (à la racine du planificateur : créer un sous-dossier
/// demande des droits administrateur). Surchargeable par `COUTCOUTICKET_DAEMON_TASK`.
#[cfg_attr(not(windows), allow(dead_code))]
pub const WINDOWS_TASK: &str = "coutcouticket-daemon";

#[cfg_attr(not(windows), allow(dead_code))]
fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

/// Définition XML de la tâche : déclencheur à l'ouverture de session de `user`
/// uniquement (un déclencheur pour tout utilisateur exige les droits administrateur),
/// jeton interactif sans élévation, aucune limite de durée (72 h par défaut), pas
/// d'arrêt sur batterie, relance en cas d'échec, priorité normale (7 par défaut,
/// inférieure à la normale : même piège que le bridage launchd du ticket 0009).
/// `conhost --headless` exécute le binaire console sans ouvrir de fenêtre.
#[cfg_attr(not(windows), allow(dead_code))]
pub fn windows_task_xml(exe: &Path, log: &Path, user: &str, conhost: Option<&Path>) -> String {
    let run = format!("\"{}\" daemon run --log-file \"{}\"", exe.display(), log.display());
    let (command, arguments) = match conhost {
        Some(c) => (c.display().to_string(), format!("--headless {run}")),
        None => (exe.display().to_string(), format!("daemon run --log-file \"{}\"", log.display())),
    };
    format!(
        r#"<?xml version="1.0" encoding="UTF-16"?>
<Task version="1.2" xmlns="http://schemas.microsoft.com/windows/2004/02/mit/task">
  <RegistrationInfo>
    <Description>Démon coutcouticket : serveur MCP local et surveillance des notes. Géré par « coutcouticket daemon install|uninstall ».</Description>
  </RegistrationInfo>
  <Triggers>
    <LogonTrigger>
      <Enabled>true</Enabled>
      <UserId>{user}</UserId>
    </LogonTrigger>
  </Triggers>
  <Principals>
    <Principal id="Author">
      <UserId>{user}</UserId>
      <LogonType>InteractiveToken</LogonType>
      <RunLevel>LeastPrivilege</RunLevel>
    </Principal>
  </Principals>
  <Settings>
    <MultipleInstancesPolicy>IgnoreNew</MultipleInstancesPolicy>
    <DisallowStartIfOnBatteries>false</DisallowStartIfOnBatteries>
    <StopIfGoingOnBatteries>false</StopIfGoingOnBatteries>
    <AllowHardTerminate>true</AllowHardTerminate>
    <StartWhenAvailable>false</StartWhenAvailable>
    <RunOnlyIfNetworkAvailable>false</RunOnlyIfNetworkAvailable>
    <IdleSettings>
      <StopOnIdleEnd>false</StopOnIdleEnd>
      <RestartOnIdle>false</RestartOnIdle>
    </IdleSettings>
    <AllowStartOnDemand>true</AllowStartOnDemand>
    <Enabled>true</Enabled>
    <Hidden>false</Hidden>
    <RunOnlyIfIdle>false</RunOnlyIfIdle>
    <WakeToRun>false</WakeToRun>
    <ExecutionTimeLimit>PT0S</ExecutionTimeLimit>
    <Priority>5</Priority>
    <RestartOnFailure>
      <Interval>PT1M</Interval>
      <Count>999</Count>
    </RestartOnFailure>
  </Settings>
  <Actions Context="Author">
    <Exec>
      <Command>{command}</Command>
      <Arguments>{arguments}</Arguments>
    </Exec>
  </Actions>
</Task>
"#,
        user = xml_escape(user),
        command = xml_escape(&command),
        arguments = xml_escape(&arguments),
    )
}

/// UTF-16 LE avec BOM : encodage attendu par `schtasks /Create /XML`.
#[cfg_attr(not(windows), allow(dead_code))]
fn utf16_with_bom(text: &str) -> Vec<u8> {
    let mut out = vec![0xFF, 0xFE];
    for unit in text.encode_utf16() {
        out.extend_from_slice(&unit.to_le_bytes());
    }
    out
}

#[cfg(windows)]
mod windows {
    use std::path::{Path, PathBuf};
    use std::process::{Command, Output};
    use std::time::{Duration, Instant};

    use anyhow::{Context, Result, bail};

    use super::{WINDOWS_TASK, pid_file, utf16_with_bom, windows_task_xml};
    use crate::config::DaemonConfig;

    pub fn task_name() -> String {
        std::env::var("COUTCOUTICKET_DAEMON_TASK").unwrap_or_else(|_| WINDOWS_TASK.into())
    }

    /// `%LOCALAPPDATA%\coutcouticket` : journal, pid et définition de la tâche.
    pub fn log_dir() -> Result<PathBuf> {
        let base = std::env::var("LOCALAPPDATA")
            .context("variable LOCALAPPDATA absente : lancer la commande depuis une session Windows ordinaire")?;
        Ok(PathBuf::from(base).join("coutcouticket"))
    }

    fn user() -> Result<String> {
        let name = std::env::var("USERNAME").context("variable USERNAME absente")?;
        Ok(match std::env::var("USERDOMAIN") {
            Ok(domain) if !domain.is_empty() => format!("{domain}\\{name}"),
            _ => name,
        })
    }

    fn conhost() -> Option<PathBuf> {
        let root = std::env::var("SystemRoot").unwrap_or_else(|_| r"C:\Windows".into());
        Some(PathBuf::from(root).join("System32").join("conhost.exe")).filter(|p| p.is_file())
    }

    fn schtasks(args: &[&str]) -> Result<Output> {
        Command::new("schtasks")
            .args(args)
            .output()
            .context("impossible de lancer schtasks (planificateur de tâches Windows)")
    }

    fn text(o: &Output) -> String {
        format!("{}{}", String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr)).trim().to_string()
    }

    pub fn task_installed() -> bool {
        schtasks(&["/Query", "/TN", &task_name()]).is_ok_and(|o| o.status.success())
    }

    /// Arrête le démon lancé par la tâche : fin de la tâche, puis arrêt du processus
    /// noté dans daemon.pid (s'il s'agit bien de coutcouticket), puis attente de la
    /// libération du port pour que la nouvelle instance puisse écouter.
    fn stop(log_dir: &Path) {
        let _ = schtasks(&["/End", "/TN", &task_name()]);
        let pid_path = pid_file(&log_dir.join("daemon.log"));
        if let Some(pid) = std::fs::read_to_string(&pid_path).ok().and_then(|p| p.trim().parse::<u32>().ok())
            && pid != std::process::id()
            && is_coutcouticket(pid)
        {
            let _ = Command::new("taskkill").args(["/PID", &pid.to_string(), "/F"]).output();
        }
        let _ = std::fs::remove_file(&pid_path);
        if let Ok(Some(cfg)) = DaemonConfig::load() {
            let start = Instant::now();
            while super::port_open(cfg.port) && start.elapsed() < Duration::from_secs(5) {
                std::thread::sleep(Duration::from_millis(100));
            }
        }
    }

    fn is_coutcouticket(pid: u32) -> bool {
        let filter = format!("PID eq {pid}");
        Command::new("tasklist")
            .args(["/FI", &filter, "/FO", "CSV", "/NH"])
            .output()
            .is_ok_and(|o| String::from_utf8_lossy(&o.stdout).to_lowercase().contains("coutcouticket"))
    }

    pub fn install() -> Result<String> {
        let cfg = DaemonConfig::load_or_create()?;
        let exe = crate::fsutil::canonicalize(&std::env::current_exe()?)?;
        let dir = log_dir()?;
        std::fs::create_dir_all(&dir).with_context(|| format!("création de {}", dir.display()))?;
        let log = dir.join("daemon.log");
        let conhost = conhost();
        let xml_path = dir.join("daemon-task.xml");
        std::fs::write(&xml_path, utf16_with_bom(&windows_task_xml(&exe, &log, &user()?, conhost.as_deref())))
            .with_context(|| format!("écriture de {}", xml_path.display()))?;

        stop(&dir);
        let name = task_name();
        let xml = xml_path.to_string_lossy().to_string();
        let out = schtasks(&["/Create", "/TN", &name, "/XML", &xml, "/F"])?;
        if !out.status.success() {
            bail!(
                "schtasks /Create a échoué : {}\nDéfinition de la tâche : {}. Vérifier que la session est une session utilisateur ordinaire (aucun droit administrateur n'est requis), puis relancer « coutcouticket daemon install ».",
                text(&out),
                xml_path.display()
            );
        }
        let out = schtasks(&["/Run", "/TN", &name])?;
        if !out.status.success() {
            bail!(
                "tâche « {name} » créée mais son lancement a échoué : {}\nElle démarrera à la prochaine ouverture de session ; pour réessayer : « coutcouticket daemon install ».",
                text(&out)
            );
        }
        let start = Instant::now();
        let status = loop {
            match super::health() {
                Ok(body) => break format!("Réponse : {body}"),
                Err(_) if start.elapsed() < Duration::from_secs(10) => std::thread::sleep(Duration::from_millis(200)),
                Err(e) => break format!("Le démon ne répond pas encore ({e:#}) : consulter le journal, puis « coutcouticket daemon status »."),
            }
        };
        let window = if conhost.is_some() {
            ""
        } else {
            "\nAttention : conhost.exe introuvable, le démon s'exécute dans une fenêtre console visible."
        };
        Ok(format!(
            "Démon installé (tâche planifiée « {name} », lancée à l'ouverture de session) et démarré sur le port {}.\n{status}\nLogs : {}{window}\nÉtape suivante : « coutcouticket setup-claude --apply »",
            cfg.port,
            log.display()
        ))
    }

    pub fn uninstall() -> Result<String> {
        let dir = log_dir()?;
        stop(&dir);
        let name = task_name();
        if task_installed() {
            let out = schtasks(&["/Delete", "/TN", &name, "/F"])?;
            if !out.status.success() {
                bail!("schtasks /Delete a échoué : {}. Retirer la tâche « {name} » dans le Planificateur de tâches, puis relancer.", text(&out));
            }
        }
        let _ = std::fs::remove_file(dir.join("daemon-task.xml"));
        Ok(format!(
            "Démon arrêté et désinstallé (tâche « {name} » retirée). La configuration ({}) est conservée.",
            crate::config::global_dir()?.display()
        ))
    }
}

#[cfg_attr(not(windows), allow(dead_code))]
fn port_open(port: u16) -> bool {
    TcpStream::connect_timeout(&std::net::SocketAddr::from(([127, 0, 0, 1], port)), Duration::from_millis(300)).is_ok()
}

#[cfg(windows)]
pub fn install() -> Result<String> {
    windows::install()
}

#[cfg(windows)]
pub fn uninstall() -> Result<String> {
    windows::uninstall()
}

#[cfg(not(any(target_os = "macos", windows)))]
pub fn install() -> Result<String> {
    bail!("l'installation en service n'est prise en charge que sur macOS (launchd) et Windows (tâche planifiée). Lancer « coutcouticket daemon run » à la main.")
}

#[cfg(not(any(target_os = "macos", windows)))]
pub fn uninstall() -> Result<String> {
    bail!("désinstallation prise en charge uniquement sur macOS (launchd) et Windows (tâche planifiée)")
}

/// `daemon status` : réponse de /health ; sous Windows, en cas d'échec, indique
/// si la tâche planifiée est installée.
#[cfg(not(windows))]
pub fn status() -> Result<String> {
    health()
}

#[cfg(windows)]
pub fn status() -> Result<String> {
    health().map_err(|e| {
        if windows::task_installed() {
            e.context(format!(
                "tâche planifiée « {} » installée mais démon injoignable : consulter {}, ou relancer « coutcouticket daemon install »",
                windows::task_name(),
                windows::log_dir().map(|d| d.join("daemon.log").display().to_string()).unwrap_or_default()
            ))
        } else {
            e.context("tâche planifiée absente : lancer « coutcouticket daemon install »")
        }
    })
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

    #[test]
    fn tache_windows_bien_formee() {
        let xml = windows_task_xml(
            Path::new(r"C:\Users\Léa & co\bin\coutcouticket.exe"),
            Path::new(r"C:\Users\Léa & co\AppData\Local\coutcouticket\daemon.log"),
            r"PC\Léa",
            Some(Path::new(r"C:\Windows\System32\conhost.exe")),
        );
        assert!(xml.contains("<UserId>PC\\Léa</UserId>\n    </LogonTrigger>"), "{xml}");
        assert!(xml.contains("<LogonType>InteractiveToken</LogonType>") && xml.contains("<RunLevel>LeastPrivilege</RunLevel>"));
        assert!(xml.contains("<Command>C:\\Windows\\System32\\conhost.exe</Command>"), "{xml}");
        assert!(
            xml.contains(r#"<Arguments>--headless &quot;C:\Users\Léa &amp; co\bin\coutcouticket.exe&quot; daemon run --log-file &quot;C:\Users\Léa &amp; co\AppData\Local\coutcouticket\daemon.log&quot;</Arguments>"#),
            "{xml}"
        );
        // Pas de limite de durée, pas de bridage, relance en cas d'échec.
        for needle in ["<ExecutionTimeLimit>PT0S</ExecutionTimeLimit>", "<Priority>5</Priority>", "<RestartOnFailure>", "<StopIfGoingOnBatteries>false"] {
            assert!(xml.contains(needle), "{needle} absent : {xml}");
        }
        for tag in ["Task", "Settings", "Principal", "LogonTrigger", "Exec", "RestartOnFailure", "IdleSettings"] {
            assert_eq!(xml.matches(&format!("<{tag}>")).count() + xml.matches(&format!("<{tag} ")).count(), xml.matches(&format!("</{tag}>")).count(), "{tag}");
        }
        let direct = windows_task_xml(Path::new(r"C:\b\coutcouticket.exe"), Path::new(r"C:\l\daemon.log"), "u", None);
        assert!(direct.contains(r#"<Command>C:\b\coutcouticket.exe</Command>"#) && direct.contains("<Arguments>daemon run --log-file"), "{direct}");
        let bytes = utf16_with_bom("<a/>");
        assert_eq!(bytes, [0xFF, 0xFE, b'<', 0, b'a', 0, b'/', 0, b'>', 0]);
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
