//! Panneau web en lecture seule, servi par le démon sous `/ui` : page embarquée
//! (`src/ui/`), API JSON (`api/overview`, `api/ticket`) et flux SSE (`api/events`)
//! alimenté par le watcher. Rien ne tourne tant qu'aucun onglet n'est ouvert.
//!
//! Sécurité (le navigateur ne peut pas envoyer le jeton Bearer du démon) :
//! - `coutcouticket ui` demande au démon, avec le Bearer, un code de connexion à
//!   usage unique (2 min) et ouvre `/ui/login?code=…` ;
//! - `/ui/login` consomme le code et pose un cookie de session (`HttpOnly`,
//!   `SameSite=Strict`, `Path=/ui`) dont le secret est tiré au lancement du démon ;
//! - toute route `/ui` vérifie l'en-tête Host (DNS rebinding), l'en-tête Origin
//!   s'il existe (même origine uniquement) et `Sec-Fetch-Site` s'il existe
//!   (refus des requêtes venues d'un autre site, y compris un autre port local) ;
//! - réponses avec CSP stricte (aucun script ni style en ligne), `nosniff`,
//!   `no-referrer`, `no-store`, et interdiction d'affichage en cadre.

use std::convert::Infallible;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail};
use axum::{
    Router,
    extract::{Request, State},
    http::{HeaderMap, HeaderValue, StatusCode, Uri, header},
    middleware::{self, Next},
    response::{
        IntoResponse, Response,
        sse::{Event, KeepAlive, Sse},
    },
    routing::{get, post},
};
use tokio::sync::broadcast;
use tokio_util::sync::CancellationToken;

use crate::config::{DaemonConfig, Registry, random_token};
use crate::daemon::constant_time_eq;
use crate::model::TicketId;
use crate::overview;

const INDEX_HTML: &str = include_str!("ui/index.html");
const APP_JS: &str = include_str!("ui/app.js");
const APP_CSS: &str = include_str!("ui/app.css");

const COOKIE: &str = "cct_ui";
/// Durée de validité d'un code de connexion.
const CODE_TTL: Duration = Duration::from_secs(120);
/// Codes en attente conservés au plus (les plus anciens sont oubliés).
const MAX_CODES: usize = 16;
/// Chemin (protégé par le Bearer) où `coutcouticket ui` obtient un code.
pub const CODE_PATH: &str = "/ui-login-code";

const CSP: &str = "default-src 'none'; script-src 'self'; style-src 'self'; connect-src 'self'; img-src data:; \
                   base-uri 'none'; form-action 'none'; frame-ancestors 'none'";

pub struct UiState {
    port: u16,
    /// Secret du cookie de session, tiré au lancement : redémarrer le démon ferme les sessions.
    session: String,
    codes: Mutex<Vec<(String, Instant)>>,
    events: broadcast::Sender<()>,
    /// Annulé à l'arrêt du démon : ferme les flux SSE, sinon l'arrêt les attendrait.
    shutdown: CancellationToken,
}

impl UiState {
    /// `events` : un message par lot de changements détecté par le watcher.
    pub fn new(port: u16, events: broadcast::Sender<()>, shutdown: CancellationToken) -> Result<Arc<Self>> {
        Ok(Arc::new(UiState { port, session: random_token()?, codes: Mutex::new(Vec::new()), events, shutdown }))
    }

    fn new_code(&self) -> Result<String> {
        let code = random_token()?;
        let mut codes = self.codes.lock().unwrap_or_else(|e| e.into_inner());
        codes.retain(|(_, at)| at.elapsed() < CODE_TTL);
        if codes.len() >= MAX_CODES {
            codes.remove(0);
        }
        codes.push((code.clone(), Instant::now()));
        Ok(code)
    }

    /// Consomme un code valide (usage unique).
    fn take_code(&self, given: &str) -> bool {
        let mut codes = self.codes.lock().unwrap_or_else(|e| e.into_inner());
        codes.retain(|(_, at)| at.elapsed() < CODE_TTL);
        match codes.iter().position(|(c, _)| constant_time_eq(c.as_bytes(), given.as_bytes())) {
            Some(i) => {
                codes.remove(i);
                true
            }
            None => false,
        }
    }

    fn has_session(&self, headers: &HeaderMap) -> bool {
        headers
            .get_all(header::COOKIE)
            .iter()
            .filter_map(|v| v.to_str().ok())
            .flat_map(|v| v.split(';'))
            .filter_map(|kv| kv.trim().split_once('='))
            .any(|(k, v)| k == COOKIE && constant_time_eq(v.as_bytes(), self.session.as_bytes()))
    }
}

/// Route du code de connexion, à placer derrière l'authentification Bearer du démon.
pub fn code_router(state: Arc<UiState>) -> Router {
    Router::new().route(CODE_PATH, post(login_code)).with_state(state)
}

/// Routes du panneau, protégées par `guard`.
pub fn router(state: Arc<UiState>) -> Router {
    Router::new()
        .route("/ui", get(|| async { redirect("/ui/") }))
        .route("/ui/", get(index))
        .route("/ui/app.js", get(|| async { asset("text/javascript; charset=utf-8", APP_JS) }))
        .route("/ui/app.css", get(|| async { asset("text/css; charset=utf-8", APP_CSS) }))
        .route("/ui/login", get(login))
        .route("/ui/api/overview", get(api_overview))
        .route("/ui/api/ticket", get(api_ticket))
        .route("/ui/api/events", get(api_events))
        .route_layer(middleware::from_fn_with_state(state.clone(), guard))
        .with_state(state)
}

/// Hôte attendu (anti DNS rebinding) : 127.0.0.1 ou localhost, sur le port du démon.
fn host_ok(host: &str, port: u16) -> bool {
    let (name, p) = match host.rsplit_once(':') {
        Some((n, p)) => (n, Some(p)),
        None => (host, None),
    };
    matches!(name, "127.0.0.1" | "localhost") && p.is_none_or(|p| p == port.to_string())
}

fn origin_ok(origin: &str, port: u16) -> bool {
    origin == format!("http://127.0.0.1:{port}") || origin == format!("http://localhost:{port}")
}

async fn guard(State(state): State<Arc<UiState>>, req: Request, next: Next) -> Response {
    let headers = req.headers();
    let text = |name: header::HeaderName| headers.get(name).map(|v| v.to_str().unwrap_or("?"));
    let refused = if !text(header::HOST).is_some_and(|h| host_ok(h, state.port)) {
        Some("hôte non autorisé : ouvrir le panneau avec « coutcouticket ui »")
    } else if text(header::ORIGIN).is_some_and(|o| !origin_ok(o, state.port)) {
        Some("requête d'une autre origine refusée")
    } else if text(header::HeaderName::from_static("sec-fetch-site")).is_some_and(|s| s != "same-origin" && s != "none") {
        Some("requête d'un autre site refusée")
    } else {
        None
    };
    let path = req.uri().path();
    let public = matches!(path, "/ui/login" | "/ui/app.js" | "/ui/app.css");
    let api = path.starts_with("/ui/api/");
    let mut resp = match refused {
        Some(msg) => error(StatusCode::FORBIDDEN, api, msg),
        None if !public && !state.has_session(headers) => error(
            StatusCode::UNAUTHORIZED,
            api,
            "session absente ou expirée : lancer « coutcouticket ui » pour ouvrir le panneau",
        ),
        None => next.run(req).await,
    };
    let h = resp.headers_mut();
    h.insert(header::CONTENT_SECURITY_POLICY, HeaderValue::from_static(CSP));
    h.insert(header::X_CONTENT_TYPE_OPTIONS, HeaderValue::from_static("nosniff"));
    h.insert(header::REFERRER_POLICY, HeaderValue::from_static("no-referrer"));
    h.insert(header::X_FRAME_OPTIONS, HeaderValue::from_static("DENY"));
    h.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    h.insert("cross-origin-opener-policy", HeaderValue::from_static("same-origin"));
    h.insert("cross-origin-resource-policy", HeaderValue::from_static("same-origin"));
    resp
}

fn redirect(to: &'static str) -> Response {
    (StatusCode::PERMANENT_REDIRECT, [(header::LOCATION, to)]).into_response()
}

fn asset(content_type: &'static str, body: &'static str) -> Response {
    ([(header::CONTENT_TYPE, content_type)], body).into_response()
}

fn html(status: StatusCode, body: String) -> Response {
    (status, [(header::CONTENT_TYPE, "text/html; charset=utf-8")], body).into_response()
}

fn json(status: StatusCode, value: &impl serde::Serialize) -> Response {
    match serde_json::to_string(value) {
        Ok(body) => (status, [(header::CONTENT_TYPE, "application/json")], body).into_response(),
        Err(e) => error(StatusCode::INTERNAL_SERVER_ERROR, true, &format!("sérialisation impossible : {e}")),
    }
}

fn escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

/// Page courte (connexion, erreurs), stylée par `app.css`.
fn notice(title: &str, message: &str, head: &str) -> String {
    format!(
        "<!doctype html>\n<html lang=\"fr\"><head><meta charset=\"utf-8\">{head}<title>coutcouticket</title>\
         <link rel=\"icon\" href=\"data:,\"><link rel=\"stylesheet\" href=\"/ui/app.css\"></head>\
         <body><div class=\"notice\"><h1>{}</h1><p>{}</p></div></body></html>\n",
        escape(title),
        escape(message).replace('«', "<code>«").replace('»', "»</code>")
    )
}

/// Erreur en JSON (`{"error": …}`) pour l'API, en page HTML sinon.
fn error(status: StatusCode, api: bool, message: &str) -> Response {
    if api {
        json(status, &serde_json::json!({ "error": message }))
    } else {
        html(status, notice("Panneau coutcouticket", message, ""))
    }
}

fn query_param(uri: &Uri, name: &str) -> Option<String> {
    uri.query()?.split('&').find_map(|kv| {
        let (k, v) = kv.split_once('=').unwrap_or((kv, ""));
        (k == name).then(|| percent_encoding::percent_decode_str(&v.replace('+', " ")).decode_utf8_lossy().into_owned())
    })
}

async fn login_code(State(state): State<Arc<UiState>>, headers: HeaderMap) -> Response {
    // Réservé à la CLI : un navigateur envoie toujours Origin sur un POST.
    if headers.contains_key(header::ORIGIN) {
        return (StatusCode::FORBIDDEN, "requête de navigateur refusée").into_response();
    }
    match state.new_code() {
        Ok(code) => code.into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, format!("{e:#}")).into_response(),
    }
}

async fn login(State(state): State<Arc<UiState>>, uri: Uri) -> Response {
    let code = query_param(&uri, "code").unwrap_or_default();
    if code.is_empty() || !state.take_code(&code) {
        return error(
            StatusCode::UNAUTHORIZED,
            false,
            "lien de connexion invalide, expiré ou déjà utilisé : relancer « coutcouticket ui »",
        );
    }
    // Page intermédiaire plutôt qu'une redirection HTTP : la navigation suivante part
    // de la même origine, le cookie SameSite=Strict est donc envoyé par tous les navigateurs.
    let cookie = format!("{COOKIE}={}; Path=/ui; HttpOnly; SameSite=Strict", state.session);
    let mut resp = html(
        StatusCode::OK,
        notice("Connexion…", "Ouverture du panneau.", "<meta http-equiv=\"refresh\" content=\"0; url=./\">"),
    );
    if let Ok(v) = HeaderValue::from_str(&cookie) {
        resp.headers_mut().insert(header::SET_COOKIE, v);
    }
    resp
}

async fn index() -> Response {
    html(StatusCode::OK, INDEX_HTML.to_string())
}

async fn api_overview() -> Response {
    match Registry::load().and_then(|r| overview::build(&r.projects, overview::Filter::default())) {
        Ok(o) => json(StatusCode::OK, &o),
        Err(e) => error(StatusCode::INTERNAL_SERVER_ERROR, true, &format!("{e:#}")),
    }
}

async fn api_ticket(uri: Uri) -> Response {
    let (Some(project), Some(id)) = (query_param(&uri, "project"), query_param(&uri, "id")) else {
        return error(StatusCode::BAD_REQUEST, true, "paramètres « project » et « id » obligatoires");
    };
    let id = match TicketId::parse(&id) {
        Ok(id) => id,
        Err(e) => return error(StatusCode::BAD_REQUEST, true, &format!("{e:#}")),
    };
    let roots = match Registry::load() {
        Ok(r) => r.projects,
        Err(e) => return error(StatusCode::INTERNAL_SERVER_ERROR, true, &format!("{e:#}")),
    };
    match overview::detail(&roots, &project, id) {
        Ok(Some(d)) => json(StatusCode::OK, &d),
        Ok(None) => error(
            StatusCode::NOT_FOUND,
            true,
            "projet non enregistré : lancer « coutcouticket init » dans ce projet",
        ),
        Err(e) => error(StatusCode::NOT_FOUND, true, &format!("{e:#}")),
    }
}

/// Flux SSE : un événement `changed` par lot de changements, commentaire de maintien
/// toutes les 30 s. Aucune tâche lancée : le flux vit avec la connexion.
async fn api_events(State(state): State<Arc<UiState>>) -> Response {
    let rx = state.events.subscribe();
    let stop = state.shutdown.clone();
    let stream = futures_util::stream::unfold((rx, stop), |(mut rx, stop)| async move {
        let changed = tokio::select! {
            r = rx.recv() => !matches!(r, Err(broadcast::error::RecvError::Closed)),
            _ = stop.cancelled() => false,
        };
        // Un retard (messages perdus) signale aussi un changement.
        changed.then(|| (Ok::<_, Infallible>(Event::default().data("changed")), (rx, stop)))
    });
    let mut resp = Sse::new(stream).keep_alive(KeepAlive::new().interval(Duration::from_secs(30))).into_response();
    resp.headers_mut().insert("x-accel-buffering", HeaderValue::from_static("no"));
    resp
}

// ---------------------------------------------------------------------------
// CLI : `coutcouticket ui`
// ---------------------------------------------------------------------------

/// Demande un code au démon et rend l'adresse de connexion.
pub fn login_url() -> Result<String> {
    let cfg = DaemonConfig::load()?
        .context("démon jamais configuré : lancer « coutcouticket daemon install », puis relancer « coutcouticket ui »")?;
    let unreachable = || {
        format!(
            "démon injoignable sur le port {} : le démarrer avec « coutcouticket daemon install », puis vérifier avec « coutcouticket daemon status »",
            cfg.port
        )
    };
    let mut stream = TcpStream::connect_timeout(&std::net::SocketAddr::from(([127, 0, 0, 1], cfg.port)), Duration::from_secs(2))
        .with_context(unreachable)?;
    stream.set_read_timeout(Some(Duration::from_secs(5)))?;
    write!(
        stream,
        "POST {CODE_PATH} HTTP/1.1\r\nHost: 127.0.0.1:{}\r\nAuthorization: Bearer {}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
        cfg.port, cfg.token
    )
    .with_context(unreachable)?;
    let mut resp = String::new();
    stream.read_to_string(&mut resp).with_context(unreachable)?;
    let status = resp.split_whitespace().nth(1).unwrap_or("");
    let body = resp.split_once("\r\n\r\n").map(|(_, b)| b.trim()).unwrap_or("");
    match status {
        "200" if !body.is_empty() && body.chars().all(|c| c.is_ascii_hexdigit()) => {
            Ok(format!("http://127.0.0.1:{}/ui/login?code={body}", cfg.port))
        }
        "404" => bail!(
            "le démon en cours d'exécution ne sert pas le panneau (version antérieure) : relancer « coutcouticket daemon install » pour le redémarrer avec ce binaire"
        ),
        "401" => bail!(
            "jeton refusé par le démon ({} a changé depuis son lancement) : relancer « coutcouticket daemon install »",
            DaemonConfig::path()?.display()
        ),
        _ => bail!("réponse inattendue du démon : {}", resp.lines().next().unwrap_or("(vide)")),
    }
}

/// Ouvre l'adresse dans le navigateur par défaut.
pub fn open_browser(url: &str) -> Result<()> {
    use std::process::Command;
    #[cfg(target_os = "macos")]
    let mut cmd = Command::new("open");
    // `start` est une commande interne de cmd ; "" est le titre de fenêtre. L'adresse
    // ne contient ni espace ni caractère spécial de cmd (code hexadécimal).
    #[cfg(windows)]
    let mut cmd = {
        let mut c = Command::new("cmd");
        c.args(["/C", "start", ""]);
        c
    };
    #[cfg(not(any(target_os = "macos", windows)))]
    let mut cmd = Command::new("xdg-open");
    let status = cmd.arg(url).status().context("impossible de lancer le navigateur")?;
    if !status.success() {
        bail!("l'ouverture du navigateur a échoué ({status})");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hote_et_origine() {
        for ok in ["127.0.0.1", "127.0.0.1:4000", "localhost:4000", "localhost"] {
            assert!(host_ok(ok, 4000), "{ok}");
        }
        for ko in ["127.0.0.1:4001", "evil.example", "evil.example:4000", "127.0.0.1.evil.example:4000", "[::1]:4000", ""] {
            assert!(!host_ok(ko, 4000), "{ko}");
        }
        assert!(origin_ok("http://127.0.0.1:4000", 4000) && origin_ok("http://localhost:4000", 4000));
        for ko in ["http://127.0.0.1:4001", "https://127.0.0.1:4000", "http://evil.example", "null", "http://127.0.0.1:4000.evil"] {
            assert!(!origin_ok(ko, 4000), "{ko}");
        }
    }

    #[test]
    fn codes_a_usage_unique_et_cookie() {
        let (tx, _) = broadcast::channel(1);
        let s = UiState::new(4000, tx, CancellationToken::new()).unwrap();
        let code = s.new_code().unwrap();
        assert!(!s.take_code("faux") && !s.take_code(""));
        assert!(s.take_code(&code));
        assert!(!s.take_code(&code), "code réutilisé");
        for _ in 0..MAX_CODES + 4 {
            s.new_code().unwrap();
        }
        assert_eq!(s.codes.lock().unwrap().len(), MAX_CODES);

        let mut h = HeaderMap::new();
        assert!(!s.has_session(&h));
        h.insert(header::COOKIE, HeaderValue::from_str(&format!("a=b; {COOKIE}=faux")).unwrap());
        assert!(!s.has_session(&h));
        h.append(header::COOKIE, HeaderValue::from_str(&format!("x=y; {COOKIE}={}", s.session)).unwrap());
        assert!(s.has_session(&h));
    }

    #[test]
    fn parametres_decodes() {
        let uri: Uri = "/ui/api/ticket?project=%2FUsers%2Fl%C3%A9a%2Fmon+projet&id=0003&x".parse().unwrap();
        assert_eq!(query_param(&uri, "project").as_deref(), Some("/Users/léa/mon projet"));
        assert_eq!(query_param(&uri, "id").as_deref(), Some("0003"));
        assert_eq!(query_param(&uri, "x").as_deref(), Some(""));
        assert_eq!(query_param(&uri, "absent"), None);
    }

    #[test]
    fn page_sans_script_en_ligne() {
        // La CSP interdit les scripts et styles en ligne : la page ne doit pas en contenir.
        for needle in ["<script>", "<style", "style=", "onclick=", "onload=", "http://", "https://"] {
            assert!(!INDEX_HTML.contains(needle), "{needle} dans index.html");
        }
        assert!(!APP_JS.contains("http://") && !APP_JS.contains("https://"), "dépendance externe dans app.js");
        assert!(notice("t", "lancer « coutcouticket ui » <b>", "").contains("<code>« coutcouticket ui »</code> &lt;b&gt;"));
    }
}
