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
    fs::rename(&tmp, path).with_context(|| format!("remplacement de {}", path.display()))?;
    Ok(())
}

/// Écrit seulement si le contenu change. Retourne true si le fichier a été écrit.
/// Évite le bruit git et les boucles avec le watcher du démon.
pub fn write_if_changed(path: &Path, content: &str) -> Result<bool> {
    if let Ok(existing) = fs::read_to_string(path) {
        if existing == content {
            return Ok(false);
        }
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
