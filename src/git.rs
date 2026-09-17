//! Accès git en appelant le binaire `git` : même comportement que dans le
//! terminal (config, hooksPath, worktrees), sans dépendance lourde.
//!
//! « Pas un dépôt » et « git en échec » sont distincts : un git cassé (binaire
//! absent, licence Xcode non acceptée…) ne doit jamais passer pour un dossier
//! hors dépôt. Binaire surchargeable par `COUTCOUTICKET_GIT_BIN` (tests).

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use anyhow::{Result, anyhow, bail};

fn git_bin() -> OsString {
    std::env::var_os("COUTCOUTICKET_GIT_BIN").unwrap_or_else(|| "git".into())
}

/// Lance git sans interpréter le code de sortie.
fn run(root: &Path, args: &[&str], envs: &[(&str, &str)]) -> Result<Output> {
    let bin = git_bin();
    let mut cmd = Command::new(&bin);
    cmd.arg("-C").arg(root).args(args).envs(envs.iter().copied());
    match cmd.output() {
        Ok(o) => Ok(o),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => bail!(
            "binaire git « {} » introuvable. Installer git (macOS : « xcode-select --install ») ou corriger le PATH, puis relancer.",
            bin.to_string_lossy()
        ),
        Err(e) => bail!("impossible de lancer git « {} » : {e}", bin.to_string_lossy()),
    }
}

/// Erreur d'un appel git : commande, code, stderr et correction connue.
fn failure(args: &[&str], out: &Output) -> anyhow::Error {
    let stderr = String::from_utf8_lossy(&out.stderr).trim().to_string();
    let code = out.status.code().map_or_else(|| "tué par un signal".to_string(), |c| format!("code {c}"));
    let mut msg = format!(
        "« git {} » a échoué ({code}) : {}",
        args.join(" "),
        if stderr.is_empty() { "aucune sortie d'erreur" } else { &stderr }
    );
    if let Some(hint) = known_fix(&stderr) {
        msg.push('\n');
        msg.push_str(hint);
    }
    anyhow!(msg)
}

/// Correction connue pour les pannes de git fréquentes sous macOS.
fn known_fix(stderr: &str) -> Option<&'static str> {
    let s = stderr.to_lowercase();
    if s.contains("xcodebuild -license") || s.contains("xcode license") {
        Some(
            "Correction : licence Xcode non acceptée. Lancer « sudo xcodebuild -license » (ou utiliser les Command Line Tools : « sudo xcode-select -s /Library/Developer/CommandLineTools »), puis relancer.",
        )
    } else if s.contains("invalid active developer path") || s.contains("no developer tools were found") {
        Some("Correction : outils de développement Apple absents. Lancer « xcode-select --install », puis relancer.")
    } else {
        None
    }
}

fn git(root: &Path, args: &[&str]) -> Result<String> {
    let out = run(root, args, &[])?;
    if !out.status.success() {
        return Err(failure(args, &out));
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim_end().to_string())
}

/// Vrai si `root` est dans un arbre de travail git, faux s'il est hors dépôt.
/// Erreur si git lui-même échoue.
pub fn is_repo(root: &Path) -> Result<bool> {
    let args = ["rev-parse", "--is-inside-work-tree"];
    // Messages en anglais : le cas « pas un dépôt » se reconnaît au texte.
    let out = run(root, &args, &[("LC_ALL", "C")])?;
    if out.status.success() {
        return Ok(String::from_utf8_lossy(&out.stdout).trim() == "true");
    }
    if is_not_a_repo(&out) {
        return Ok(false);
    }
    Err(failure(&args, &out))
}

fn is_not_a_repo(out: &Output) -> bool {
    out.status.code() == Some(128) && String::from_utf8_lossy(&out.stderr).contains("not a git repository")
}

/// Branche courante ; None si HEAD détaché (rebase, bisect…).
pub fn current_branch(root: &Path) -> Result<Option<String>> {
    let args = ["symbolic-ref", "--quiet", "--short", "HEAD"];
    let out = run(root, &args, &[])?;
    match out.status.code() {
        Some(0) => Ok(Some(String::from_utf8_lossy(&out.stdout).trim().to_string())),
        // --quiet : code 1 sans message quand HEAD n'est pas une référence symbolique.
        Some(1) => Ok(None),
        _ => Err(failure(&args, &out)),
    }
}

/// Branche courante, None hors dépôt ou HEAD détaché ; erreur si git échoue.
pub fn current_branch_if_repo(root: &Path) -> Result<Option<String>> {
    if is_repo(root)? { current_branch(root) } else { Ok(None) }
}

pub fn branch_exists(root: &Path, branch: &str) -> Result<bool> {
    let refname = format!("refs/heads/{branch}");
    let args = ["show-ref", "--verify", "--quiet", &refname];
    let out = run(root, &args, &[])?;
    match out.status.code() {
        Some(0) => Ok(true),
        Some(1) => Ok(false),
        _ => Err(failure(&args, &out)),
    }
}

/// Bascule sur la branche, en la créant depuis HEAD si besoin.
/// Retourne true si la branche a été créée.
pub fn switch_or_create(root: &Path, branch: &str) -> Result<bool> {
    if branch_exists(root, branch)? {
        git(root, &["switch", branch])?;
        Ok(false)
    } else {
        git(root, &["switch", "-c", branch])?;
        Ok(true)
    }
}

/// Dossier des hooks effectif (respecte core.hooksPath).
pub fn hooks_dir(root: &Path) -> Result<PathBuf> {
    let p = git(root, &["rev-parse", "--git-path", "hooks"])?;
    let path = PathBuf::from(p);
    Ok(if path.is_absolute() { path } else { root.join(path) })
}

/// Vrai si au moins un fichier sous `rel` (relatif à `root`) est suivi par git.
pub fn has_tracked_files(root: &Path, rel: &str) -> Result<bool> {
    Ok(!git(root, &["ls-files", "--", rel])?.is_empty())
}

/// Vrai si git ignore ce chemin (.gitignore, exclude, config globale).
pub fn is_ignored(root: &Path, path: &Path) -> Result<bool> {
    let rel = path.strip_prefix(root).unwrap_or(path);
    let rel = rel.to_string_lossy();
    let args = ["check-ignore", "--quiet", "--", &rel];
    let out = run(root, &args, &[])?;
    match out.status.code() {
        Some(0) => Ok(true),
        Some(1) => Ok(false),
        _ => Err(failure(&args, &out)),
    }
}

pub fn add(root: &Path, path: &Path) -> Result<()> {
    let rel = path.strip_prefix(root).unwrap_or(path);
    git(root, &["add", "--", &rel.to_string_lossy()])?;
    Ok(())
}

/// Ajoute le trailer `Ticket: <id>` s'il est absent (délégué à git interpret-trailers).
pub fn add_trailer(root: &Path, msg_file: &Path, value: &str) -> Result<()> {
    git(
        root,
        &[
            "interpret-trailers",
            "--in-place",
            "--if-exists",
            "doNothing",
            "--trailer",
            &format!("Ticket: {value}"),
            &msg_file.to_string_lossy(),
        ],
    )?;
    Ok(())
}

/// Fichiers touchés par les commits portant le trailer du ticket, toutes branches.
pub fn files_for_ticket(root: &Path, id: &str) -> Result<Vec<String>> {
    let out = git(
        root,
        &[
            "log",
            "--all",
            "--fixed-strings",
            &format!("--grep=Ticket: {id}"),
            "--name-only",
            "--pretty=format:",
        ],
    )?;
    let mut files: Vec<String> = out.lines().map(str::trim).filter(|l| !l.is_empty()).map(String::from).collect();
    files.sort();
    files.dedup();
    Ok(files)
}

/// Fichiers modifiés non commités (pour le ticket de la branche courante).
pub fn uncommitted_files(root: &Path) -> Result<Vec<String>> {
    let out = git(root, &["status", "--porcelain", "--untracked-files=all"])?;
    let mut files: Vec<String> = out
        .lines()
        .filter_map(|l| l.get(3..))
        .map(|f| f.rsplit(" -> ").next().unwrap_or(f).to_string())
        .collect();
    files.sort();
    Ok(files)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::process::ExitStatusExt;
    use std::process::ExitStatus;

    const XCODE: &str = "You have not agreed to the Xcode license agreements. Please run 'sudo xcodebuild -license' from within a Terminal window to review and agree to the Xcode and Apple SDKs license.";

    fn output(code: i32, stderr: &str) -> Output {
        Output { status: ExitStatus::from_raw(code << 8), stdout: vec![], stderr: stderr.as_bytes().to_vec() }
    }

    #[test]
    fn echec_avec_commande_code_et_stderr() {
        let msg = failure(&["log", "--all"], &output(128, "fatal: dubious ownership\n")).to_string();
        assert!(msg.contains("« git log --all » a échoué (code 128) : fatal: dubious ownership"), "{msg}");
        assert!(!msg.contains("Correction"), "{msg}");
        let msg = failure(&["status"], &output(3, "")).to_string();
        assert!(msg.contains("(code 3) : aucune sortie d'erreur"), "{msg}");
    }

    #[test]
    fn licence_xcode_reconnue() {
        let msg = failure(&["rev-parse", "--is-inside-work-tree"], &output(69, XCODE)).to_string();
        assert!(msg.contains("(code 69)") && msg.contains("You have not agreed"), "{msg}");
        assert!(msg.contains("sudo xcodebuild -license") && msg.contains("sudo xcode-select -s /Library/Developer/CommandLineTools"), "{msg}");
        let msg = failure(&["status"], &output(1, "xcrun: error: invalid active developer path (/Library/Developer/CommandLineTools)")).to_string();
        assert!(msg.contains("xcode-select --install"), "{msg}");
    }

    #[test]
    fn pas_un_depot_distinct_d_un_echec() {
        assert!(is_not_a_repo(&output(128, "fatal: not a git repository (or any of the parent directories): .git")));
        assert!(!is_not_a_repo(&output(69, XCODE)));
        assert!(!is_not_a_repo(&output(128, "fatal: detected dubious ownership in repository")));
        let tmp = tempfile::tempdir().unwrap();
        assert!(!is_repo(tmp.path()).unwrap());
        assert_eq!(current_branch_if_repo(tmp.path()).unwrap(), None);
    }
}
