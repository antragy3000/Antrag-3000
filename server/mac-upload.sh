#!/usr/bin/env bash
# ============================================================
# Erzwungener Befehl fuer den CI-Deploy-Schluessel des macOS-Pilots.
#
# In ~/.ssh/authorized_keys des Benutzers "antrag" steht VOR dem
# Deploy-Schluessel:
#   command="/bin/bash /home/antrag/Antrag-3000/server/mac-upload.sh",restrict ssh-ed25519 AAAA...
#
# Damit kann dieser Schluessel GENAU zwei Dinge – egal, welchen Befehl der
# Client mitschickt (er landet nur in SSH_ORIGINAL_COMMAND):
#   ssh ... "dmg Antrag-3000_<x.y.z>_universal.dmg" < datei.dmg
#   ssh ... "manifest"                               < mac.json
# Keine Shell, kein Lesen, keine anderen Pfade, kein scp/sftp, keine
# Weiterleitungen ("restrict"). Die Datei wird erst vollstaendig empfangen
# und geprueft (Name, Groesse, Inhalt) und erst dann atomar ersetzt.
#
# WARUM: Vorher hatte der Schluessel eine volle Shell auf dem Server und
# haette damit auch .env, CA-Schluessel und Datenbank erreicht. Leckt er
# jetzt aus der CI, kann ein Angreifer hoechstens eine .dmg/mac.json ablegen,
# die diese Pruefungen besteht (Befund D-08, Delta-Pentest 09/2026).
# ============================================================
set -euo pipefail
umask 022

BASIS="/home/antrag/Antrag-3000/server/updates"
MAX_DMG=$((512 * 1024 * 1024)) # 512 MiB
MAX_JSON=16384                 # 16 KiB

ablehnen() {
  echo "abgelehnt: $*" >&2
  exit 2
}

befehl="${SSH_ORIGINAL_COMMAND:-}"
# Mehrzeilige Befehle sind nie legitim.
case "$befehl" in
  *$'\n'* | *$'\r'*) ablehnen "Zeilenumbruch im Befehl" ;;
esac
# Nur zwei Woerter sind erlaubt; es wird nichts ausgewertet (kein eval).
read -r art name rest <<<"$befehl" || true
[ -z "${rest:-}" ] || ablehnen "zu viele Angaben"

case "${art:-}" in
  dmg)
    [[ "${name:-}" =~ ^Antrag-3000_[0-9]+\.[0-9]+\.[0-9]+_universal\.dmg$ ]] || ablehnen "Dateiname"
    ziel="$BASIS/mac/$name"
    max=$MAX_DMG
    ;;
  manifest)
    [ -z "${name:-}" ] || ablehnen "manifest erwartet keinen Dateinamen"
    ziel="$BASIS/mac.json"
    max=$MAX_JSON
    ;;
  *) ablehnen "unbekannte Aktion" ;;
esac

mkdir -p "$BASIS/mac"
tmp="$(mktemp "$(dirname "$ziel")/.upload.XXXXXX")"
trap 'rm -f "$tmp"' EXIT

# Hoechstens max+1 Byte annehmen: so wird eine zu grosse Datei sicher
# erkannt, ohne dass sie die Platte fuellen kann.
head -c "$((max + 1))" >"$tmp"
groesse=$(stat -c %s "$tmp")
{ [ "$groesse" -gt 0 ] && [ "$groesse" -le "$max" ]; } || ablehnen "Groesse $groesse Byte"

if [ "$art" = dmg ]; then
  # Apple-Disk-Images (UDIF) enden mit einem 512-Byte-Block, der mit "koly" beginnt.
  [ "$(tail -c 512 "$tmp" | head -c 4)" = "koly" ] || ablehnen "keine .dmg-Datei"
else
  # Muss ein JSON-Objekt sein, das auf eine .dmg unter /updates/mac/ zeigt
  # und eine SHA-256-Pruefsumme traegt (die App prueft das zusaetzlich).
  [ "$(head -c 1 "$tmp")" = "{" ] || ablehnen "kein JSON-Objekt"
  grep -Eq '"url": *"https://sync\.antrag3000\.de/updates/mac/Antrag-3000_[0-9]+\.[0-9]+\.[0-9]+_universal\.dmg"' "$tmp" ||
    ablehnen "url zeigt nicht auf den eigenen Update-Ort"
  grep -Eq '"sha256": *"[0-9a-f]{64}"' "$tmp" || ablehnen "sha256 fehlt oder falsches Format"
fi

chmod 644 "$tmp"
mv -f "$tmp" "$ziel"
trap - EXIT
echo "OK: $ziel ($groesse Byte)"
