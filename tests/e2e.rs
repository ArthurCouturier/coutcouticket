//! Tests de bout en bout : binaire réel, vrai dépôt git, vrais hooks, vrai MCP.

use std::fs;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Output, Stdio};
use std::time::{Duration, Instant};

const BIN: &str = env!("CARGO_BIN_EXE_coutcouticket");

struct Env {
    _tmp: tempfile::TempDir,
    repo: PathBuf,
    home: PathBuf,
    path_var: String,
}

impl Env {
    fn new() -> Env {
        let tmp = tempfile::tempdir().unwrap();
        let repo = tmp.path().join("mon-projet");
        let home = tmp.path().join("cct-home");
        fs::create_dir_all(&repo).unwrap();
        fs::create_dir_all(&home).unwrap();
        let bin_dir = Path::new(BIN).parent().unwrap().to_path_buf();
        let path_var = std::env::join_paths(std::iter::once(bin_dir).chain(std::env::split_paths(&std::env::var_os("PATH").unwrap())))
            .unwrap()
            .into_string()
            .unwrap();
        let env = Env { _tmp: tmp, repo, home, path_var };
        env.git(&["init", "-q", "-b", "main"]);
        env.git(&["config", "user.email", "test@example.com"]);
        env.git(&["config", "user.name", "Test"]);
        env
    }

    fn cmd(&self, program: &str) -> Command {
        let mut c = Command::new(program);
        c.current_dir(&self.repo)
            .env("COUTCOUTICKET_HOME", &self.home)
            .env("PATH", &self.path_var)
            .env_remove("XDG_CONFIG_HOME");
        c
    }

    fn cct(&self, args: &[&str]) -> Output {
        self.cmd(BIN).args(args).output().unwrap()
    }

    fn ok(&self, args: &[&str]) -> String {
        let out = self.cct(args);
        assert!(out.status.success(), "coutcouticket {args:?} a échoué :\n{}\n{}", stdout(&out), stderr(&out));
        stdout(&out)
    }

    fn git(&self, args: &[&str]) -> Output {
        self.cmd("git").args(args).output().unwrap()
    }

    fn git_ok(&self, args: &[&str]) -> String {
        let out = self.git(args);
        assert!(out.status.success(), "git {args:?} a échoué :\n{}\n{}", stdout(&out), stderr(&out));
        stdout(&out)
    }

    fn notes(&self) -> PathBuf {
        self.repo.join("0-notes")
    }
}

/// Chemin canonique tel que l'écrit coutcouticket (sans le préfixe `\\?\` de Windows).
fn canon(p: &Path) -> PathBuf {
    let c = p.canonicalize().unwrap();
    match c.to_str().and_then(|s| s.strip_prefix(r"\\?\")) {
        Some(rest) => PathBuf::from(rest),
        None => c,
    }
}

fn stdout(o: &Output) -> String {
    String::from_utf8_lossy(&o.stdout).to_string()
}
fn stderr(o: &Output) -> String {
    String::from_utf8_lossy(&o.stderr).to_string()
}

#[test]
fn workflow_complet() {
    let env = Env::new();

    // --- init crée l'arborescence, puis est idempotent
    let out = env.ok(&["init"]);
    assert!(out.contains("créé"), "{out}");
    for p in ["0-global/README.md", "0-global/BOARD.md", "tickets", "doc/INDEX.md"] {
        assert!(env.notes().join(p).exists(), "{p} manquant");
    }
    assert!(env.repo.join(".coutcouticket.toml").is_file());
    assert!(fs::read_to_string(env.repo.join("CLAUDE.md")).unwrap().contains("coutcouticket:start"));
    assert!(env.repo.join(".git/hooks/pre-commit").is_file());
    assert!(fs::read_to_string(env.home.join("projects.toml")).unwrap().contains("mon-projet"));
    let again = env.ok(&["init"]);
    assert!(again.contains("déjà complète"), "{again}");

    // --- arborescence partielle réparée par init
    fs::remove_dir_all(env.notes().join("doc")).unwrap();
    let repaired = env.ok(&["init"]);
    assert!(repaired.contains("0-notes/doc/INDEX.md"), "{repaired}");

    // --- création de tickets
    let out = env.ok(&["new", "-t", "Ajouter l'écran « Mes plantes »", "--projects", "app,backend", "-p", "p1", "-a", "La liste s'affiche"]);
    assert!(out.contains("0-notes/tickets/0001-ajouter-l-ecran-mes-plantes"), "{out}");
    assert!(out.contains("feat/0001-ajouter-l-ecran-mes-plantes"), "{out}");
    env.ok(&["new", "-t", "Corriger le crash au démarrage", "-k", "fix"]);
    let bad_type = env.cct(&["new", "-t", "x", "-k", "feature"]);
    assert!(!bad_type.status.success());
    assert!(stderr(&bad_type).contains("non autorisé"));
    let board = fs::read_to_string(env.notes().join("0-global/BOARD.md")).unwrap();
    assert!(board.contains("[0001]") && board.contains("[0002]"), "{board}");
    assert!(board.contains("| app |"), "{board}");
    env.ok(&["validate"]);

    // --- commit sur main autorisé
    env.git_ok(&["add", "-A"]);
    env.git_ok(&["commit", "-q", "-m", "init notes"]);

    // --- branche non conforme refusée
    env.git_ok(&["switch", "-q", "-c", "wip"]);
    fs::write(env.repo.join("a.txt"), "a").unwrap();
    env.git_ok(&["add", "a.txt"]);
    let refused = env.git(&["commit", "-q", "-m", "wip"]);
    assert!(!refused.status.success(), "commit sur « wip » aurait dû être refusé");
    assert!(stderr(&refused).contains("non conforme"), "{}", stderr(&refused));
    env.git_ok(&["switch", "-q", "main"]);
    env.git_ok(&["branch", "-q", "-D", "wip"]);

    // --- branche au mauvais format (id sur 2 chiffres) refusée
    env.git_ok(&["switch", "-q", "-c", "feat/01-ajouter-l-ecran-mes-plantes"]);
    let refused = env.git(&["commit", "-q", "-m", "x"]);
    assert!(!refused.status.success());
    env.git_ok(&["switch", "-q", "main"]);

    // --- démarrage : branche stricte + statut + trailer automatique
    let out = env.ok(&["start", "1"]);
    assert!(out.contains("feat/0001-ajouter-l-ecran-mes-plantes"), "{out}");
    assert_eq!(env.git_ok(&["branch", "--show-current"]).trim(), "feat/0001-ajouter-l-ecran-mes-plantes");
    fs::write(env.repo.join("src.rs"), "fn main() {}").unwrap();
    env.git_ok(&["add", "-A"]);
    env.git_ok(&["commit", "-q", "-m", "écran mes plantes"]);
    let msg = env.git_ok(&["log", "-1", "--pretty=%B"]);
    assert!(msg.contains("Ticket: 0001"), "trailer absent : {msg}");

    // --- journal, décision, contexte
    env.ok(&["log", "1", "-t", "Liste branchée sur l'API", "-n", "Gérer l'état vide"]);
    env.ok(&["decide", "1", "-t", "Pagination", "-d", "Curseur", "-a", "Offset", "-w", "Stable si insertions"]);
    let show = env.ok(&["show", "1", "--json"]);
    assert!(show.contains("\"last_next_step\": \"Gérer l'état vide\""), "{show}");
    assert!(show.contains("\"decisions\": 1"), "{show}");
    assert!(show.contains("\"on_ticket_branch\": true"), "{show}");
    let bad_log = env.cct(&["log", "1", "-t", "x", "-n", " "]);
    assert!(!bad_log.status.success());

    // --- fichiers dérivés de git
    let files = env.ok(&["files", "1"]);
    assert!(files.contains("src.rs"), "{files}");

    // --- statut et liste
    env.ok(&["status", "1", "review", "-n", "Prêt à relire"]);
    let list = env.ok(&["list", "--status", "review"]);
    assert!(list.contains("0001") && !list.contains("0002"), "{list}");
    let bad_status = env.cct(&["status", "2", "en-cours"]);
    assert!(!bad_status.status.success());

    // --- détection des dérives et réparation
    let t2 = env.notes().join("tickets/0002-corriger-le-crash-au-demarrage");
    fs::remove_file(t2.join("journal.md")).unwrap();
    assert!(!env.cct(&["validate"]).status.success());
    env.ok(&["init"]);
    env.ok(&["validate"]);
    let ticket = t2.join("ticket.md");
    let original = fs::read_to_string(&ticket).unwrap();
    fs::write(&ticket, original.replace("status: todo", "status: wip")).unwrap();
    let invalid = env.cct(&["validate"]);
    assert!(stdout(&invalid).contains("statut invalide"), "{}", stdout(&invalid));
    fs::write(&ticket, original).unwrap();
    env.ok(&["board"]);
    env.ok(&["validate"]);

    // --- hook SessionStart de Claude Code
    let mut child = env
        .cmd(BIN)
        .args(["hook", "session-start"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let payload = format!("{{\"cwd\": {:?}, \"hook_event_name\": \"SessionStart\"}}", env.repo.display().to_string());
    child.stdin.take().unwrap().write_all(payload.as_bytes()).unwrap();
    let out = child.wait_with_output().unwrap();
    let text = stdout(&out);
    assert!(text.contains("additionalContext"), "{text}");
    assert!(text.contains("Gérer l'état vide"), "{text}");
}

#[test]
fn trailers_des_commits_de_notes() {
    let env = Env::new();
    env.ok(&["init", "--no-gitignore"]);
    env.ok(&["new", "-t", "Un"]);
    env.ok(&["new", "-t", "Deux"]);
    env.ok(&["new", "-t", "Trois"]);
    env.git_ok(&["add", "-A"]);
    env.git_ok(&["commit", "-q", "-m", "init"]);
    env.ok(&["start", "1"]);
    let last_msg = || env.git_ok(&["log", "-1", "--pretty=%B"]);
    let trailers = || -> Vec<String> {
        last_msg().lines().filter(|l| l.starts_with("Ticket:")).map(String::from).collect()
    };

    // --- code + notes du ticket de la branche : trailer de la branche
    fs::write(env.repo.join("code.rs"), "fn main() {}").unwrap();
    env.git_ok(&["add", "-A"]);
    env.git_ok(&["commit", "-q", "-m", "code"]);
    assert_eq!(trailers(), ["Ticket: 0001"]);

    // --- notes d'autres tickets seulement : leurs trailers, pas celui de la branche
    env.ok(&["log", "2", "-t", "Avancée", "-n", "Suite"]);
    env.ok(&["status", "3", "done"]);
    env.git_ok(&["add", "-A"]);
    env.git_ok(&["commit", "-q", "-m", "notes des autres"]);
    assert_eq!(trailers(), ["Ticket: 0002", "Ticket: 0003"]);
    let committed = env.git_ok(&["show", "--name-only", "--pretty=format:", "HEAD"]);
    assert!(committed.contains("0-global/BOARD.md"), "{committed}");

    // --- idem avec commit -a (index temporaire de git)
    env.ok(&["log", "2", "-t", "Encore", "-n", "Suite"]);
    env.git_ok(&["commit", "-q", "-a", "-m", "notes via -a"]);
    assert_eq!(trailers(), ["Ticket: 0002"]);

    // --- doc seule : relève du ticket de la branche
    fs::write(env.notes().join("doc/INDEX.md"), "# Index\n").unwrap();
    env.git_ok(&["commit", "-q", "-a", "-m", "doc"]);
    assert_eq!(trailers(), ["Ticket: 0001"]);

    // --- notes du ticket de la branche avec celles d'un autre : trailer de la branche
    env.ok(&["log", "1", "-t", "Jalon", "-n", "Suite"]);
    env.ok(&["log", "2", "-t", "Jalon", "-n", "Suite"]);
    env.git_ok(&["commit", "-q", "-a", "-m", "notes mêlées"]);
    assert_eq!(trailers(), ["Ticket: 0001"]);

    // --- trailer déjà écrit : message inchangé
    fs::write(env.repo.join("autre.rs"), "fn autre() {}").unwrap();
    env.git_ok(&["add", "-A"]);
    env.git_ok(&["commit", "-q", "-m", "pour trois\n\nTicket: 0003"]);
    assert_eq!(trailers(), ["Ticket: 0003"]);

    // --- commit mal attribué (ancien comportement) : files le filtre quand même
    env.ok(&["log", "3", "-t", "Note tardive", "-n", "Aucune"]);
    env.git_ok(&["commit", "-q", "-a", "-m", "mal attribué\n\nTicket: 0001"]);

    // --- files : ni notes d'autres tickets ni BOARD.md, commités ou non
    env.ok(&["log", "2", "-t", "Non commité", "-n", "Suite"]);
    fs::write(env.repo.join("code.rs"), "fn main() { }").unwrap();
    let files1 = env.ok(&["files", "1"]);
    for f in ["code.rs", "0-notes/tickets/0001-un/ticket.md", "0-notes/tickets/0001-un/journal.md", "0-notes/doc/INDEX.md"] {
        assert!(files1.contains(f), "{f} absent :\n{files1}");
    }
    for f in ["0002-deux", "0003-trois", "BOARD.md", "autre.rs"] {
        assert!(!files1.contains(f), "{f} présent :\n{files1}");
    }
    let files2 = env.ok(&["files", "2"]);
    assert!(files2.contains("0-notes/tickets/0002-deux/journal.md"), "{files2}");
    assert!(!files2.contains("0001-un") && !files2.contains("0003-trois") && !files2.contains("BOARD.md"), "{files2}");
    let files3 = env.ok(&["files", "3"]);
    assert!(files3.contains("0-notes/tickets/0003-trois/ticket.md") && files3.contains("autre.rs"), "{files3}");
}

#[test]
fn dependances_cli() {
    let env = Env::new();
    env.ok(&["init"]);
    env.ok(&["new", "-t", "Socle"]);
    env.ok(&["new", "-t", "Écran"]);
    let out = env.ok(&["new", "-t", "Export", "--blocked-by", "2,#1"]);
    assert!(out.contains("Bloqué par : 0001, 0002"), "{out}");
    let refused = env.cct(&["new", "-t", "Orphelin", "--blocked-by", "9"]);
    assert!(!refused.status.success());
    assert!(stderr(&refused).contains("0009 introuvable"), "{}", stderr(&refused));

    // cycle refusé, ajout et retrait
    let cycle = env.cct(&["depend", "1", "--on", "3"]);
    assert!(!cycle.status.success());
    assert!(stderr(&cycle).contains("cycle 0001 → 0003 → 0001"), "{}", stderr(&cycle));
    let out = env.ok(&["depend", "2", "--on", "1"]);
    assert!(out.contains("dépendances 0001 (encore ouvertes : 0001)"), "{out}");
    env.ok(&["depend", "3", "--on", "2", "--remove"]);
    let show = env.ok(&["show", "3", "--json"]);
    assert!(show.contains("\"blocked_by\": [\n    \"0001\"\n  ]"), "{show}");

    // une dépendance terminée ne bloque plus
    let list = env.ok(&["list"]);
    assert!(list.contains("(bloqué par 0001)"), "{list}");
    env.ok(&["status", "1", "done"]);
    let list = env.ok(&["list", "--json"]);
    assert!(list.contains("\"open_blockers\": []") && !list.contains("\"open_blockers\": [\n"), "{list}");
    let board = fs::read_to_string(env.notes().join("0-global/BOARD.md")).unwrap();
    assert!(board.contains("| Bloqué par |"), "{board}");
    env.ok(&["validate"]);

    // édition manuelle incohérente détectée par validate
    let ticket = env.notes().join("tickets/0001-socle/ticket.md");
    let original = fs::read_to_string(&ticket).unwrap();
    fs::write(&ticket, original.replace("projects: []", "projects: []\nblocked_by: [3]")).unwrap();
    let invalid = env.cct(&["validate"]);
    assert!(stdout(&invalid).contains("cycle de dépendances : 0001 → 0003 → 0001"), "{}", stdout(&invalid));
    fs::write(&ticket, original).unwrap();
    env.ok(&["board"]);
    env.ok(&["validate"]);
}

// ---------------------------------------------------------------------------
// Hooks git hors du terminal (client graphique : PATH de launchd)
// ---------------------------------------------------------------------------

#[cfg(unix)]
const LAUNCHD_PATH: &str = "/usr/bin:/bin:/usr/sbin:/sbin";

#[cfg(unix)]
impl Env {
    /// Commande lancée comme par un client git graphique : environnement vide, PATH minimal.
    fn gui(&self, args: &[&str]) -> Output {
        Command::new("git")
            .current_dir(&self.repo)
            .env_clear()
            .env("HOME", &self.home)
            .env("PATH", LAUNCHD_PATH)
            .env("COUTCOUTICKET_HOME", &self.home)
            .args(args)
            .output()
            .unwrap()
    }
}

#[cfg(unix)]
#[test]
fn hooks_hors_du_path() {
    let env = Env::new();
    env.ok(&["init"]);
    let pre_commit = env.repo.join(".git/hooks/pre-commit");
    assert!(fs::read_to_string(&pre_commit).unwrap().contains(BIN), "chemin du binaire absent du hook");

    // --- ancien gabarit (recherche dans le PATH seule) mis à jour par init
    let old = "#!/bin/sh\n# coutcouticket-hook (ancien)\nexec coutcouticket hook pre-commit\n";
    fs::write(&pre_commit, old).unwrap();
    let out = env.ok(&["init"]);
    assert!(out.contains("mis à jour hook git pre-commit"), "{out}");
    assert!(fs::read_to_string(&pre_commit).unwrap().contains(BIN));

    // --- commit avec le PATH de launchd : accepté, trailer ajouté
    env.ok(&["new", "-t", "Tester les hooks", "-k", "fix"]);
    env.git_ok(&["add", "-A"]);
    env.git_ok(&["commit", "-q", "-m", "init notes"]);
    env.ok(&["start", "1"]);
    let out = env.gui(&["commit", "-q", "--allow-empty", "-m", "depuis un client graphique"]);
    assert!(out.status.success(), "commit refusé :\n{}", stderr(&out));
    let msg = env.git_ok(&["log", "-1", "--pretty=%B"]);
    assert!(msg.contains("Ticket: 0001"), "trailer absent : {msg}");

    // --- binaire noté disparu : repli sur le PATH, puis message d'erreur
    let moved = env.home.join("bin-temporaire");
    fs::create_dir_all(&moved).unwrap();
    let copy = moved.join("coutcouticket");
    fs::copy(BIN, &copy).unwrap();
    let out = env.cmd(copy.to_str().unwrap()).arg("init").output().unwrap();
    assert!(out.status.success(), "{}", stderr(&out));
    assert!(fs::read_to_string(&pre_commit).unwrap().contains(copy.to_str().unwrap()));
    fs::remove_file(&copy).unwrap();
    env.git_ok(&["commit", "-q", "--allow-empty", "-m", "repli sur le PATH"]);
    let refused = env.gui(&["commit", "-q", "--allow-empty", "-m", "introuvable"]);
    assert!(!refused.status.success(), "commit accepté sans binaire");
    assert!(stderr(&refused).contains("coutcouticket introuvable"), "{}", stderr(&refused));

    // --- hook tiers jamais modifié
    let tiers = "#!/bin/sh\n# hook husky\nexit 0\n";
    fs::write(&pre_commit, tiers).unwrap();
    let out = env.ok(&["init"]);
    assert!(out.contains("non géré par coutcouticket"), "{out}");
    assert_eq!(fs::read_to_string(&pre_commit).unwrap(), tiers);
}

// ---------------------------------------------------------------------------
// Vue de tous les projets
// ---------------------------------------------------------------------------

#[test]
fn overview_plusieurs_projets() {
    let env = Env::new();
    let base = env.repo.parent().unwrap().to_path_buf();
    env.ok(&["init"]);
    env.ok(&["new", "-t", "Tâche locale", "-p", "p1"]);
    env.ok(&["new", "-t", "Suite locale", "--blocked-by", "1"]);
    env.ok(&["new", "-t", "Déjà faite"]);
    env.ok(&["status", "3", "done"]);

    // Deuxième projet, un ticket en cours ; troisième projet enregistré puis supprimé.
    let autre = base.join("autre-projet");
    let disparu = base.join("projet-disparu");
    for dir in [&autre, &disparu] {
        fs::create_dir_all(dir).unwrap();
        assert!(env.cmd("git").arg("-C").arg(dir).args(["init", "-q", "-b", "main"]).status().unwrap().success());
        env.ok(&["init", dir.to_str().unwrap()]);
    }
    let autre_s = autre.to_str().unwrap();
    env.ok(&["-C", autre_s, "new", "-t", "Chantier distant", "-p", "p3"]);
    env.ok(&["-C", autre_s, "status", "1", "in-progress"]);
    let disparu_canon = canon(&disparu);
    fs::remove_dir_all(&disparu).unwrap();

    let out = env.cct(&["overview"]);
    let text = stdout(&out);
    assert!(out.status.success(), "la vue échoue malgré le projet introuvable :\n{text}\n{}", stderr(&out));
    assert!(text.contains("3 ticket(s) ouvert(s) dans 3 projet(s)."), "{text}");
    let pos = |needle: &str| text.find(needle).unwrap_or_else(|| panic!("« {needle} » absent :\n{text}"));
    assert!(pos("En cours (1)") < pos("autre-projet  0001  p3") && pos("autre-projet  0001") < pos("À faire (2)"), "{text}");
    assert!(pos("mon-projet    0001  p1") < pos("mon-projet    0002  p2"), "{text}");
    assert!(text.contains("Suite locale  (bloqué par 0001)"), "{text}");
    assert!(!text.contains("Déjà faite"), "ticket fermé affiché :\n{text}");
    assert!(text.contains(&format!("⚠️ {} : projet introuvable", disparu_canon.display())), "{text}");

    let json: serde_json::Value = serde_json::from_str(&env.ok(&["overview", "--json"])).unwrap();
    assert_eq!(json["projects"], 3);
    let tickets = json["tickets"].as_array().unwrap();
    let ids: Vec<(String, String)> = tickets.iter().map(|t| (t["project"].as_str().unwrap().into(), t["id"].as_str().unwrap().into())).collect();
    assert_eq!(ids, [("autre-projet", "0001"), ("mon-projet", "0001"), ("mon-projet", "0002")].map(|(a, b)| (a.to_string(), b.to_string())));
    assert_eq!(tickets[0]["project_path"], canon(&autre).display().to_string());
    assert_eq!(tickets[2]["open_blockers"], serde_json::json!(["0001"]));
    assert_eq!(json["warnings"].as_array().unwrap().len(), 1, "{json}");

    let filtered = env.ok(&["overview", "--status", "todo", "--priority", "p2"]);
    assert!(filtered.contains("1 ticket(s)") && filtered.contains("Suite locale"), "{filtered}");
    let out = env.cct(&["overview", "--status", "done"]);
    assert!(!out.status.success() && stderr(&out).contains("tickets ouverts"), "{}", stderr(&out));

    // MCP stdio : tickets_overview sans paramètre project, depuis un dossier hors projet.
    let mut child = env
        .cmd(BIN)
        .current_dir(&base)
        .arg("mcp")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let mut stdin = child.stdin.take().unwrap();
    let mut reader = BufReader::new(child.stdout.take().unwrap());
    send(&mut stdin, serde_json::json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"test","version":"0"}}}));
    recv(&mut reader, 1);
    send(&mut stdin, serde_json::json!({"jsonrpc":"2.0","method":"notifications/initialized"}));
    send(&mut stdin, serde_json::json!({"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"tickets_overview","arguments":{"status":"in-progress"}}}));
    let resp = recv(&mut reader, 2);
    assert_ne!(resp["result"]["isError"], true, "{resp}");
    let view: serde_json::Value = serde_json::from_str(resp["result"]["content"][0]["text"].as_str().unwrap()).unwrap();
    assert_eq!(view["tickets"].as_array().unwrap().len(), 1, "{view}");
    assert_eq!(view["tickets"][0]["title"], "Chantier distant");
    assert!(view["warnings"][0]["message"].as_str().unwrap().contains("introuvable"), "{view}");
    drop(stdin);
    let _ = child.kill();
    let _ = child.wait();
}

// ---------------------------------------------------------------------------
// MCP stdio
// ---------------------------------------------------------------------------

fn send(stdin: &mut impl Write, msg: serde_json::Value) {
    writeln!(stdin, "{msg}").unwrap();
    stdin.flush().unwrap();
}

fn recv(reader: &mut impl BufRead, id: i64) -> serde_json::Value {
    loop {
        let mut line = String::new();
        assert!(reader.read_line(&mut line).unwrap() > 0, "flux MCP fermé");
        let v: serde_json::Value = serde_json::from_str(&line).unwrap();
        if v.get("id").and_then(|x| x.as_i64()) == Some(id) {
            return v;
        }
    }
}

#[test]
fn mcp_stdio() {
    let env = Env::new();
    env.ok(&["init"]);
    let mut child = env
        .cmd(BIN)
        .arg("mcp")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let mut stdin = child.stdin.take().unwrap();
    let mut reader = BufReader::new(child.stdout.take().unwrap());

    send(&mut stdin, serde_json::json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"test","version":"0"}}}));
    let init = recv(&mut reader, 1);
    assert_eq!(init["result"]["serverInfo"]["name"], "coutcouticket", "{init}");
    send(&mut stdin, serde_json::json!({"jsonrpc":"2.0","method":"notifications/initialized"}));

    send(&mut stdin, serde_json::json!({"jsonrpc":"2.0","id":2,"method":"tools/list"}));
    let tools = recv(&mut reader, 2);
    let names: Vec<String> = tools["result"]["tools"].as_array().unwrap().iter().map(|t| t["name"].as_str().unwrap().to_string()).collect();
    for expected in ["ticket_create", "ticket_start", "ticket_set_status", "ticket_log", "ticket_decide", "ticket_list", "ticket_context", "ticket_files", "notes_validate", "ticket_depend", "tickets_overview"] {
        assert!(names.contains(&expected.to_string()), "outil {expected} absent : {names:?}");
    }
    let status_schema = tools["result"]["tools"].as_array().unwrap().iter().find(|t| t["name"] == "ticket_set_status").unwrap().to_string();
    assert!(status_schema.contains("in-progress"), "enum de statut absent du schéma : {status_schema}");

    let repo = env.repo.display().to_string();
    send(&mut stdin, serde_json::json!({"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"ticket_create","arguments":{"project":repo,"title":"Via MCP","type":"design","acceptance":["ok"]}}}));
    let created = recv(&mut reader, 3).to_string();
    assert!(created.contains("design/0001-via-mcp"), "{created}");

    send(&mut stdin, serde_json::json!({"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"ticket_set_status","arguments":{"project":repo,"id":"1","status":"en cours"}}}));
    let invalid = recv(&mut reader, 4).to_string();
    assert!(invalid.contains("error") || invalid.contains("isError"), "statut invalide accepté : {invalid}");

    send(&mut stdin, serde_json::json!({"jsonrpc":"2.0","id":6,"method":"tools/call","params":{"name":"ticket_create","arguments":{"project":repo,"title":"Dépendant","blocked_by":["1"]}}}));
    let dependent = recv(&mut reader, 6).to_string();
    assert!(dependent.contains("open_blockers") && dependent.contains("0001"), "{dependent}");
    send(&mut stdin, serde_json::json!({"jsonrpc":"2.0","id":7,"method":"tools/call","params":{"name":"ticket_depend","arguments":{"project":repo,"id":"1","on":["2"]}}}));
    let cycle = recv(&mut reader, 7).to_string();
    assert!(cycle.contains("isError") && cycle.contains("cycle"), "cycle accepté : {cycle}");
    send(&mut stdin, serde_json::json!({"jsonrpc":"2.0","id":8,"method":"tools/call","params":{"name":"ticket_depend","arguments":{"project":repo,"id":"2","on":["1"],"remove":true}}}));
    let removed = recv(&mut reader, 8).to_string();
    assert!(removed.contains("blocked_by") && !removed.contains("isError\":true"), "{removed}");
    send(&mut stdin, serde_json::json!({"jsonrpc":"2.0","id":9,"method":"tools/call","params":{"name":"ticket_context","arguments":{"project":repo,"id":"2"}}}));
    let ctx = recv(&mut reader, 9).to_string();
    assert!(ctx.contains("open_blockers"), "{ctx}");

    send(&mut stdin, serde_json::json!({"jsonrpc":"2.0","id":5,"method":"tools/call","params":{"name":"notes_validate","arguments":{"project":repo}}}));
    let valid = recv(&mut reader, 5).to_string();
    assert!(valid.contains("notes valides"), "{valid}");

    drop(stdin);
    let _ = child.kill();
    let _ = child.wait();
}

// ---------------------------------------------------------------------------
// Démon HTTP
// ---------------------------------------------------------------------------

struct Kill(Child);
impl Drop for Kill {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn free_port() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port()
}

fn http(port: u16, method: &str, path: &str, headers: &[(&str, &str)], body: &str) -> (u16, String, String) {
    let mut s = TcpStream::connect(("127.0.0.1", port)).unwrap();
    s.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
    let mut req = format!("{method} {path} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nConnection: close\r\nContent-Length: {}\r\n", body.len());
    for (k, v) in headers {
        req.push_str(&format!("{k}: {v}\r\n"));
    }
    req.push_str("\r\n");
    req.push_str(body);
    s.write_all(req.as_bytes()).unwrap();
    let mut resp = String::new();
    let _ = s.read_to_string(&mut resp);
    let status: u16 = resp.split_whitespace().nth(1).unwrap_or("0").parse().unwrap_or(0);
    let (head, body) = resp.split_once("\r\n\r\n").unwrap_or((&resp, ""));
    (status, head.to_string(), body.to_string())
}

#[test]
fn daemon_http_auth_et_watcher() {
    let env = Env::new();
    env.ok(&["init"]);
    env.ok(&["new", "-t", "Premier ticket"]);
    let port = free_port();
    let token = "jeton-de-test";
    fs::write(env.home.join("daemon.toml"), format!("port = {port}\ntoken = \"{token}\"\n")).unwrap();
    let _daemon = Kill(env.cmd(BIN).args(["daemon", "run"]).stderr(Stdio::null()).spawn().unwrap());

    let start = Instant::now();
    while TcpStream::connect(("127.0.0.1", port)).is_err() {
        assert!(start.elapsed() < Duration::from_secs(10), "le démon ne démarre pas");
        std::thread::sleep(Duration::from_millis(50));
    }
    assert!(env.ok(&["daemon", "status"]).contains("ok"));

    let init = r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"test","version":"0"}}}"#;
    let accept = ("Accept", "application/json, text/event-stream");
    let ctype = ("Content-Type", "application/json");
    let bearer = format!("Bearer {token}");

    let (st, _, _) = http(port, "POST", "/mcp", &[accept, ctype], init);
    assert_eq!(st, 401, "sans jeton : doit être refusé");
    let (st, _, _) = http(port, "POST", "/mcp", &[accept, ctype, ("Authorization", "Bearer faux")], init);
    assert_eq!(st, 401, "mauvais jeton : doit être refusé");
    let (st, _, _) = http(port, "POST", "/mcp", &[accept, ctype, ("Authorization", &bearer), ("Origin", "http://evil.example")], init);
    assert_eq!(st, 403, "Origin navigateur : doit être refusé");

    let (st, head, body) = http(port, "POST", "/mcp", &[accept, ctype, ("Authorization", &bearer)], init);
    assert_eq!(st, 200, "{head}\n{body}");
    assert!(body.contains("coutcouticket"), "{body}");
    let session = head
        .lines()
        .find_map(|l| l.to_ascii_lowercase().starts_with("mcp-session-id:").then(|| l.split_once(':').unwrap().1.trim().to_string()))
        .expect("en-tête mcp-session-id absent");
    let sess = ("Mcp-Session-Id", session.as_str());
    let proto = ("MCP-Protocol-Version", "2025-06-18");
    http(port, "POST", "/mcp", &[accept, ctype, ("Authorization", &bearer), sess, proto], r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#);

    // project obligatoire en mode démon
    let (_, _, body) = http(port, "POST", "/mcp", &[accept, ctype, ("Authorization", &bearer), sess, proto],
        r#"{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"ticket_list","arguments":{}}}"#);
    assert!(body.contains("obligatoire"), "{body}");

    let call = serde_json::json!({"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"ticket_list","arguments":{"project": env.repo.display().to_string()}}}).to_string();
    let (_, _, body) = http(port, "POST", "/mcp", &[accept, ctype, ("Authorization", &bearer), sess, proto], &call);
    assert!(body.contains("Premier ticket"), "{body}");

    // tickets_overview : pas de paramètre project, même en mode démon
    let (_, _, body) = http(port, "POST", "/mcp", &[accept, ctype, ("Authorization", &bearer), sess, proto],
        r#"{"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"tickets_overview","arguments":{}}}"#);
    assert!(body.contains("Premier ticket") && body.contains("project_path") && !body.contains("obligatoire"), "{body}");

    // watcher : une édition manuelle doit régénérer BOARD.md, même pendant que
    // quelqu'un relit les notes en continu (la boucle ci-dessous lit toutes les 100 ms,
    // ce qui empêchait la régénération avant la correction de l'anti-rebond)
    let ticket = env.notes().join("tickets/0001-premier-ticket/ticket.md");
    let content = fs::read_to_string(&ticket).unwrap();
    fs::write(&ticket, content.replace("priority: p2", "priority: p0")).unwrap();
    let board = env.notes().join("0-global/BOARD.md");
    let start = Instant::now();
    loop {
        if fs::read_to_string(&board).unwrap().contains("| p0 |") {
            break;
        }
        assert!(start.elapsed() < Duration::from_secs(10), "BOARD.md non régénéré par le watcher");
        std::thread::sleep(Duration::from_millis(100));
    }

    // OVERVIEW.md (config globale) suit aussi les éditions, et le registre.
    let overview = env.home.join("OVERVIEW.md");
    let wait_overview = |needle: &str| {
        let start = Instant::now();
        loop {
            let text = fs::read_to_string(&overview).unwrap_or_default();
            if text.contains(needle) {
                return text;
            }
            assert!(start.elapsed() < Duration::from_secs(10), "OVERVIEW.md sans « {needle} » :\n{text}");
            std::thread::sleep(Duration::from_millis(100));
        }
    };
    let text = wait_overview("| p0 |");
    assert!(text.contains("| mon-projet | [0001](<") && text.contains("Premier ticket"), "{text}");
    let registry = fs::read_to_string(env.home.join("projects.toml")).unwrap();
    fs::write(env.home.join("projects.toml"), registry.replace("projects = [", "projects = [\"/chemin/disparu\", ")).unwrap();
    wait_overview("`/chemin/disparu` : projet introuvable");
}

/// L'écoute précède l'initialisation du watcher, et le journal est horodaté
/// pour mesurer le délai de démarrage (ticket 0009).
#[test]
fn daemon_ecoute_avant_le_watcher() {
    let env = Env::new();
    env.ok(&["init"]);
    let port = free_port();
    fs::write(env.home.join("daemon.toml"), format!("port = {port}\ntoken = \"t\"\n")).unwrap();
    let log = env.home.join("daemon.log");
    let file = fs::File::create(&log).unwrap();
    let _daemon = Kill(env.cmd(BIN).args(["daemon", "run"]).stderr(file).spawn().unwrap());

    let start = Instant::now();
    let text = loop {
        let text = fs::read_to_string(&log).unwrap();
        if text.contains("surveillance prête") {
            break text;
        }
        assert!(start.elapsed() < Duration::from_secs(10), "watcher non prêt :\n{text}");
        std::thread::sleep(Duration::from_millis(50));
    };
    let pos = |needle: &str| text.find(needle).unwrap_or_else(|| panic!("« {needle} » absent :\n{text}"));
    assert!(pos("lancement du démon") < pos("à l'écoute"), "{text}");
    assert!(pos("à l'écoute") < pos("surveillance de "), "{text}");
    assert!(pos("à l'écoute") < pos("surveillance prête (1 projet(s)"), "{text}");
    // Chaque ligne commence par « AAAA-MM-JJ HH:MM:SS.mmm coutcouticket : ».
    for line in text.lines() {
        let (stamp, rest) = line.split_at_checked(23).unwrap_or_else(|| panic!("ligne non horodatée : {line}"));
        assert!(stamp.as_bytes()[4] == b'-' && stamp.as_bytes()[10] == b' ' && stamp.as_bytes()[19] == b'.', "{line}");
        assert!(rest.starts_with(" coutcouticket : "), "{line}");
    }
    assert!(TcpStream::connect(("127.0.0.1", port)).is_ok());
}

/// Requête HTTP brute (en-tête Host libre). Rend (statut, en-têtes en minuscules, corps).
fn raw_http(port: u16, request: &str) -> (u16, String, String) {
    let mut s = TcpStream::connect(("127.0.0.1", port)).unwrap();
    s.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
    s.write_all(request.as_bytes()).unwrap();
    let mut resp = String::new();
    let _ = s.read_to_string(&mut resp);
    let status: u16 = resp.split_whitespace().nth(1).unwrap_or("0").parse().unwrap_or(0);
    let (head, body) = resp.split_once("\r\n\r\n").unwrap_or((&resp, ""));
    (status, head.to_ascii_lowercase(), body.to_string())
}

fn get(port: u16, path: &str, headers: &[(&str, &str)]) -> (u16, String, String) {
    let mut req = format!("GET {path} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nConnection: close\r\n");
    for (k, v) in headers {
        req.push_str(&format!("{k}: {v}\r\n"));
    }
    raw_http(port, &(req + "\r\n"))
}

fn encode(s: &str) -> String {
    s.bytes()
        .map(|b| if b.is_ascii_alphanumeric() || b"-_.~".contains(&b) { (b as char).to_string() } else { format!("%{b:02X}") })
        .collect()
}

/// Panneau web (ticket 0026) : connexion par code à usage unique, cookie de session,
/// en-têtes de sécurité, API limitée aux projets enregistrés, Host et Origin, SSE.
#[test]
fn panneau_web() {
    let env = Env::new();
    // Sans configuration du démon, puis démon arrêté : message explicite.
    let out = env.cct(&["ui", "--print"]);
    assert!(!out.status.success() && stderr(&out).contains("daemon install"), "{}", stderr(&out));
    env.ok(&["init"]);
    env.ok(&["new", "-t", "Premier ticket", "-a", "Critère visible"]);
    env.ok(&["log", "1", "-t", "Entrée de journal", "-n", "Suite"]);
    let port = free_port();
    fs::write(env.home.join("daemon.toml"), format!("port = {port}\ntoken = \"jeton-ui\"\n")).unwrap();
    let out = env.cct(&["ui", "--print"]);
    assert!(!out.status.success(), "démon arrêté : doit échouer");
    let err = stderr(&out);
    assert!(err.contains(&format!("injoignable sur le port {port}")) && err.contains("daemon install") && err.contains("daemon status"), "{err}");

    let _daemon = Kill(env.cmd(BIN).args(["daemon", "run"]).stderr(Stdio::null()).spawn().unwrap());
    let start = Instant::now();
    while !env.home.join("OVERVIEW.md").exists() {
        assert!(start.elapsed() < Duration::from_secs(10), "le démon ne démarre pas");
        std::thread::sleep(Duration::from_millis(50));
    }

    // Sans session : 401, page explicative, en-têtes de sécurité.
    let (st, head, body) = get(port, "/ui/", &[]);
    assert_eq!(st, 401, "{head}");
    assert!(body.contains("coutcouticket ui"), "{body}");
    for needle in [
        "content-security-policy: default-src 'none'; script-src 'self'; style-src 'self'",
        "frame-ancestors 'none'",
        "x-content-type-options: nosniff",
        "referrer-policy: no-referrer",
        "x-frame-options: deny",
        "cache-control: no-store",
    ] {
        assert!(head.contains(needle), "{needle} absent :\n{head}");
    }
    let (st, _, body) = get(port, "/ui/api/overview", &[]);
    assert!(st == 401 && body.contains("\"error\"") && !body.contains("Premier"), "{st} {body}");
    let (st, _, _) = get(port, "/ui/api/events", &[]);
    assert_eq!(st, 401);

    // Code de connexion : Bearer obligatoire, jamais depuis un navigateur.
    let post = |extra: &str| {
        raw_http(port, &format!("POST /ui-login-code HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nContent-Length: 0\r\nConnection: close\r\n{extra}\r\n"))
    };
    assert_eq!(post("").0, 401);
    assert_eq!(post("Authorization: Bearer faux\r\n").0, 401);
    assert_eq!(post(&format!("Authorization: Bearer jeton-ui\r\nOrigin: http://127.0.0.1:{port}\r\n")).0, 403);

    let url = env.ok(&["ui", "--print"]).trim().to_string();
    let prefix = format!("http://127.0.0.1:{port}/ui/login?code=");
    assert!(url.starts_with(&prefix), "{url}");
    let login = &url[format!("http://127.0.0.1:{port}").len()..];
    let (st, _, _) = get(port, "/ui/login?code=0123abcd", &[]);
    assert_eq!(st, 401, "code inventé accepté");
    let (st, head, body) = get(port, login, &[]);
    assert_eq!(st, 200, "{head}\n{body}");
    let cookie_line = head.lines().find(|l| l.starts_with("set-cookie:")).expect(&head).to_string();
    for needle in ["cct_ui=", "path=/ui", "httponly", "samesite=strict"] {
        assert!(cookie_line.contains(needle), "{needle} absent : {cookie_line}");
    }
    assert!(body.contains("http-equiv=\"refresh\" content=\"0; url=./\""), "{body}");
    let (st, _, _) = get(port, login, &[]);
    assert_eq!(st, 401, "code réutilisable");
    // Cookie tel que le navigateur le renverra (valeur hexadécimale : la mise en minuscules ne l'altère pas).
    let cookie = cookie_line.trim_start_matches("set-cookie:").trim().split(';').next().unwrap().to_string();
    let c = ("Cookie", cookie.as_str());
    let (st, _, _) = get(port, "/ui/", &[("Cookie", "cct_ui=faux")]);
    assert_eq!(st, 401, "faux cookie accepté");

    // Page, ressources et API avec la session.
    let (st, head, body) = get(port, "/ui/", &[c, ("Sec-Fetch-Site", "none")]);
    assert_eq!(st, 200, "{head}");
    assert!(head.contains("content-type: text/html") && head.contains("content-security-policy") && body.contains("app.js"), "{head}");
    let (st, head, _) = get(port, "/ui/app.js", &[]);
    assert!(st == 200 && head.contains("content-type: text/javascript") && head.contains("nosniff"), "{head}");
    let (st, head, _) = get(port, "/ui", &[c]);
    assert!(st == 308 && head.contains("location: /ui/"), "{head}");
    let origin = format!("http://127.0.0.1:{port}");
    let (st, head, body) = get(port, "/ui/api/overview", &[c, ("Origin", &origin), ("Sec-Fetch-Site", "same-origin")]);
    assert_eq!(st, 200, "{head}\n{body}");
    assert!(head.contains("application/json") && head.contains("cache-control: no-store"), "{head}");
    let overview: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(overview["tickets"][0]["title"], "Premier ticket", "{body}");
    let project_path = overview["tickets"][0]["project_path"].as_str().unwrap().to_string();
    assert_eq!(project_path, canon(&env.repo).display().to_string());

    let ticket = |project: &str, id: &str| get(port, &format!("/ui/api/ticket?project={}&id={id}", encode(project)), &[c]);
    let (st, _, body) = ticket(&project_path, "1");
    assert_eq!(st, 200, "{body}");
    let detail: serde_json::Value = serde_json::from_str(&body).unwrap();
    let text = detail["ticket"].as_str().unwrap();
    assert!(text.starts_with("## Description") && text.contains("Critère visible") && !text.contains("status:"), "{text}");
    let journal = detail["journal"].as_str().unwrap();
    assert!(journal.contains("Entrée de journal") && !journal.contains("<!--"), "{journal}");
    assert!(detail["decisions"].as_str().unwrap().starts_with("# Décisions"), "{body}");
    assert!(detail["ticket_file"].as_str().unwrap().ends_with("ticket.md"), "{body}");

    // Refus : id invalide, ticket absent, projet non enregistré, chemin détourné.
    assert_eq!(ticket(&project_path, "abc").0, 400);
    assert_eq!(ticket(&project_path, "..%2F..").0, 400);
    assert_eq!(ticket(&project_path, "42").0, 404);
    let other = env.repo.parent().unwrap().join("hors-registre");
    fs::create_dir_all(&other).unwrap();
    env.ok(&["init", "--no-register", "--no-git-hooks", other.to_str().unwrap()]);
    env.ok(&["-C", other.to_str().unwrap(), "new", "-t", "Secret"]);
    let (st, _, body) = ticket(&canon(&other).display().to_string(), "1");
    assert!(st == 404 && body.contains("non enregistré") && !body.contains("Secret"), "{st} {body}");
    let detour = format!("{project_path}{0}..{0}hors-registre", std::path::MAIN_SEPARATOR);
    assert_eq!(ticket(&detour, "1").0, 404);
    assert_eq!(get(port, "/ui/api/ticket?id=1", &[c]).0, 400);

    // Origine, site et hôte étrangers : refusés même avec la session.
    for bad in [("Origin", "http://evil.example"), ("Origin", "null"), ("Origin", "http://127.0.0.1:1"), ("Sec-Fetch-Site", "cross-site"), ("Sec-Fetch-Site", "same-site")] {
        let (st, _, body) = get(port, "/ui/api/overview", &[c, bad]);
        assert!(st == 403 && !body.contains("Premier"), "{bad:?} : {st} {body}");
    }
    for host in ["evil.example", &format!("evil.example:{port}"), "127.0.0.1:1"] {
        let (st, _, body) = raw_http(port, &format!("GET /ui/api/overview HTTP/1.1\r\nHost: {host}\r\nCookie: {cookie}\r\nConnection: close\r\n\r\n"));
        assert!(st == 403 && !body.contains("Premier"), "Host {host} : {st} {body}");
    }
    let (st, _, _) = raw_http(port, &format!("GET /ui/api/overview HTTP/1.1\r\nHost: localhost:{port}\r\nCookie: {cookie}\r\nConnection: close\r\n\r\n"));
    assert_eq!(st, 200, "localhost refusé");
    // /mcp garde son comportement : Origin toujours refusé.
    let (st, _, _) = http(port, "POST", "/mcp", &[("Authorization", "Bearer jeton-ui"), ("Origin", &origin), ("Content-Type", "application/json"), ("Accept", "application/json, text/event-stream")], "{}");
    assert_eq!(st, 403);

    // SSE : un événement après modification d'une note.
    let mut sse = TcpStream::connect(("127.0.0.1", port)).unwrap();
    write!(sse, "GET /ui/api/events HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nCookie: {cookie}\r\nAccept: text/event-stream\r\n\r\n").unwrap();
    sse.set_read_timeout(Some(Duration::from_millis(500))).unwrap();
    let mut received = Vec::new();
    let mut buf = [0u8; 4096];
    let start = Instant::now();
    let mut edited = false;
    loop {
        match sse.read(&mut buf) {
            Ok(0) => panic!("flux SSE fermé : {}", String::from_utf8_lossy(&received)),
            Ok(n) => received.extend_from_slice(&buf[..n]),
            Err(_) => {}
        }
        let text = String::from_utf8_lossy(&received).to_string();
        if !edited && text.contains("\r\n\r\n") {
            let head = text.to_ascii_lowercase();
            assert!(head.starts_with("http/1.1 200") && head.contains("content-type: text/event-stream"), "{text}");
            assert!(!text.contains("data: changed"), "événement avant toute modification : {text}");
            let journal = env.notes().join("tickets/0001-premier-ticket/journal.md");
            let content = fs::read_to_string(&journal).unwrap();
            fs::write(&journal, content + "\nÉdition manuelle.\n").unwrap();
            edited = true;
        }
        if text.contains("data: changed") {
            break;
        }
        assert!(start.elapsed() < Duration::from_secs(15), "aucun événement SSE :\n{text}");
    }
}

/// `daemon run --log-file` (service Windows) : journal dans le fichier, pid à côté.
#[test]
fn daemon_journal_dans_un_fichier() {
    let env = Env::new();
    env.ok(&["init"]);
    let port = free_port();
    fs::write(env.home.join("daemon.toml"), format!("port = {port}\ntoken = \"t\"\n")).unwrap();
    let log = env.home.join("logs").join("daemon.log");
    let daemon = Kill(
        env.cmd(BIN)
            .args(["daemon", "run", "--log-file"])
            .arg(&log)
            .stderr(Stdio::piped())
            .spawn()
            .unwrap(),
    );
    let start = Instant::now();
    let text = loop {
        let text = fs::read_to_string(&log).unwrap_or_default();
        if text.contains("surveillance prête") {
            break text;
        }
        assert!(start.elapsed() < Duration::from_secs(10), "journal incomplet :\n{text}");
        std::thread::sleep(Duration::from_millis(50));
    };
    assert!(text.contains("à l'écoute sur http://127.0.0.1:"), "{text}");
    let pid = fs::read_to_string(log.with_file_name("daemon.pid")).unwrap();
    assert_eq!(pid, daemon.0.id().to_string());

    // Seconde instance : refusée, l'erreur va au journal et le pid n'est pas écrasé.
    let out = env.cmd(BIN).args(["daemon", "run", "--log-file"]).arg(&log).output().unwrap();
    assert!(!out.status.success());
    let text = fs::read_to_string(&log).unwrap();
    assert!(text.contains("arrêt sur erreur : impossible d'écouter"), "{text}");
    assert_eq!(fs::read_to_string(log.with_file_name("daemon.pid")).unwrap(), pid);
}


// ---------------------------------------------------------------------------
// .gitignore du dossier de notes
// ---------------------------------------------------------------------------

#[test]
fn gitignore_des_notes() {
    // --- sans .gitignore : créé, notes invisibles pour git, idempotent
    let env = Env::new();
    let gitignore = env.repo.join(".gitignore");
    env.ok(&["init"]);
    assert_eq!(fs::read_to_string(&gitignore).unwrap(), "/0-notes/\n");
    let status = env.git_ok(&["status", "--porcelain", "--untracked-files=all"]);
    assert!(!status.contains("0-notes"), "{status}");
    let again = env.ok(&["init"]);
    assert!(again.contains("déjà complète"), "{again}");
    assert_eq!(fs::read_to_string(&gitignore).unwrap(), "/0-notes/\n");

    // --- commit avec notes ignorées : BOARD.md régénéré sans git add
    env.git_ok(&["add", "-A"]);
    env.git_ok(&["commit", "-q", "-m", "init"]);
    env.ok(&["new", "-t", "Premier ticket"]);
    fs::write(env.notes().join("0-global/BOARD.md"), "périmé").unwrap();
    let out = env.git(&["commit", "-q", "--allow-empty", "-m", "board ignoré"]);
    assert!(out.status.success(), "commit refusé :\n{}", stderr(&out));
    assert!(stderr(&out).contains("non ajouté au commit"), "{}", stderr(&out));
    assert!(env.git_ok(&["ls-files", "0-notes"]).is_empty());

    // --- .gitignore existant sans fin de ligne : seule la règle est ajoutée
    let env = Env::new();
    let gitignore = env.repo.join(".gitignore");
    let original = "# Dépendances\nnode_modules/\ndist";
    fs::write(&gitignore, original).unwrap();
    let out = env.ok(&["init"]);
    assert!(out.contains("mis à jour .gitignore"), "{out}");
    assert_eq!(fs::read_to_string(&gitignore).unwrap(), format!("{original}\n/0-notes/\n"));

    // --- règle équivalente déjà présente : rien n'est ajouté
    let env = Env::new();
    let gitignore = env.repo.join(".gitignore");
    fs::write(&gitignore, "0-notes/\n").unwrap();
    env.ok(&["init"]);
    assert_eq!(fs::read_to_string(&gitignore).unwrap(), "0-notes/\n");

    // --- --no-gitignore
    let env = Env::new();
    env.ok(&["init", "--no-gitignore"]);
    assert!(!env.repo.join(".gitignore").exists());

    // --- notes déjà versionnées : .gitignore intact, avertissement, index intact
    env.git_ok(&["add", "-A"]);
    env.git_ok(&["commit", "-q", "-m", "notes versionnées"]);
    let tracked = env.git_ok(&["ls-files", "0-notes"]);
    assert!(!tracked.is_empty());
    let out = env.ok(&["init"]);
    assert!(out.contains("git rm -r --cached 0-notes"), "{out}");
    assert!(out.contains("écrase les notes locales"), "{out}");
    assert!(!env.repo.join(".gitignore").exists());
    assert_eq!(env.git_ok(&["ls-files", "0-notes"]), tracked);

    // --- notes_dir personnalisé
    let env = Env::new();
    fs::write(env.repo.join(".coutcouticket.toml"), "notes_dir = \"docs/notes\"\n").unwrap();
    env.ok(&["init"]);
    assert_eq!(fs::read_to_string(env.repo.join(".gitignore")).unwrap(), "/docs/notes/\n");
    let status = env.git_ok(&["status", "--porcelain", "--untracked-files=all"]);
    assert!(!status.contains("docs/notes"), "{status}");
    assert!(env.repo.join("docs/notes/0-global/BOARD.md").is_file());
}

// ---------------------------------------------------------------------------
// setup-claude --apply (binaire claude simulé)
// ---------------------------------------------------------------------------

/// Faux `claude` : l'enregistrement vit dans `$FAKE_CLAUDE_STATE` (fichier absent =
/// serveur absent), chaque appel est journalisé dans `$FAKE_CLAUDE_LOG`. Reproduit le
/// format de `claude mcp get` et le refus de `mcp add` sur un nom existant.
#[cfg(unix)]
const FAKE_CLAUDE: &str = r#"#!/bin/sh
echo "$*" >> "$FAKE_CLAUDE_LOG"
state="$FAKE_CLAUDE_STATE"
case "$1 $2" in
"mcp get")
  if [ ! -f "$state" ]; then echo "No MCP server named \"$3\". Configured servers: autre" >&2; exit 1; fi
  . "$state"
  case "$SCOPE" in user) label="User config (available in all your projects)";; *) label="Local config (private to you in this project)";; esac
  printf '%s:\n  Scope: %s\n  Status: ✘ Failed to connect\n  Type: http\n  URL: %s\n  Headers:\n    Authorization: %s\n\nTo remove this server, run: claude mcp remove %s -s %s\n' "$3" "$label" "$URL" "$AUTH" "$3" "$SCOPE"
  ;;
"mcp list")
  if [ -f "$state" ]; then . "$state"; echo "coutcouticket: $URL (HTTP) - ✘ Failed to connect"; fi
  ;;
"mcp add")
  if [ -f "$state" ]; then . "$state"; echo "MCP server $7 already exists in $SCOPE config" >&2; exit 1; fi
  printf "SCOPE='%s'\nURL='%s'\nAUTH='%s'\n" "$6" "$8" "${10#Authorization: }" > "$state"
  echo "Added HTTP MCP server $7 with URL: $8 to $6 config"
  ;;
"mcp remove")
  if [ ! -f "$state" ]; then echo "No MCP server found with name: $3" >&2; exit 1; fi
  . "$state"
  if [ "$4" = "--scope" ] && [ "$5" != "$SCOPE" ]; then echo "No MCP server found with name: $3 in $5 config" >&2; exit 1; fi
  rm "$state"
  echo "Removed MCP server $3 from $SCOPE config"
  ;;
*) echo "commande simulée inconnue : $*" >&2; exit 2;;
esac
"#;

#[cfg(unix)]
#[test]
fn setup_claude_idempotent() {
    use std::os::unix::fs::PermissionsExt;
    let env = Env::new();
    let fake = env.home.join("claude-simule");
    fs::write(&fake, FAKE_CLAUDE).unwrap();
    fs::set_permissions(&fake, fs::Permissions::from_mode(0o755)).unwrap();
    let state = env.home.join("claude-etat");
    let log = env.home.join("claude-appels.log");
    fs::write(env.home.join("daemon.toml"), "port = 49999\ntoken = \"jeton-attendu\"\n").unwrap();
    let apply = || {
        let _ = fs::remove_file(&log);
        let out = env
            .cmd(BIN)
            .args(["setup-claude", "--apply"])
            .env("COUTCOUTICKET_CLAUDE_BIN", &fake)
            .env("FAKE_CLAUDE_STATE", &state)
            .env("FAKE_CLAUDE_LOG", &log)
            .output()
            .unwrap();
        let calls = fs::read_to_string(&log).unwrap_or_default();
        (out, calls)
    };
    let want_state = "SCOPE='user'\nURL='http://127.0.0.1:49999/mcp'\nAUTH='Bearer jeton-attendu'\n";

    // --- absent : ajouté
    let (out, calls) = apply();
    assert!(out.status.success(), "{}", stderr(&out));
    assert!(stdout(&out).contains("enregistré dans Claude Code (portée user)"), "{}", stdout(&out));
    assert!(calls.contains("mcp add --transport http --scope user coutcouticket http://127.0.0.1:49999/mcp --header Authorization: Bearer jeton-attendu"), "{calls}");
    assert_eq!(fs::read_to_string(&state).unwrap(), want_state);

    // --- identique : second --apply sans effet, code 0
    let (out, calls) = apply();
    assert!(out.status.success(), "{}", stderr(&out));
    assert!(stdout(&out).contains("déjà enregistré") && stdout(&out).contains("à jour"), "{}", stdout(&out));
    assert_eq!(calls, "mcp get coutcouticket\n", "aucune écriture attendue");

    // --- différent (autre jeton et port) : retiré de sa portée puis recréé
    fs::write(&state, "SCOPE='user'\nURL='http://127.0.0.1:47813/mcp'\nAUTH='Bearer ancien-jeton'\n").unwrap();
    let (out, calls) = apply();
    assert!(out.status.success(), "{}", stderr(&out));
    assert!(stdout(&out).contains("mis à jour") && stdout(&out).contains("portée user"), "{}", stdout(&out));
    let verbs: Vec<String> = calls.lines().map(|l| l.split(' ').take(2).collect::<Vec<_>>().join(" ")).collect();
    assert_eq!(verbs, ["mcp get", "mcp remove", "mcp get", "mcp add", "mcp get"], "{calls}");
    assert!(calls.contains("mcp remove coutcouticket --scope user\n"), "{calls}");
    assert_eq!(fs::read_to_string(&state).unwrap(), want_state);
    let list = env.cmd(fake.to_str().unwrap()).args(["mcp", "list"]).env("FAKE_CLAUDE_STATE", &state).env("FAKE_CLAUDE_LOG", &log).output().unwrap();
    assert!(stdout(&list).contains("http://127.0.0.1:49999/mcp"), "{}", stdout(&list));

    // --- différent dans une autre portée : le remove cible cette portée
    fs::write(&state, "SCOPE='local'\nURL='http://127.0.0.1:49999/mcp'\nAUTH='Bearer ancien-jeton'\n").unwrap();
    let (out, calls) = apply();
    assert!(out.status.success(), "{}", stderr(&out));
    assert!(stdout(&out).contains("portée local"), "{}", stdout(&out));
    assert!(calls.contains("mcp remove coutcouticket --scope local"), "{calls}");
    assert_eq!(fs::read_to_string(&state).unwrap(), want_state);
    assert!(!stdout(&out).contains("ancien-jeton") && !stderr(&out).contains("ancien-jeton"));

    // --- échec de claude : message sans commande manuelle vouée à échouer, jeton masqué
    fs::write(&fake, "#!/bin/sh\necho \"$*\" >> \"$FAKE_CLAUDE_LOG\"\n[ \"$2\" = get ] && exit 1\necho \"refus simulé $*\" >&2\nexit 1\n").unwrap();
    let (out, _) = apply();
    assert!(!out.status.success());
    let err = stderr(&out);
    assert!(err.contains("« claude mcp add » a échoué") && err.contains("setup-claude --apply"), "{err}");
    assert!(!err.contains("jeton-attendu"), "jeton affiché : {err}");
    assert!(!err.contains("à la main"), "{err}");

    // --- claude introuvable
    let out = env
        .cmd(BIN)
        .args(["setup-claude", "--apply"])
        .env("COUTCOUTICKET_CLAUDE_BIN", env.home.join("inexistant"))
        .output()
        .unwrap();
    assert!(!out.status.success());
    assert!(stderr(&out).contains("« claude » introuvable"), "{}", stderr(&out));
}

// ---------------------------------------------------------------------------
// git en échec (faux binaire via COUTCOUTICKET_GIT_BIN)
// ---------------------------------------------------------------------------

/// Reproduit /usr/bin/git sous macOS quand la licence Xcode n'est pas acceptée.
#[cfg(unix)]
const FAKE_GIT_XCODE: &str = "#!/bin/sh
echo \"You have not agreed to the Xcode license agreements. Please run 'sudo xcodebuild -license' from within a Terminal window to review and agree to the Xcode and Apple SDKs license.\" >&2
exit 69
";

/// Vrai git pour rev-parse (le dépôt est reconnu), panne générique pour le reste.
#[cfg(unix)]
const FAKE_GIT_BROKEN: &str = "#!/bin/sh
case \" $* \" in
  *\" rev-parse \"*) exec git \"$@\" ;;
esac
echo \"fatal: panne simulée\" >&2
exit 128
";

#[cfg(unix)]
#[test]
fn git_en_echec() {
    use std::os::unix::fs::PermissionsExt;
    let env = Env::new();
    env.ok(&["init"]);
    env.ok(&["new", "-t", "Diagnostiquer git"]);
    let fake = |name: &str, script: &str| {
        let path = env.home.join(name);
        fs::write(&path, script).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
        path
    };
    let xcode = fake("git-xcode", FAKE_GIT_XCODE);
    let broken = fake("git-casse", FAKE_GIT_BROKEN);
    let with_git = |bin: &Path, args: &[&str]| env.cmd(BIN).args(args).env("COUTCOUTICKET_GIT_BIN", bin).stdin(Stdio::null()).output().unwrap();

    // --- licence Xcode : files échoue avec commande, code, stderr et correction
    let out = with_git(&xcode, &["files", "1"]);
    assert!(!out.status.success());
    let err = stderr(&out);
    assert!(err.contains("« git rev-parse --is-inside-work-tree » a échoué (code 69)"), "{err}");
    assert!(err.contains("You have not agreed") && err.contains("sudo xcodebuild -license"), "{err}");
    assert!(err.contains("sudo xcode-select -s /Library/Developer/CommandLineTools"), "{err}");
    assert!(!err.contains("pas un dépôt git"), "diagnostic erroné : {err}");

    // --- show : l'erreur est exposée (git_error), pas un simple null
    let out = with_git(&xcode, &["show", "1", "--json"]);
    assert!(out.status.success(), "{}", stderr(&out));
    let ctx: serde_json::Value = serde_json::from_str(&stdout(&out)).unwrap();
    assert!(ctx["current_branch"].is_null(), "{ctx}");
    let git_error = ctx["git_error"].as_str().unwrap_or_default();
    assert!(git_error.contains("code 69") && git_error.contains("sudo xcodebuild -license"), "{ctx}");
    let out = with_git(&xcode, &["show", "1"]);
    assert!(stdout(&out).contains("git en échec"), "{}", stdout(&out));
    let ok: serde_json::Value = serde_json::from_str(&env.ok(&["show", "1", "--json"])).unwrap();
    assert!(ok["git_error"].is_null() && ok["current_branch"] == "main", "{ok}");

    // --- MCP : ticket_context porte git_error
    let mut child = env
        .cmd(BIN)
        .arg("mcp")
        .env("COUTCOUTICKET_GIT_BIN", &xcode)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let mut stdin = child.stdin.take().unwrap();
    let mut reader = BufReader::new(child.stdout.take().unwrap());
    send(&mut stdin, serde_json::json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"test","version":"0"}}}));
    recv(&mut reader, 1);
    send(&mut stdin, serde_json::json!({"jsonrpc":"2.0","method":"notifications/initialized"}));
    let repo = env.repo.display().to_string();
    send(&mut stdin, serde_json::json!({"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"ticket_context","arguments":{"project":repo,"id":"1"}}}));
    let ctx = recv(&mut reader, 2).to_string();
    assert!(ctx.contains("git_error") && ctx.contains("code 69"), "{ctx}");
    drop(stdin);
    let _ = child.kill();
    let _ = child.wait();

    // --- hook SessionStart : message clair, le hook ne casse pas
    let out = with_git(&xcode, &["hook", "session-start"]);
    assert!(out.status.success(), "{}", stderr(&out));
    assert!(stdout(&out).contains("git en échec") && stdout(&out).contains("xcodebuild -license"), "{}", stdout(&out));

    // --- init : avertissement, notes intactes, hooks et .gitignore non traités
    let out = with_git(&xcode, &["init"]);
    assert!(out.status.success(), "{}", stderr(&out));
    let text = format!("{}{}", stdout(&out), stderr(&out));
    assert!(text.contains("code 69") && text.contains("relancer init une fois git réparé"), "{text}");

    // --- autre code d'échec, dépôt reconnu : la commande fautive est nommée
    let out = with_git(&broken, &["files", "1"]);
    assert!(!out.status.success());
    let err = stderr(&out);
    assert!(err.contains("« git log --all") && err.contains("(code 128) : fatal: panne simulée"), "{err}");
    assert!(!err.contains("Correction"), "{err}");
    let out = with_git(&broken, &["start", "1"]);
    assert!(!out.status.success());
    assert!(stderr(&out).contains("« git show-ref --verify --quiet refs/heads/feat/0001-diagnostiquer-git » a échoué (code 128)"), "{}", stderr(&out));

    // --- binaire introuvable
    let out = with_git(&env.home.join("inexistant"), &["files", "1"]);
    assert!(!out.status.success());
    assert!(stderr(&out).contains("introuvable") && stderr(&out).contains("xcode-select --install"), "{}", stderr(&out));

    // --- vrai dossier hors dépôt : diagnostic inchangé
    let hors = env.home.join("hors-depot");
    fs::create_dir_all(&hors).unwrap();
    let out = env.cmd(BIN).args(["init"]).current_dir(&hors).output().unwrap();
    assert!(out.status.success(), "{}", stderr(&out));
    env.cmd(BIN).args(["new", "-t", "Sans git"]).current_dir(&hors).output().unwrap();
    let out = env.cmd(BIN).args(["files", "1"]).current_dir(&hors).output().unwrap();
    assert!(stderr(&out).contains("n'est pas un dépôt git"), "{}", stderr(&out));
    let out = env.cmd(BIN).args(["show", "1", "--json"]).current_dir(&hors).output().unwrap();
    let ctx: serde_json::Value = serde_json::from_str(&stdout(&out)).unwrap();
    assert!(ctx["git_error"].is_null() && ctx["current_branch"].is_null(), "{ctx}");
}

// ---------------------------------------------------------------------------
// Windows : hooks avec le sh de Git for Windows, tâche planifiée du démon
// ---------------------------------------------------------------------------

/// Hooks exécutés par le sh de Git for Windows, binaire noté au format `C:/…/coutcouticket.exe`,
/// avec un PATH qui ne contient pas le dossier du binaire (client git graphique).
#[cfg(windows)]
#[test]
fn hooks_git_for_windows() {
    let env = Env::new();
    env.ok(&["init"]);
    let pre_commit = env.repo.join(".git/hooks/pre-commit");
    let noted = BIN.replace('\\', "/");
    let hook = fs::read_to_string(&pre_commit).unwrap();
    assert!(hook.contains(&format!("cct='{noted}'")), "{hook}");
    assert!(!hook.contains('\r'), "fins de ligne CRLF dans le hook");

    // PATH sans aucun dossier contenant coutcouticket.exe.
    let without_bin = std::env::join_paths(
        std::env::split_paths(&std::env::var_os("PATH").unwrap()).filter(|d| !d.join("coutcouticket.exe").exists()),
    )
    .unwrap();
    let git_without_bin = |args: &[&str]| env.cmd("git").env("PATH", &without_bin).args(args).output().unwrap();

    // --- commit hors du PATH : accepté, trailer ajouté ; branche non conforme refusée
    env.ok(&["new", "-t", "Tester les hooks", "-k", "fix"]);
    env.git_ok(&["add", "-A"]);
    env.git_ok(&["commit", "-q", "-m", "init notes"]);
    env.ok(&["start", "1"]);
    let out = git_without_bin(&["commit", "-q", "--allow-empty", "-m", "depuis un client graphique"]);
    assert!(out.status.success(), "commit refusé :\n{}", stderr(&out));
    let msg = env.git_ok(&["log", "-1", "--pretty=%B"]);
    assert!(msg.contains("Ticket: 0001"), "trailer absent : {msg}");
    env.git_ok(&["switch", "-q", "-c", "wip"]);
    let refused = git_without_bin(&["commit", "-q", "--allow-empty", "-m", "wip"]);
    assert!(!refused.status.success() && stderr(&refused).contains("non conforme"), "{}", stderr(&refused));
    env.git_ok(&["switch", "-q", "fix/0001-tester-les-hooks"]);

    // --- binaire noté disparu et absent du PATH : message d'erreur explicite
    let moved = env.home.join("bin temporaire");
    fs::create_dir_all(&moved).unwrap();
    let copy = moved.join("coutcouticket.exe");
    fs::copy(BIN, &copy).unwrap();
    let out = env.cmd(copy.to_str().unwrap()).arg("init").output().unwrap();
    assert!(out.status.success(), "{}", stderr(&out));
    let noted = copy.to_str().unwrap().replace('\\', "/");
    assert!(fs::read_to_string(&pre_commit).unwrap().contains(&noted));
    let out = git_without_bin(&["commit", "-q", "--allow-empty", "-m", "binaire copié"]);
    assert!(out.status.success(), "commit refusé :\n{}", stderr(&out));
    fs::remove_file(&copy).unwrap();
    let refused = git_without_bin(&["commit", "-q", "--allow-empty", "-m", "introuvable"]);
    assert!(!refused.status.success(), "commit accepté sans binaire");
    assert!(stderr(&refused).contains("coutcouticket introuvable"), "{}", stderr(&refused));
}

/// Sortie de schtasks, en UTF-16 ou en page de code locale.
#[cfg(windows)]
fn schtasks_query(name: &str) -> (bool, String) {
    let out = Command::new("schtasks").args(["/Query", "/TN", name, "/XML"]).output().unwrap();
    let bytes: Vec<u8> = out.stdout.iter().copied().filter(|b| *b != 0).collect();
    (out.status.success(), String::from_utf8_lossy(&bytes).to_string())
}

/// Installation réelle de la tâche planifiée : modifie la session Windows courante,
/// donc lancée seulement si COUTCOUTICKET_TEST_WINDOWS_SERVICE est défini (CI).
/// La tâche ne reçoit pas les variables du test : configuration réelle (%APPDATA%).
#[cfg(windows)]
#[test]
fn daemon_tache_planifiee_windows() {
    if std::env::var_os("COUTCOUTICKET_TEST_WINDOWS_SERVICE").is_none() {
        eprintln!("ignoré : définir COUTCOUTICKET_TEST_WINDOWS_SERVICE=1 pour installer réellement la tâche planifiée");
        return;
    }
    const TASK: &str = "coutcouticket-daemon";
    let run = |args: &[&str]| Command::new(BIN).args(args).env_remove("COUTCOUTICKET_HOME").env_remove("COUTCOUTICKET_DAEMON_TASK").output().unwrap();
    let wait_status = || {
        let start = Instant::now();
        loop {
            let out = run(&["daemon", "status"]);
            if out.status.success() {
                return stdout(&out);
            }
            assert!(start.elapsed() < Duration::from_secs(20), "démon injoignable : {}", stderr(&out));
            std::thread::sleep(Duration::from_millis(250));
        }
    };

    // --- installation : tâche à l'ouverture de session de l'utilisateur, sans élévation
    let out = run(&["daemon", "install"]);
    assert!(out.status.success(), "{}\n{}", stdout(&out), stderr(&out));
    assert!(stdout(&out).contains(&format!("tâche planifiée « {TASK} »")), "{}", stdout(&out));
    let (exists, xml) = schtasks_query(TASK);
    assert!(exists, "tâche absente après install");
    // RunLevel LeastPrivilege est la valeur par défaut : absente de l'export.
    for needle in ["<LogonTrigger>", "InteractiveToken", "--headless", "daemon run --log-file", "PT0S", "<Priority>5</Priority>"] {
        assert!(xml.contains(needle), "{needle} absent :\n{xml}");
    }
    assert!(!xml.contains("HighestAvailable"), "tâche élevée :\n{xml}");
    assert!(wait_status().contains("ok"));
    let log_dir = PathBuf::from(std::env::var("LOCALAPPDATA").unwrap()).join("coutcouticket");
    let first_pid = fs::read_to_string(log_dir.join("daemon.pid")).unwrap();
    assert!(fs::read_to_string(log_dir.join("daemon.log")).unwrap().contains("à l'écoute"));

    // --- réinstallation idempotente : l'ancienne instance est remplacée
    let out = run(&["daemon", "install"]);
    assert!(out.status.success(), "{}\n{}", stdout(&out), stderr(&out));
    assert!(wait_status().contains("ok"));
    let start = Instant::now();
    while fs::read_to_string(log_dir.join("daemon.pid")).unwrap_or_default() == first_pid {
        assert!(start.elapsed() < Duration::from_secs(20), "démon non relancé par install");
        std::thread::sleep(Duration::from_millis(250));
    }

    // --- désinstallation : tâche retirée, démon arrêté, status explicite
    let out = run(&["daemon", "uninstall"]);
    assert!(out.status.success(), "{}\n{}", stdout(&out), stderr(&out));
    assert!(!schtasks_query(TASK).0, "tâche encore présente après uninstall");
    let out = run(&["daemon", "status"]);
    assert!(!out.status.success(), "démon encore joignable : {}", stdout(&out));
    assert!(stderr(&out).contains("tâche planifiée absente"), "{}", stderr(&out));
}
