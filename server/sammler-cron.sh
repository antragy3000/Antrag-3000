#!/bin/sh
# ============================================================
# Wöchentlicher Sammler-Lauf auf der NAS (Synology Task Scheduler).
#
# WAS DAS MACHT (in einfachen Worten):
# Startet einmal kurz einen Wegwerf-Container aus demselben Image wie der
# Server, der NUR den Sammler ausführt: Er vergleicht die Rohquelle
# (server/katalog/rohquelle.json) mit dem aktuell veröffentlichten Katalog
# in der Server-Datenbank und legt aus den Unterschieden Vorschläge an
# (neu / geändert). Diese Vorschläge gibst DU danach in der Admin-App frei
# – nichts wird automatisch veröffentlicht.
#
# Der laufende Server bleibt unberührt: Beide nutzen dieselbe SQLite-Datei,
# das geht dank WAL-Modus + busy_timeout gefahrlos gleichzeitig.
#
# EINRICHTEN auf der Synology:
#   1. Rohquelle erzeugen (am Arbeitsrechner):  node import/rohquelle-bauen.mjs
#      und die Datei server/katalog/rohquelle.json auf die NAS in den
#      gleichen katalog/-Ordner legen wie foerderungen.json.
#   2. DSM > Systemsteuerung > Aufgabenplaner > Erstellen >
#      Geplante Aufgabe > Benutzerdefiniertes Skript.
#      - Zeitplan: wöchentlich (z. B. Montag 03:00).
#      - Befehl:  sh /pfad/zu/server/sammler-cron.sh
#   3. Danach in der Admin-App unter "Vorschläge" prüfen und freigeben.
#
# Test von Hand (auf der NAS, im server/-Ordner):
#   sh sammler-cron.sh
# ============================================================
set -eu

# In den Ordner dieses Skripts wechseln (dort liegen die compose-Dateien).
cd "$(dirname "$0")"

# Welche compose-Datei? Standard: Tailscale-Variante. Über Umgebungs-
# variable COMPOSE_DATEI überschreibbar.
COMPOSE_DATEI="${COMPOSE_DATEI:-docker-compose.tailscale.yml}"

# Pfad der Rohquelle IM CONTAINER. Der katalog/-Ordner ist als /srv
# eingehängt (siehe compose-Datei), also liegt rohquelle.json unter /srv.
ROHQUELLE="${ROHQUELLE:-/srv/rohquelle.json}"

if [ ! -f "katalog/rohquelle.json" ]; then
  echo "FEHLER: katalog/rohquelle.json fehlt. Zuerst 'node import/rohquelle-bauen.mjs'"
  echo "        ausführen und die Datei in den katalog/-Ordner legen." >&2
  exit 1
fi

echo "[$(date '+%Y-%m-%d %H:%M:%S')] Sammler-Lauf startet ($COMPOSE_DATEI, $ROHQUELLE) ..."

# Einmaliger Wegwerf-Container: nur der Sammler, danach automatisch weg
# (--rm). 'sammeln <datei>' wird als Argument an den Entrypoint (die Server-
# Binary) angehängt; mit diesem Argument läuft sie im Sammler-Modus statt
# als Webserver.
docker compose -f "$COMPOSE_DATEI" run --rm api sammeln "$ROHQUELLE"

echo "[$(date '+%Y-%m-%d %H:%M:%S')] Sammler-Lauf fertig. Jetzt in der Admin-App unter 'Vorschläge' prüfen."
