# ============================================================
# Haelt die Pruefsumme einer frisch veroeffentlichten Version im Git fest.
#
# Wird am Ende der drei veroeffentlichen.ps1 aufgerufen. Warum im Git: Nach dem
# naechsten Push traegt GitHub Actions sie daraus als Release ein, ganz ohne
# zusaetzlichen Schluessel auf diesem Rechner
# (.github/workflows/pruefsummen.yml).
#
# Committet genau diese eine Datei. Was sonst gerade vorgemerkt ist, bleibt
# unberuehrt. Gepusht wird nicht.
# ============================================================

param(
  [Parameter(Mandatory = $true)] [string]$SumDatei,
  # z. B. "antrag-3000-0.5.0-windows" (siehe ARTEN in pruefsummen-veroeffentlichen.mjs)
  [Parameter(Mandatory = $true)] [string]$Name
)

$repo   = Split-Path -Parent $PSScriptRoot
$pfad   = "release/pruefsummen/$Name.sha256"
New-Item -ItemType Directory -Force (Join-Path $PSScriptRoot "pruefsummen") | Out-Null
Copy-Item -LiteralPath $SumDatei (Join-Path $repo $pfad) -Force

# safecrlf=false: spart die fuer diese Datei belanglose Zeilenende-Warnung von Git.
git -C $repo -c core.safecrlf=false add -- $pfad
git -C $repo -c core.safecrlf=false diff --cached --quiet -- $pfad
if ($LASTEXITCODE -eq 0) {
  Write-Host "`nPruefsumme $Name ist schon im Git festgehalten." -ForegroundColor Green
  return
}
git -C $repo -c core.safecrlf=false commit -q -m "Pruefsumme $Name" -- $pfad
if ($LASTEXITCODE -ne 0) {
  Write-Host "`nACHTUNG: Pruefsumme konnte nicht committet werden. Die Version ist trotzdem" -ForegroundColor Yellow
  Write-Host "veroeffentlicht; bitte $pfad von Hand committen." -ForegroundColor Yellow
  return
}
Write-Host "`nPruefsumme im Git festgehalten: $pfad" -ForegroundColor Green
Write-Host "Damit sie auf GitHub erscheint, jetzt noch pushen:  git push" -ForegroundColor Green
