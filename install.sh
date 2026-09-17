#!/bin/sh
# Installation et mise à jour de coutcouticket depuis les releases GitHub (macOS).
#
#   curl -fsSL https://raw.githubusercontent.com/ArthurCouturier/coutcouticket/main/install.sh | sh
#   curl -fsSL .../install.sh | sh -s -- --version 0.2.0 --dir ~/.local/bin
#
# Options (ou variables d'environnement) :
#   --version X.Y.Z   COUTCOUTICKET_VERSION      version à installer (défaut : dernière release)
#   --dir DOSSIER     COUTCOUTICKET_INSTALL_DIR  dossier d'installation (défaut : voir plus bas)
#   --no-daemon       COUTCOUTICKET_NO_DAEMON=1  ne pas relancer le démon
#   COUTCOUTICKET_BASE_URL  source des archives (défaut : releases GitHub ; file:// accepté)
#
# Dossier par défaut : celui du coutcouticket déjà présent dans le PATH (remplacé sur
# place : les hooks git et le LaunchAgent gardent un chemin valide), sinon ~/.local/bin.
set -eu

REPO="ArthurCouturier/coutcouticket"
PLIST="$HOME/Library/LaunchAgents/app.coutcouticket.daemon.plist"

version="${COUTCOUTICKET_VERSION:-}"
dir="${COUTCOUTICKET_INSTALL_DIR:-}"
no_daemon="${COUTCOUTICKET_NO_DAEMON:-}"
base_url="${COUTCOUTICKET_BASE_URL:-}"

info() { printf '%s\n' "$*"; }
warn() { printf 'attention : %s\n' "$*" >&2; }
die() { printf 'erreur : %s\n' "$*" >&2; exit 1; }
usage() {
  cat <<'USAGE'
Usage : install.sh [--version X.Y.Z] [--dir DOSSIER] [--no-daemon]
  --version X.Y.Z  version à installer (défaut : dernière release)
  --dir DOSSIER    dossier d'installation (défaut : celui du coutcouticket du PATH, sinon ~/.local/bin)
  --no-daemon      ne pas relancer le démon
USAGE
}

while [ $# -gt 0 ]; do
  case "$1" in
    --version) [ $# -ge 2 ] || die "--version attend un numéro (ex. --version 0.2.0)"; version="$2"; shift 2 ;;
    --dir) [ $# -ge 2 ] || die "--dir attend un dossier (ex. --dir ~/.local/bin)"; dir="$2"; shift 2 ;;
    --no-daemon) no_daemon=1; shift ;;
    -h|--help) usage; exit 0 ;;
    *) die "option inconnue « $1 ». Options : --version X.Y.Z, --dir DOSSIER, --no-daemon" ;;
  esac
done

# --- Plateforme -----------------------------------------------------------------
[ "$(uname -s)" = "Darwin" ] || die "ce script est pour macOS. Windows : install.ps1 ; ailleurs : « cargo install --git https://github.com/$REPO »."
case "$(uname -m)" in
  arm64|aarch64) target="aarch64-apple-darwin" ;;
  x86_64) target="x86_64-apple-darwin" ;;
  *) die "architecture $(uname -m) non publiée. Installer avec « cargo install --git https://github.com/$REPO »." ;;
esac
command -v curl >/dev/null 2>&1 || die "curl introuvable : l'installer puis relancer."
command -v shasum >/dev/null 2>&1 || die "shasum introuvable : il est fourni par macOS (/usr/bin/shasum), vérifier le PATH."

# --- Version --------------------------------------------------------------------
version="${version#v}"
if [ -z "$version" ]; then
  # Redirection /releases/latest → /releases/tag/vX.Y.Z : pas d'API, pas de quota.
  latest="$(curl -fsSLI -o /dev/null -w '%{url_effective}' "https://github.com/$REPO/releases/latest")" \
    || die "impossible de joindre GitHub pour trouver la dernière version. Vérifier la connexion ou passer --version X.Y.Z."
  case "$latest" in
    */tag/v*) version="${latest##*/tag/v}" ;;
    *) die "aucune release publiée trouvée ($latest). Passer --version X.Y.Z ou installer avec cargo." ;;
  esac
fi
[ -n "$base_url" ] || base_url="https://github.com/$REPO/releases/download/v$version"

# --- Dossier d'installation -----------------------------------------------------
existing="$(command -v coutcouticket 2>/dev/null || true)"
case "$existing" in /*) ;; *) existing="" ;; esac
if [ -z "$dir" ]; then
  if [ -n "$existing" ]; then dir="$(dirname "$existing")"; else dir="$HOME/.local/bin"; fi
fi
case "$dir" in "~"/*) dir="$HOME/${dir#"~/"}" ;; esac
case "$dir" in /*) ;; *) dir="$(pwd)/$dir" ;; esac
mkdir -p "$dir" || die "impossible de créer $dir. Choisir un autre dossier avec --dir."
[ -w "$dir" ] || die "$dir n'est pas accessible en écriture. Choisir un dossier à soi avec --dir (ex. --dir ~/.local/bin), sans sudo."
dest="$dir/coutcouticket"

# --- Téléchargement et vérification --------------------------------------------
name="coutcouticket-$version-$target"
tmp="$(mktemp -d "${TMPDIR:-/tmp}/coutcouticket.XXXXXX")"
trap 'rm -rf "$tmp"' EXIT
trap 'exit 130' INT TERM

info "Téléchargement de coutcouticket $version ($target)…"
curl -fsSL -o "$tmp/$name.tar.gz" "$base_url/$name.tar.gz" \
  || die "téléchargement impossible : $base_url/$name.tar.gz. Vérifier que la version $version existe (https://github.com/$REPO/releases)."
curl -fsSL -o "$tmp/$name.tar.gz.sha256" "$base_url/$name.tar.gz.sha256" \
  || die "somme de contrôle introuvable : $base_url/$name.tar.gz.sha256. Release incomplète : ne pas installer."

expected="$(awk '{print $1; exit}' "$tmp/$name.tar.gz.sha256")"
actual="$(shasum -a 256 "$tmp/$name.tar.gz" | awk '{print $1}')"
[ -n "$expected" ] && [ "$expected" = "$actual" ] \
  || die "somme SHA-256 incorrecte (attendu $expected, obtenu $actual). Archive corrompue ou altérée : relancer, et signaler le problème si ça persiste."

tar -xzf "$tmp/$name.tar.gz" -C "$tmp" || die "archive illisible : $name.tar.gz."
new="$tmp/$name/coutcouticket"
[ -f "$new" ] || die "binaire absent de l'archive $name.tar.gz."
chmod 755 "$new"
# Quarantaine Gatekeeper : posée si l'archive a transité par un navigateur.
xattr -d com.apple.quarantine "$new" 2>/dev/null || true
"$new" --version >/dev/null 2>&1 || die "le binaire téléchargé ne s'exécute pas (architecture ou signature). Installer avec cargo en attendant."

# --- Installation atomique ------------------------------------------------------
# Copie dans le dossier cible puis renommage : nouvel inode, jamais d'écrasement du
# fichier en cours d'exécution (macOS tue un binaire signé modifié sur place).
old_version=""
[ -x "$dest" ] && old_version="$("$dest" --version 2>/dev/null || true)"
cp "$new" "$dest.new.$$" && mv -f "$dest.new.$$" "$dest" \
  || { rm -f "$dest.new.$$"; die "impossible d'écrire $dest."; }
if [ -n "$old_version" ]; then
  info "Mis à jour : $old_version → $("$dest" --version) ($dest)"
else
  info "Installé : $("$dest" --version) ($dest)"
fi

# --- Démon ----------------------------------------------------------------------
if [ -f "$PLIST" ]; then
  if [ -n "$no_daemon" ]; then
    info "Démon non relancé (--no-daemon). Pour charger la nouvelle version : « $dest daemon install »."
  else
    # daemon install est idempotent : réécrit le plist avec ce binaire, garde port et jeton, relance.
    info "Relance du démon…"
    # Nouvel essai : launchctl bootstrap peut échouer si le bootout précédent n'est pas terminé.
    n=0
    until "$dest" daemon install >/dev/null; do
      n=$((n + 1))
      [ "$n" -lt 3 ] || die "échec de « $dest daemon install ». Consulter ~/Library/Logs/coutcouticket/daemon.log puis relancer la commande."
      sleep 2
    done
    i=0
    until status="$("$dest" daemon status 2>/dev/null)"; do
      i=$((i + 1))
      if [ "$i" -ge 10 ]; then
        warn "le démon ne répond pas encore. Vérifier plus tard avec « coutcouticket daemon status »."
        break
      fi
      sleep 1
    done
    [ -n "${status:-}" ] && info "Démon : $status"
  fi
else
  info "Démon non installé. Pour l'installer : « $dest daemon install » puis « $dest setup-claude --apply »."
fi

# --- Vérifications de chemin ----------------------------------------------------
case ":$PATH:" in
  *":$dir:"*) ;;
  *) warn "$dir n'est pas dans le PATH. Ajouter à ~/.zshrc : export PATH=\"$dir:\$PATH\"" ;;
esac
first="$(command -v coutcouticket 2>/dev/null || true)"
if [ -n "$first" ] && [ "$first" != "$dest" ]; then
  warn "« coutcouticket » désigne $first, qui masque $dest. Supprimer l'ancien binaire ou réordonner le PATH."
fi
if [ -n "$existing" ] && [ "$existing" != "$dest" ]; then
  warn "les hooks git des projets initialisés avec $existing l'appellent encore. Relancer « coutcouticket init » dans chaque projet (liste : « coutcouticket projects list »)."
fi
