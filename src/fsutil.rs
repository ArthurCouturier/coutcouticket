//! Écritures sûres et verrou inter-processus (démon, CLI et hooks git peuvent
//! écrire en même temps).

use std::fs::{self, File, OpenOptions};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

/// Écrit via un fichier temporaire puis renomme : jamais de fichier à moitié écrit.
pub fn write_atomic(path: &Path, content: &str) -> Result<()> {
    let dir = path.parent().context("chemin sans parent")?;
    fs::create_dir_all(dir)?;
    let tmp = dir.join(format!(
        ".{}.tmp-{}",
        path.file_name().and_then(|n| n.to_str()).unwrap_or("file"),
        std::process::id()
    ));
    fs::write(&tmp, content).with_context(|| format!("écriture de {}", tmp.display()))?;
    rename(&tmp, path).with_context(|| format!("remplacement de {}", path.display()))?;
    Ok(())
}

/// Renommage avec remplacement. Sous Windows, un fichier ouvert ailleurs sans partage
/// en suppression (antivirus, indexation, éditeur) fait échouer le remplacement :
/// quelques essais rapprochés avant d'abandonner.
fn rename(from: &Path, to: &Path) -> std::io::Result<()> {
    #[cfg(windows)]
    {
        for _ in 0..10 {
            match fs::rename(from, to) {
                Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => {
                    std::thread::sleep(std::time::Duration::from_millis(20))
                }
                other => return other,
            }
        }
    }
    fs::rename(from, to)
}

/// Chemin absolu canonique, sans le préfixe `\\?\` que Windows ajoute
/// (`C:\…` au lieu de `\\?\C:\…`) : git, les hooks et l'utilisateur ne le comprennent pas.
pub fn canonicalize(path: &Path) -> std::io::Result<PathBuf> {
    Ok(simplify_verbatim(path.canonicalize()?))
}

/// Retire le préfixe verbatim `\\?\` quand le chemin a une forme classique équivalente
/// (lecteur `X:\…` ou partage `\\serveur\…`). Sans effet ailleurs que sous Windows.
fn simplify_verbatim(path: PathBuf) -> PathBuf {
    let Some(s) = path.to_str() else { return path };
    if let Some(rest) = s.strip_prefix(r"\\?\UNC\") {
        return PathBuf::from(format!(r"\\{rest}"));
    }
    match s.strip_prefix(r"\\?\") {
        Some(rest) if rest.as_bytes().first().is_some_and(u8::is_ascii_alphabetic) && rest.get(1..3) == Some(r":\") => {
            PathBuf::from(rest)
        }
        _ => path,
    }
}

/// Écrit seulement si le contenu change. Retourne true si le fichier a été écrit.
/// Évite le bruit git et les boucles avec le watcher du démon.
pub fn write_if_changed(path: &Path, content: &str) -> Result<bool> {
    if let Ok(existing) = fs::read_to_string(path)
        && existing == content
    {
        return Ok(false);
    }
    write_atomic(path, content)?;
    Ok(true)
}

/// Crée le fichier seulement s'il n'existe pas. Retourne true s'il a été créé.
pub fn create_if_missing(path: &Path, content: &str) -> Result<bool> {
    if path.exists() {
        return Ok(false);
    }
    write_atomic(path, content)?;
    Ok(true)
}

/// Ajoute du texte en fin de fichier (le fichier doit exister).
pub fn append(path: &Path, content: &str) -> Result<()> {
    use std::io::Write;
    let mut f = OpenOptions::new()
        .append(true)
        .open(path)
        .with_context(|| format!("ouverture de {}", path.display()))?;
    f.write_all(content.as_bytes())?;
    Ok(())
}

/// Hachage FNV-1a stable (indépendant de la version de Rust), pour nommer le verrou.
fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in data {
        h ^= *b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// Verrou exclusif par projet, stocké hors du repo (rien à ignorer dans git).
pub struct ProjectLock {
    _file: File,
}

impl ProjectLock {
    pub fn acquire(root: &Path) -> Result<Self> {
        let key = fnv1a(root.as_os_str().as_encoded_bytes());
        let path: PathBuf = std::env::temp_dir().join(format!("coutcouticket-{key:016x}.lock"));
        let file = OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .open(&path)
            .with_context(|| format!("ouverture du verrou {}", path.display()))?;
        file.lock().with_context(|| format!("prise du verrou {}", path.display()))?;
        Ok(ProjectLock { _file: file })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prefixe_verbatim_retire() {
        let s = |p: &str| simplify_verbatim(PathBuf::from(p)).to_string_lossy().into_owned();
        assert_eq!(s(r"\\?\C:\Users\a\projet"), r"C:\Users\a\projet");
        assert_eq!(s(r"\\?\UNC\serveur\partage\x"), r"\\serveur\partage\x");
        assert_eq!(s(r"\\?\Volume{abc}\x"), r"\\?\Volume{abc}\x");
        assert_eq!(s("/Users/a/projet"), "/Users/a/projet");
    }
}
