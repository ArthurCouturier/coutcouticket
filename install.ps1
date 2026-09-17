# Installation et mise à jour de coutcouticket depuis les releases GitHub (Windows x64).
# Encodé en UTF-8 avec BOM : sans BOM, Windows PowerShell 5.1 lit le fichier en ANSI.
#
#   irm https://raw.githubusercontent.com/ArthurCouturier/coutcouticket/main/install.ps1 | iex
#   & ([scriptblock]::Create((irm .../install.ps1))) -Version 0.2.0 -Dir "$env:USERPROFILE\bin"
#
# Paramètres (ou variables d'environnement) :
#   -Version X.Y.Z   COUTCOUTICKET_VERSION      version à installer (défaut : dernière release)
#   -Dir DOSSIER     COUTCOUTICKET_INSTALL_DIR  dossier d'installation (défaut : voir plus bas)
#   -NoDaemon        COUTCOUTICKET_NO_DAEMON=1  ne pas relancer le démon
#   COUTCOUTICKET_BASE_URL  source des archives (défaut : releases GitHub ; file:// accepté)
#
# Dossier par défaut : celui du coutcouticket.exe déjà présent dans le PATH (remplacé sur
# place : les hooks git et la tâche planifiée gardent un chemin valide), sinon
# %LOCALAPPDATA%\Programs\coutcouticket. Aucun droit administrateur n'est nécessaire.
param(
    [string]$Version = $env:COUTCOUTICKET_VERSION,
    [string]$Dir = $env:COUTCOUTICKET_INSTALL_DIR,
    [switch]$NoDaemon = [bool]$env:COUTCOUTICKET_NO_DAEMON
)

$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'
[Net.ServicePointManager]::SecurityProtocol = [Net.ServicePointManager]::SecurityProtocol -bor [Net.SecurityProtocolType]::Tls12

$Repo = 'ArthurCouturier/coutcouticket'
$Task = 'coutcouticket-daemon'
$Target = 'x86_64-pc-windows-msvc'

# throw plutôt que exit : avec « irm | iex », exit fermerait le terminal de l'utilisateur.
function Fail([string]$Message) {
    throw "erreur : $Message"
}
# Commande native sans erreur terminale sur stderr (Windows PowerShell 5.1 avec Stop).
function Invoke-Native([string]$Exe, [string[]]$Arguments) {
    $ErrorActionPreference = 'Continue'
    $out = & $Exe @Arguments 2>&1 | ForEach-Object { "$_" }
    [pscustomobject]@{ Code = $LASTEXITCODE; Output = ($out -join "`n").Trim() }
}
function Warn([string]$Message) {
    Write-Host "attention : $Message" -ForegroundColor Yellow
}

# --- Plateforme -----------------------------------------------------------------
if ($env:OS -ne 'Windows_NT') {
    Fail "ce script est pour Windows. macOS : install.sh ; ailleurs : « cargo install --git https://github.com/$Repo »."
}
if (-not [Environment]::Is64BitOperatingSystem -or $env:PROCESSOR_ARCHITECTURE -eq 'ARM64') {
    Warn "seul Windows x64 est publié ; sous Windows ARM64, le binaire x64 s'exécute en émulation."
}

# --- Version --------------------------------------------------------------------
$Version = "$Version".TrimStart('v')
if (-not $Version) {
    # Redirection /releases/latest → /releases/tag/vX.Y.Z : pas d'API, pas de quota.
    try {
        $resp = Invoke-WebRequest -Uri "https://github.com/$Repo/releases/latest" -UseBasicParsing -Method Head
        $final = if ($resp.BaseResponse.ResponseUri) { $resp.BaseResponse.ResponseUri.AbsoluteUri } else { $resp.BaseResponse.RequestMessage.RequestUri.AbsoluteUri }
    } catch {
        Fail "impossible de joindre GitHub pour trouver la dernière version ($($_.Exception.Message)). Vérifier la connexion ou passer -Version X.Y.Z."
    }
    if ($final -match '/tag/v([^/]+)$') { $Version = $Matches[1] }
    else { Fail "aucune release publiée trouvée ($final). Passer -Version X.Y.Z ou installer avec cargo." }
}
$BaseUrl = if ($env:COUTCOUTICKET_BASE_URL) { $env:COUTCOUTICKET_BASE_URL } else { "https://github.com/$Repo/releases/download/v$Version" }

# --- Dossier d'installation -----------------------------------------------------
$existing = (Get-Command coutcouticket.exe -CommandType Application -ErrorAction SilentlyContinue | Select-Object -First 1).Source
if (-not $Dir) {
    $Dir = if ($existing) { Split-Path -Parent $existing } else { Join-Path $env:LOCALAPPDATA 'Programs\coutcouticket' }
}
$Dir = [IO.Path]::GetFullPath($Dir)
try { New-Item -ItemType Directory -Force -Path $Dir | Out-Null }
catch { Fail "impossible de créer $Dir. Choisir un autre dossier avec -Dir." }
$Dest = Join-Path $Dir 'coutcouticket.exe'

# --- Téléchargement et vérification --------------------------------------------
$Name = "coutcouticket-$Version-$Target"
$Tmp = Join-Path ([IO.Path]::GetTempPath()) ("coutcouticket-" + [Guid]::NewGuid())
New-Item -ItemType Directory -Path $Tmp | Out-Null
try {
    Write-Host "Téléchargement de coutcouticket $Version ($Target)…"
    # WebClient : accepte https:// comme file:// (tests sans release).
    $web = New-Object Net.WebClient
    $zip = Join-Path $Tmp "$Name.zip"
    try { $web.DownloadFile("$BaseUrl/$Name.zip", $zip) }
    catch { Fail "téléchargement impossible : $BaseUrl/$Name.zip. Vérifier que la version $Version existe (https://github.com/$Repo/releases)." }
    try { $web.DownloadFile("$BaseUrl/$Name.zip.sha256", "$zip.sha256") }
    catch { Fail "somme de contrôle introuvable : $BaseUrl/$Name.zip.sha256. Release incomplète : ne pas installer." }

    $expected = ((Get-Content -Raw "$zip.sha256").Trim() -split '\s+')[0].ToLowerInvariant()
    $actual = (Get-FileHash -Algorithm SHA256 $zip).Hash.ToLowerInvariant()
    if (-not $expected -or $expected -ne $actual) {
        Fail "somme SHA-256 incorrecte (attendu $expected, obtenu $actual). Archive corrompue ou altérée : relancer, et signaler le problème si ça persiste."
    }

    try { Expand-Archive -Path $zip -DestinationPath $Tmp -Force }
    catch { Fail "archive illisible : $Name.zip." }
    $New = Join-Path $Tmp "$Name\coutcouticket.exe"
    if (-not (Test-Path $New)) { Fail "binaire absent de l'archive $Name.zip." }
    # Marque « téléchargé depuis Internet » (SmartScreen) : retirée, la somme est vérifiée.
    Unblock-File -Path $New -ErrorAction SilentlyContinue
    if ((Invoke-Native $New @('--version')).Code -ne 0) { Fail "le binaire téléchargé ne s'exécute pas. Installer avec cargo en attendant." }

    # --- Installation -----------------------------------------------------------
    # Un .exe en cours d'exécution (démon) ne peut pas être remplacé mais peut être
    # renommé : l'ancien devient coutcouticket.exe.old, supprimé au prochain passage.
    $oldVersion = $null
    if (Test-Path $Dest) {
        $oldVersion = (Invoke-Native $Dest @('--version')).Output
        Remove-Item "$Dest.old" -Force -ErrorAction SilentlyContinue
        if (Test-Path "$Dest.old") { Fail "$Dest.old est encore utilisé. Arrêter le démon (« coutcouticket daemon uninstall ») puis relancer." }
        Move-Item -Force $Dest "$Dest.old"
    }
    try { Copy-Item -Force $New $Dest }
    catch {
        if (Test-Path "$Dest.old") { Move-Item -Force "$Dest.old" $Dest }
        Fail "impossible d'écrire $Dest. Choisir un dossier à soi avec -Dir."
    }
    $newVersion = (Invoke-Native $Dest @('--version')).Output
    if ($oldVersion) { Write-Host "Mis à jour : $oldVersion → $newVersion ($Dest)" }
    else { Write-Host "Installé : $newVersion ($Dest)" }
} finally {
    Remove-Item -Recurse -Force $Tmp -ErrorAction SilentlyContinue
}

# --- PATH utilisateur -----------------------------------------------------------
$userPath = [Environment]::GetEnvironmentVariable('Path', 'User')
$entries = @("$userPath" -split ';' | Where-Object { $_ })
if ($entries -notcontains $Dir) {
    [Environment]::SetEnvironmentVariable('Path', (($entries + $Dir) -join ';'), 'User')
    $env:Path = "$env:Path;$Dir"
    Write-Host "$Dir ajouté au PATH de l'utilisateur (nouveaux terminaux)."
}

# --- Démon ----------------------------------------------------------------------
if ((Invoke-Native 'schtasks' @('/Query', '/TN', $Task)).Code -eq 0) {
    if ($NoDaemon) {
        Write-Host "Démon non relancé (-NoDaemon). Pour charger la nouvelle version : « $Dest daemon install »."
    } else {
        # daemon install est idempotent : réécrit la tâche avec ce binaire, garde port et jeton, relance.
        Write-Host "Relance du démon…"
        $ok = $false
        foreach ($i in 1..3) {
            if ((Invoke-Native $Dest @('daemon', 'install')).Code -eq 0) { $ok = $true; break }
            Start-Sleep -Seconds 2
        }
        if (-not $ok) { Fail "échec de « $Dest daemon install ». Consulter $env:LOCALAPPDATA\coutcouticket\daemon.log puis relancer la commande." }
        $status = Invoke-Native $Dest @('daemon', 'status')
        if ($status.Code -eq 0) { Write-Host "Démon : $($status.Output)" }
        else { Warn "le démon ne répond pas encore. Vérifier plus tard avec « coutcouticket daemon status »." }
    }
} else {
    Write-Host "Démon non installé. Pour l'installer : « coutcouticket daemon install » puis « coutcouticket setup-claude --apply »."
}

# --- Vérifications de chemin ----------------------------------------------------
$first = (Get-Command coutcouticket.exe -CommandType Application -ErrorAction SilentlyContinue | Select-Object -First 1).Source
if ($first -and $first -ne $Dest) {
    Warn "« coutcouticket » désigne $first, qui masque $Dest. Supprimer l'ancien binaire ou réordonner le PATH."
}
if ($existing -and $existing -ne $Dest) {
    Warn "les hooks git des projets initialisés avec $existing l'appellent encore. Relancer « coutcouticket init » dans chaque projet (liste : « coutcouticket projects list »)."
}
