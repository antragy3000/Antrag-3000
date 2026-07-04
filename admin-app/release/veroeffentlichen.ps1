# ============================================================
# Antrag 3000 – ADMIN-App: eine neue Version in EINEM Befehl veroeffentlichen.
#
# Gegenstueck zu release/veroeffentlichen.ps1 der Nutzer-App, nur fuer die
# Admin-App. Nutzt DENSELBEN Signatur-Schluessel (der Update-Pubkey in
# admin-app/src-tauri/tauri.conf.json ist identisch mit dem der Nutzer-App),
# liefert die Pakete aber aus dem NAS-Unterordner  updates/admin/  aus, damit
# sich Admin- und Nutzer-Updates nie vermischen.
#
# Macht nacheinander:
#   1. alte Setup-Dateien aufraeumen (sonst landet evtl. die falsche Version
#      im Manifest),
#   2. signiert bauen (npm run tauri build),
#   3. das Update-Manifest latest.json erzeugen,
#   4. den Ziel-Ordner updates/admin/ auf der NAS anlegen (falls neu),
#   5. Installer + latest.json per SSH/scp auf die NAS hochladen (Tailscale),
#   6. am Server gegenpruefen, dass version/url/Signatur zusammenpassen.
#
# Aufruf (im admin-app-Ordner):
#   .\release\veroeffentlichen.ps1 "Was ist neu in dieser Version"
#
# Voraussetzungen:
#   - Signatur-Schluessel unter  $HOME\.tauri\antrag3000.key  (derselbe wie
#     bei der Nutzer-App),
#   - SSH-Zugang zur NAS (Tailscale). Ohne SSH-Schluessel fragt scp/ssh nach
#     dem NAS-Passwort – einmal pro Verbindung.
# ============================================================

param(
  [Parameter(Mandatory = $true, Position = 0)]
  [string]$Notes,

  [string]$SshUser     = "admin",
  [string]$NasHost     = "nas-yh.tail73a506.ts.net",
  # Admin-Updates liegen im UNTERORDNER "admin" (getrennt von der Nutzer-App).
  [string]$UpdatesPfad = "/volume1/docker/antrag3000/updates/admin",
  [string]$KeyDatei    = "$HOME\.tauri\antrag3000.key",
  # Endpoint nur zur Schluss-Pruefung (muss zu tauri.conf.json passen).
  [string]$PruefUrl    = "http://nas-yh.tail73a506.ts.net:8445/updates/admin/latest.json"
)

$ErrorActionPreference = "Stop"
# Ins admin-app-Stammverzeichnis wechseln (eine Ebene ueber diesem Skript).
$wurzel = Split-Path -Parent $PSScriptRoot
Set-Location $wurzel

function Schritt($text) { Write-Host "`n=== $text ===" -ForegroundColor Cyan }

# --- Version aus tauri.conf.json lesen ----------------------------------
$conf    = Get-Content "src-tauri\tauri.conf.json" -Raw | ConvertFrom-Json
$version = $conf.version
Write-Host "Admin-Version laut tauri.conf.json: $version" -ForegroundColor Green

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
Schritt "1/6  Alte Setup-Dateien aufraeumen"
$nsis = "src-tauri\target\release\bundle\nsis"
if (Test-Path $nsis) {
  Get-ChildItem $nsis -Filter "*-setup.exe*" -ErrorAction SilentlyContinue | ForEach-Object {
    Write-Host "  entferne $($_.Name)"
    Remove-Item $_.FullName -Force
  }
}

# --- 2. Signiert bauen ---------------------------------------------------
# createUpdaterArtifacts steht in der Admin-tauri.conf.json auf true, daher
# entsteht beim signierten Bauen automatisch die zugehoerige .sig-Datei.
Schritt "2/6  Signiert bauen (npm run tauri build)"
npm run tauri build
if ($LASTEXITCODE -ne 0) { throw "Build fehlgeschlagen." }

# --- 3. Manifest erzeugen ------------------------------------------------
Schritt "3/6  Manifest latest.json erzeugen"
node release/latest-json-bauen.mjs $Notes
if ($LASTEXITCODE -ne 0) { throw "Manifest-Erzeugung fehlgeschlagen." }

# Den frisch gebauten Installer zur aktuellen Version finden.
$setup = Get-ChildItem $nsis -Filter "*$version*-setup.exe" | Select-Object -First 1
if (-not $setup) { throw "Setup-Datei zur Version $version nicht gefunden in $nsis." }
$latest = "release\latest.json"
if (-not (Test-Path $latest)) { throw "latest.json nicht gefunden: $latest" }

# SHA-256-Pruefsumme erzeugen: damit die Echtheit der .exe auch OHNE
# Code-Signing-Zertifikat selbst geprueft werden kann
# (Get-FileHash <datei> -Algorithm SHA256 und vergleichen).
$hash     = (Get-FileHash $setup.FullName -Algorithm SHA256).Hash.ToLower()
$sumDatei = Join-Path $setup.DirectoryName ($setup.Name + ".sha256")
"$hash  $($setup.Name)" | Set-Content -Path $sumDatei -Encoding ascii
Write-Host "SHA-256: $hash" -ForegroundColor Green

# --- 4. Ziel-Ordner auf der NAS anlegen (falls neu) ---------------------
# Beim allerersten Admin-Release existiert updates/admin/ noch nicht; scp
# wuerde dann scheitern. mkdir -p ist harmlos, wenn der Ordner schon da ist.
Schritt "4/6  Ziel-Ordner auf der NAS sicherstellen ($UpdatesPfad)"
& ssh "${SshUser}@${NasHost}" "mkdir -p '$UpdatesPfad'"
if ($LASTEXITCODE -ne 0) { throw "Konnte $UpdatesPfad auf der NAS nicht anlegen." }

# --- 5. Hochladen (scp ueber Tailscale) ---------------------------------
Schritt "5/6  Hochladen auf die NAS ($SshUser@$NasHost)"
Write-Host "  -> $($setup.Name)"
Write-Host "  -> $($setup.Name).sha256"
Write-Host "  -> latest.json"
# scp ueberschreibt vorhandene Dateien. Beide Quellen in EINEM Aufruf, damit
# nur EINE Verbindung noetig ist. -O erzwingt das klassische SCP-Protokoll
# (Synology-SSH hat das SFTP-Subsystem oft nicht aktiviert).
& scp -O $setup.FullName $sumDatei $latest "${SshUser}@${NasHost}:${UpdatesPfad}/"
if ($LASTEXITCODE -ne 0) {
  Write-Host "`nUpload fehlgeschlagen." -ForegroundColor Red
  Write-Host "Falls 'Permission denied': der SSH-Benutzer darf nicht in" -ForegroundColor Yellow
  Write-Host "$UpdatesPfad schreiben. Dann einmalig den Ordner schreibbar machen" -ForegroundColor Yellow
  Write-Host "oder die Dateien per File Station ablegen." -ForegroundColor Yellow
  throw "scp-Upload fehlgeschlagen."
}

# --- 6. Am Server gegenpruefen ------------------------------------------
Schritt "6/6  Veroeffentlichung pruefen"
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

Write-Host "`nFertig: Admin-Version $version ist veroeffentlicht. Admin-Geraete mit" -ForegroundColor Green
Write-Host "aelterer Version sehen das Update per Knopf 'Auf Updates pruefen'." -ForegroundColor Green
