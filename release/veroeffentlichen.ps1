# ============================================================
# Antrag 3000 – eine neue Version in EINEM Befehl veroeffentlichen.
#
# Macht nacheinander:
#   1. alte Setup-Dateien aufraeumen (sonst landet evtl. die falsche Version
#      im Manifest – der 0.2.0/0.3.0-Stolperstein),
#   2. signiert bauen (npm run tauri build),
#   3. das Update-Manifest latest.json erzeugen,
#   4. Installer + latest.json per SSH/scp auf die NAS in den updates-Ordner
#      hochladen (ueber Tailscale),
#   5. am Server gegenpruefen, dass version/url/Signatur zusammenpassen.
#
# Aufruf (im Projektordner):
#   .\release\veroeffentlichen.ps1 "Was ist neu in dieser Version"
#
# Voraussetzungen:
#   - Signatur-Schluessel unter  $HOME\.tauri\antrag3000.key
#   - SSH-Zugang zur NAS (Tailscale). Ohne SSH-Schluessel fragt scp/ssh nach
#     dem NAS-Passwort – einmal pro Verbindung. (Tipp: einmalig einen
#     SSH-Schluessel einrichten, dann ist der Upload passwortfrei.)
# ============================================================

param(
  [Parameter(Mandatory = $true, Position = 0)]
  [string]$Notes,

  [string]$SshUser     = "admin",
  [string]$NasHost     = "100.75.66.27",
  [string]$UpdatesPfad = "/volume1/docker/antrag3000/updates",
  [string]$KeyDatei    = "$HOME\.tauri\antrag3000.key",
  # Endpoint nur zur Schluss-Pruefung (muss zu tauri.conf.json passen).
  [string]$PruefUrl    = "http://100.75.66.27:8445/updates/latest.json"
)

$ErrorActionPreference = "Stop"
# Ins Projekt-Stammverzeichnis wechseln (eine Ebene ueber diesem Skript).
$wurzel = Split-Path -Parent $PSScriptRoot
Set-Location $wurzel

function Schritt($text) { Write-Host "`n=== $text ===" -ForegroundColor Cyan }

# --- Version aus tauri.conf.json lesen ----------------------------------
$conf    = Get-Content "src-tauri\tauri.conf.json" -Raw | ConvertFrom-Json
$version = $conf.version
Write-Host "Version laut tauri.conf.json: $version" -ForegroundColor Green

if (-not (Test-Path $KeyDatei)) {
  throw "Signatur-Schluessel nicht gefunden: $KeyDatei"
}

# --- Schluessel-Passwort sicher abfragen (du tippst es selbst) -----------
$sec  = Read-Host "Passwort des Signatur-Schluessels" -AsSecureString
$bstr = [Runtime.InteropServices.Marshal]::SecureStringToBSTR($sec)
try {
  $env:TAURI_SIGNING_PRIVATE_KEY          = Get-Content $KeyDatei -Raw
  $env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD = [Runtime.InteropServices.Marshal]::PtrToStringAuto($bstr)
} finally {
  [Runtime.InteropServices.Marshal]::ZeroFreeBSTR($bstr)
}

# --- 1. Alte Setup-Dateien aufraeumen -----------------------------------
Schritt "1/5  Alte Setup-Dateien aufraeumen"
$nsis = "src-tauri\target\release\bundle\nsis"
if (Test-Path $nsis) {
  Get-ChildItem $nsis -Filter "*-setup.exe*" -ErrorAction SilentlyContinue | ForEach-Object {
    Write-Host "  entferne $($_.Name)"
    Remove-Item $_.FullName -Force
  }
}

# --- 2. Signiert bauen ---------------------------------------------------
Schritt "2/5  Signiert bauen (npm run tauri build)"
npm run tauri build
if ($LASTEXITCODE -ne 0) { throw "Build fehlgeschlagen." }

# --- 3. Manifest erzeugen ------------------------------------------------
Schritt "3/5  Manifest latest.json erzeugen"
node release/latest-json-bauen.mjs $Notes
if ($LASTEXITCODE -ne 0) { throw "Manifest-Erzeugung fehlgeschlagen." }

# Den frisch gebauten Installer zur aktuellen Version finden.
$setup = Get-ChildItem $nsis -Filter "*$version*-setup.exe" | Select-Object -First 1
if (-not $setup) { throw "Setup-Datei zur Version $version nicht gefunden in $nsis." }
$latest = "server\updates\latest.json"
if (-not (Test-Path $latest)) { throw "latest.json nicht gefunden: $latest" }

# SHA-256-Pruefsumme erzeugen: damit Pilot-Nutzer:innen die Echtheit der
# .exe auch OHNE Code-Signing-Zertifikat selbst pruefen koennen
# (Get-FileHash <datei> -Algorithm SHA256 und vergleichen).
$hash     = (Get-FileHash $setup.FullName -Algorithm SHA256).Hash.ToLower()
$sumDatei = Join-Path $setup.DirectoryName ($setup.Name + ".sha256")
"$hash  $($setup.Name)" | Set-Content -Path $sumDatei -Encoding ascii
Write-Host "SHA-256: $hash" -ForegroundColor Green

# --- 4. Hochladen (scp ueber Tailscale) ---------------------------------
Schritt "4/5  Hochladen auf die NAS ($SshUser@$NasHost)"
Write-Host "  -> $($setup.Name)"
Write-Host "  -> $($setup.Name).sha256"
Write-Host "  -> latest.json"
# scp ueberschreibt vorhandene Dateien. Beide Quellen in EINEM Aufruf, damit
# nur EINE Verbindung (= ein Passwort) noetig ist.
# -O erzwingt das klassische SCP-Protokoll: Synology-SSH hat das von neueren
# scp-Versionen genutzte SFTP-Subsystem oft NICHT aktiviert ("subsystem
# request failed"). Mit -O laeuft der Upload trotzdem.
& scp -O $setup.FullName $sumDatei $latest "${SshUser}@${NasHost}:${UpdatesPfad}/"
if ($LASTEXITCODE -ne 0) {
  Write-Host "`nUpload fehlgeschlagen." -ForegroundColor Red
  Write-Host "Falls 'Permission denied': der SSH-Benutzer darf nicht direkt in" -ForegroundColor Yellow
  Write-Host "$UpdatesPfad schreiben. Dann einmalig den Ordner schreibbar machen" -ForegroundColor Yellow
  Write-Host "oder die Dateien per File Station ablegen." -ForegroundColor Yellow
  throw "scp-Upload fehlgeschlagen."
}

# --- 5. Am Server gegenpruefen ------------------------------------------
Schritt "5/5  Veroeffentlichung pruefen"
try {
  $m = Invoke-RestMethod -Uri $PruefUrl -TimeoutSec 15
  $url = $m.platforms.'windows-x86_64'.url
  Write-Host "  Server meldet Version: $($m.version)"
  Write-Host "  Installer-URL:         $url"
  if ($m.version -eq $version -and $url -match [regex]::Escape($version)) {
    Write-Host "`n  OK – Version, URL und Manifest passen zusammen." -ForegroundColor Green
  } else {
    Write-Host "`n  ACHTUNG: version/url passen nicht zu $version – bitte pruefen!" -ForegroundColor Red
  }
} catch {
  Write-Host "  (Konnte $PruefUrl nicht abrufen – Server/Tailscale pruefen.)" -ForegroundColor Yellow
}

# Passwort-Variable wieder entfernen.
Remove-Item Env:\TAURI_SIGNING_PRIVATE_KEY_PASSWORD -ErrorAction SilentlyContinue
Remove-Item Env:\TAURI_SIGNING_PRIVATE_KEY -ErrorAction SilentlyContinue

Write-Host "`nFertig: $version ist veroeffentlicht. Geraete mit aelterer Version" -ForegroundColor Green
Write-Host "sehen das Update beim naechsten Entsperren (oder per Knopf)." -ForegroundColor Green
