//! Configuration : `.coutcouticket.toml` (par projet), registre des projets et
//! configuration du démon (globales, dans `~/.config/coutcouticket/`).

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow, bail};
use serde::{Deserialize, Serialize};

pub const CONFIG_FILE: &str = ".coutcouticket.toml";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields, default)]
pub struct Config {
    /// Dossier racine des notes, relatif à la racine du projet.
    pub notes_dir: String,
    /// Types autorisés, utilisés comme préfixe de branche.
    pub branch_types: Vec<String>,
    /// Branches dispensées de la convention de ticket.
    pub exempt_branches: Vec<String>,
    /// Nombre de chiffres des identifiants (4 → 0001..9999).
    pub id_width: usize,
    /// Longueur maximale du slug.
    pub slug_max_len: usize,
    /// Valeurs autorisées pour le champ `projects` (vide = libre).
    pub projects: Vec<String>,
    /// Type par défaut à la création d'un ticket.
    pub default_type: String,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            notes_dir: "0-notes".into(),
            branch_types: ["feat", "fix", "refacto", "design", "ci"].map(String::from).to_vec(),
            exempt_branches: ["main", "master", "develop", "dev", "stag", "staging"]
                .map(String::from)
                .to_vec(),
            id_width: 4,
            slug_max_len: 50,
            projects: vec![],
            default_type: "feat".into(),
        }
    }
}

impl Config {
    pub fn load(root: &Path) -> Result<Self> {
        let path = root.join(CONFIG_FILE);
        let text = fs::read_to_string(&path).with_context(|| format!("lecture de {}", path.display()))?;
        let cfg: Config = toml::from_str(&text).with_context(|| format!("{} invalide", path.display()))?;
        cfg.check()?;
        Ok(cfg)
    }

    pub fn check(&self) -> Result<()> {
        if self.notes_dir.trim().is_empty() || self.notes_dir.contains("..") || Path::new(&self.notes_dir).is_absolute() {
            bail!("notes_dir doit être un chemin relatif simple (ex. « 0-notes »)");
        }
        if self.branch_types.is_empty() {
            bail!("branch_types ne peut pas être vide");
        }
        for t in &self.branch_types {
            if t.is_empty() || !t.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit()) {
                bail!("type de branche invalide « {t} » (minuscules et chiffres uniquement)");
            }
        }
        if !self.branch_types.contains(&self.default_type) {
            bail!("default_type « {} » absent de branch_types", self.default_type);
        }
        if !(1..=9).contains(&self.id_width) {
            bail!("id_width doit être entre 1 et 9");
        }
        if self.slug_max_len < 8 {
            bail!("slug_max_len doit être au moins 8");
        }
        Ok(())
    }

    pub fn max_id(&self) -> u32 {
        10u32.pow(self.id_width as u32) - 1
    }

    /// Contenu écrit par `init` : la config par défaut, commentée.
    pub fn default_file_content() -> String {
        let d = Config::default();
        let list = |v: &[String]| v.iter().map(|s| format!("\"{s}\"")).collect::<Vec<_>>().join(", ");
        format!(
            "# Configuration coutcouticket de ce projet.\n\
             # Toute clé absente prend sa valeur par défaut.\n\n\
             # Dossier des notes (tickets, doc, vue globale).\n\
             notes_dir = \"{}\"\n\n\
             # Types de ticket = préfixes de branche. Branche : <type>/<id>-<slug>\n\
             branch_types = [{}]\n\
             default_type = \"{}\"\n\n\
             # Branches dispensées de la convention.\n\
             exempt_branches = [{}]\n\n\
             # Largeur des identifiants (4 → 0001 à 9999).\n\
             id_width = {}\n\
             slug_max_len = {}\n\n\
             # Valeurs autorisées pour « projects » dans les tickets. Vide = libre.\n\
             projects = []\n",
            d.notes_dir,
            list(&d.branch_types),
            d.default_type,
            list(&d.exempt_branches),
            d.id_width,
            d.slug_max_len,
        )
    }
}

/// Remonte l'arborescence depuis `start` jusqu'à trouver `.coutcouticket.toml`.
pub fn find_root(start: &Path) -> Result<PathBuf> {
    let start = start
        .canonicalize()
        .with_context(|| format!("chemin introuvable : {}", start.display()))?;
    let mut cur: Option<&Path> = Some(&start);
    while let Some(dir) = cur {
        if dir.join(CONFIG_FILE).is_file() {
            return Ok(dir.to_path_buf());
        }
        cur = dir.parent();
    }
    Err(anyhow!(
        "aucun projet coutcouticket trouvé depuis {} (lancer « coutcouticket init » à la racine du projet)",
        start.display()
    ))
}

// ---------------------------------------------------------------------------
// Configuration globale
// ---------------------------------------------------------------------------

pub fn global_dir() -> Result<PathBuf> {
    if let Ok(dir) = std::env::var("COUTCOUTICKET_HOME") {
        return Ok(PathBuf::from(dir));
    }
    if let Ok(xdg) = std::env::var("XDG_CONFIG_HOME")
        && !xdg.is_empty()
    {
        return Ok(PathBuf::from(xdg).join("coutcouticket"));
    }
    let home = std::env::var("HOME").map_err(|_| anyhow!("variable HOME absente"))?;
    Ok(PathBuf::from(home).join(".config").join("coutcouticket"))
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct Registry {
    pub projects: Vec<PathBuf>,
}

impl Registry {
    pub fn path() -> Result<PathBuf> {
        Ok(global_dir()?.join("projects.toml"))
    }

    pub fn load() -> Result<Self> {
        let path = Self::path()?;
        if !path.exists() {
            return Ok(Registry::default());
        }
        let text = fs::read_to_string(&path)?;
        toml::from_str(&text).with_context(|| format!("{} invalide", path.display()))
    }

    pub fn save(&self) -> Result<()> {
        let path = Self::path()?;
        fs::create_dir_all(path.parent().unwrap())?;
        let text = format!(
            "# Projets suivis par coutcouticket (géré par « coutcouticket projects »).\n{}",
            toml::to_string(self)?
        );
        crate::fsutil::write_atomic(&path, &text)
    }

    /// Ajoute un projet ; retourne false s'il était déjà présent.
    pub fn add(&mut self, root: &Path) -> Result<bool> {
        let root = root.canonicalize()?;
        if self.projects.contains(&root) {
            return Ok(false);
        }
        self.projects.push(root);
        self.projects.sort();
        Ok(true)
    }

    pub fn remove(&mut self, root: &Path) -> bool {
        let canon = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());
        let before = self.projects.len();
        self.projects.retain(|p| p != &canon && p != root);
        before != self.projects.len()
    }

    pub fn contains(&self, root: &Path) -> bool {
        let canon = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());
        self.projects.contains(&canon)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DaemonConfig {
    pub port: u16,
    pub token: String,
}

impl DaemonConfig {
    pub fn path() -> Result<PathBuf> {
        Ok(global_dir()?.join("daemon.toml"))
    }

    pub fn load() -> Result<Option<Self>> {
        let path = Self::path()?;
        if !path.exists() {
            return Ok(None);
        }
        let text = fs::read_to_string(&path)?;
        Ok(Some(toml::from_str(&text).with_context(|| format!("{} invalide", path.display()))?))
    }

    /// Charge la config du démon, ou la crée avec un jeton aléatoire.
    pub fn load_or_create() -> Result<Self> {
        if let Some(cfg) = Self::load()? {
            return Ok(cfg);
        }
        let cfg = DaemonConfig { port: 47813, token: random_token()? };
        let path = Self::path()?;
        fs::create_dir_all(path.parent().unwrap())?;
        let text = format!(
            "# Démon coutcouticket. Le jeton protège le serveur MCP local.\n{}",
            toml::to_string(&cfg)?
        );
        crate::fsutil::write_atomic(&path, &text)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&path, fs::Permissions::from_mode(0o600))?;
        }
        Ok(cfg)
    }
}

fn random_token() -> Result<String> {
    use std::io::Read;
    let mut buf = [0u8; 24];
    fs::File::open("/dev/urandom")
        .and_then(|mut f| f.read_exact(&mut buf))
        .context("lecture de /dev/urandom")?;
    Ok(buf.iter().map(|b| format!("{b:02x}")).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_file_parses_to_default() {
        let parsed: Config = toml::from_str(&Config::default_file_content()).unwrap();
        assert_eq!(parsed, Config::default());
    }

    #[test]
    fn rejects_unknown_key() {
        assert!(toml::from_str::<Config>("notes = \"x\"").is_err());
    }
}
