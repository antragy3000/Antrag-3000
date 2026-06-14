# ============================================================
# Antrag 3000 - Zertifikate erzeugen (Team-CA + Geraete-Ausweise)
#
# Erstellt EINMALIG deine eigene Mini-Zertifizierungsstelle (CA) und
# daraus pro Geraet einen Ausweis (Client-Zertifikat) fuer den mTLS-Zugang.
#
# WICHTIG:
#  - Der CA-Schluessel (ca\team-ca.key) ist dein "Aussteller-Stempel".
#    Er bleibt GEHEIM und OFFLINE auf diesem Rechner - niemals auf die
#    NAS, niemals per Mail.
#  - Jede Geraete-Datei (geraete\<name>.pem) enthaelt einen privaten
#    Schluessel. Uebergib sie nur PERSOENLICH/OFFLINE (USB), nicht per Mail.
#
# (Bewusst ASCII-only, damit Windows PowerShell das Skript ohne
#  Encoding-Probleme einliest.)
#
# Beispiele:
#   .\zertifikate-erzeugen.ps1 -Geraete "Laptop-Jenny"
#   .\zertifikate-erzeugen.ps1 -Geraete "Laptop-Jenny","Tablet-Anna"
#   .\zertifikate-erzeugen.ps1            (fragt die Geraetenamen ab)
# ============================================================

param(
  [string[]]$Geraete = @(),
  [string]$Adresse = "",
  [string]$Ordner = $PSScriptRoot,
  [int]$CaJahre = 10,
  [int]$GeraetTage = 825
)

$ErrorActionPreference = "Stop"
# Git-OpenSSL ist MSYS-basiert und wuerde "/O=..." in einen Windows-Pfad
# umbiegen - das hier verhindert das.
$env:MSYS_NO_PATHCONV = "1"

function Find-OpenSSL {
  foreach ($p in @(
    "C:\Program Files\Git\usr\bin\openssl.exe",
    "C:\Program Files\Git\mingw64\bin\openssl.exe",
    "C:\Program Files (x86)\Git\usr\bin\openssl.exe"
  )) { if (Test-Path $p) { return $p } }
  $c = (Get-Command openssl -ErrorAction SilentlyContinue).Source
  if ($c) { return $c }
  throw "OpenSSL nicht gefunden. Installiere 'Git for Windows' (enthaelt OpenSSL)."
}

# Ruft openssl mit einer Argument-Liste auf (als EIN Array, damit
# PowerShell die -Optionen nicht als eigene Parameter missversteht).
function Run-SSL {
  param([string[]]$A)
  & $ssl @A
  if ($LASTEXITCODE -ne 0) { throw "OpenSSL-Schritt fehlgeschlagen: openssl $($A -join ' ')" }
}

$ssl = Find-OpenSSL
Write-Host "OpenSSL: $ssl" -ForegroundColor DarkGray

if ($Geraete.Count -eq 0) {
  $Geraete = Read-Host "Geraetenamen (mit Komma trennen, z. B. Laptop-Jenny,Tablet-Anna)"
}
# Eingaben aufteilen (egal ob als Array oder als ein Komma-String) und saeubern.
$Geraete = @($Geraete) | ForEach-Object { $_ -split ',' } | ForEach-Object { $_.Trim() } | Where-Object { $_ }
if ($Geraete.Count -eq 0) { throw "Keine Geraetenamen angegeben." }

# Team-Adresse (DDNS) fuer das Zugangs-Paket. Wenn nicht uebergeben,
# aus .env (DOMAIN) lesen, sonst abfragen.
if (-not $Adresse) {
  $envPfad = Join-Path $Ordner ".env"
  if (Test-Path $envPfad) {
    $zeile = Get-Content $envPfad | Where-Object { $_ -match '^\s*DOMAIN\s*=' } | Select-Object -First 1
    if ($zeile) { $Adresse = ($zeile -split '=', 2)[1].Trim() }
  }
}
if (-not $Adresse) {
  $Adresse = (Read-Host "Team-Adresse (DDNS, z. B. deinteam.synology.me)").Trim()
}
if (-not $Adresse) { throw "Keine Team-Adresse angegeben." }

$caDir  = Join-Path $Ordner "ca"
$devDir = Join-Path $Ordner "geraete"
New-Item -ItemType Directory -Force -Path $caDir, $devDir | Out-Null

$caKey = Join-Path $caDir "team-ca.key"
$caCrt = Join-Path $caDir "team-ca.crt"

# --- 1. Team-CA (nur einmal) ---
if (Test-Path $caCrt) {
  Write-Host "Team-CA existiert bereits - wird wiederverwendet (gut so)." -ForegroundColor Green
} else {
  Write-Host "Erzeuge Team-CA ..." -ForegroundColor Cyan
  $caDays = $CaJahre * 365
  Run-SSL @("ecparam", "-name", "prime256v1", "-genkey", "-noout", "-out", $caKey)
  Run-SSL @("req", "-x509", "-new", "-key", $caKey, "-sha256", "-days", "$caDays", "-out", $caCrt,
            "-subj", "/O=Antrag 3000 Team/CN=Antrag 3000 Team CA",
            "-addext", "basicConstraints=critical,CA:TRUE,pathlen:0",
            "-addext", "keyUsage=critical,keyCertSign,cRLSign")
  Write-Host "  -> $caCrt (oeffentlich, fuer Caddy)" -ForegroundColor Green
  Write-Host "  -> $caKey (GEHEIM, offline halten!)" -ForegroundColor Yellow
}

# --- 2. Geraete-Ausweise ---
foreach ($name in $Geraete) {
  $safe = ($name -replace '[^A-Za-z0-9_.-]', '_')
  $key = Join-Path $devDir "$safe.key"
  $csr = Join-Path $devDir "$safe.csr"
  $crt = Join-Path $devDir "$safe.crt"
  $pem = Join-Path $devDir "$safe.pem"
  $ext = Join-Path $devDir "$safe.ext"

  Write-Host "Erzeuge Geraete-Ausweis fuer '$name' ..." -ForegroundColor Cyan
  Run-SSL @("ecparam", "-name", "prime256v1", "-genkey", "-noout", "-out", $key)
  Run-SSL @("req", "-new", "-key", $key, "-out", $csr, "-subj", "/O=Antrag 3000 Team/CN=$name")

  @"
basicConstraints=critical,CA:FALSE
keyUsage=critical,digitalSignature
extendedKeyUsage=clientAuth
"@ | Set-Content -Encoding ascii $ext

  Run-SSL @("x509", "-req", "-in", $csr, "-CA", $caCrt, "-CAkey", $caKey, "-CAcreateserial",
            "-days", "$GeraetTage", "-sha256", "-extfile", $ext, "-out", $crt)

  # Kombinierte PEM-Datei (privater Schluessel + Zertifikat) fuer die App.
  Get-Content $key, $crt | Set-Content -Encoding ascii $pem
  Remove-Item $csr, $ext -ErrorAction SilentlyContinue

  Run-SSL @("verify", "-CAfile", $caCrt, $crt)

  # Gebuendeltes Zugangs-Paket: Ausweis (PEM) + Team-Adresse in EINER
  # Datei. Die App liest daraus alles, prueft den Ausweis und benennt das
  # Geraet aus dem Zertifikat (CN). So entfaellt das Adresse-Eintippen.
  $paketPfad = Join-Path $devDir "$safe.a3kpaket"
  $paket = [ordered]@{
    typ         = "antrag3000-zugangspaket"
    version     = 1
    adresse     = $Adresse
    ausweis_pem = (Get-Content $pem -Raw)
  }
  ($paket | ConvertTo-Json) | Set-Content -Encoding ascii $paketPfad
  Write-Host "  -> $paketPfad" -ForegroundColor Green
  Write-Host "     Zugangs-Paket fuer '$name' - OFFLINE uebergeben (enthaelt den privaten Schluessel)!" -ForegroundColor Yellow
}

Write-Host ""
Write-Host "Fertig." -ForegroundColor Green
Write-Host "Naechste Schritte:" -ForegroundColor White
Write-Host "  - ca\team-ca.crt liegt schon am richtigen Ort (Caddy nutzt sie)."
Write-Host "  - Jedes geraete\<name>.a3kpaket (Zugangs-Paket) kommt auf das"
Write-Host "    jeweilige Geraet (offline, z. B. USB) und wird dort in der App geladen."
Write-Host "  - ca\team-ca.key NIEMALS weitergeben oder auf die NAS kopieren."
