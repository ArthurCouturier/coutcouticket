//! Accès git en appelant le binaire `git` : même comportement que dans le
//! terminal (config, hooksPath, worktrees), sans dépendance lourde.

use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, bail};

fn git(root: &Path, args: &[&str]) -> Result<String> {
    let out = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .output()
        .context("impossible de lancer git (est-il installé ?)")?;
    if !out.status.success() {
        bail!(
            "git {} a échoué : {}",
            args.join(" "),
            String::from_utf8_lossy(&out.stderr).trim()
        );
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim_end().to_string())
}

pub fn is_repo(root: &Path) -> bool {
    git(root, &["rev-parse", "--is-inside-work-tree"]).map(|s| s == "true").unwrap_or(false)
}

/// Branche courante ; None si HEAD détaché (rebase, bisect…).
pub fn current_branch(root: &Path) -> Result<Option<String>> {
    let out = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["symbolic-ref", "--quiet", "--short", "HEAD"])
        .output()
        .context("impossible de lancer git")?;
    if out.status.success() {
        Ok(Some(String::from_utf8_lossy(&out.stdout).trim().to_string()))
    } else {
        Ok(None)
    }
}

pub fn branch_exists(root: &Path, branch: &str) -> bool {
    git(root, &["show-ref", "--verify", "--quiet", &format!("refs/heads/{branch}")]).is_ok()
}

/// Bascule sur la branche, en la créant depuis HEAD si besoin.
/// Retourne true si la branche a été créée.
pub fn switch_or_create(root: &Path, branch: &str) -> Result<bool> {
    if branch_exists(root, branch) {
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
