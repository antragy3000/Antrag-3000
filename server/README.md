# Antrag 3000 – Sync-Dienst (Phase 2)

Dieser Ordner enthält den kleinen Server, der **nur unkritische
Team-Daten** synchronisiert (Förder-Status, Merklisten, Fristen). Sensible
Daten bleiben verschlüsselt auf den Geräten und erreichen diesen Dienst
**nie** (Garantie: siehe `../src/lib/sync.js` und `../tools/sync-waechter.mjs`).

> Stand Etappe 2: Der Server existiert hier als Dateien und ist
> übersetzbar. **Aufs NAS gebracht** wird er erst in **Etappe 3** – dort
> kommen auch die Zertifikate (Team-CA + Geräte-Ausweise) und die
> bebilderte Schritt-für-Schritt-Anleitung dazu.

## Was die Dateien tun (in einfacher Sprache)

| Datei | Aufgabe |
|---|---|
| `api/` | Der eigentliche Sync-Dienst – ein winziges Rust-Programm. Es kann nur „Board lesen" und „Board schreiben". |
| `api/src/main.rs` | Der ganze Dienst: Datenbank-Tabellen, Geräte-Erkennung, die paar Endpunkte. |
| `api/Dockerfile` | Bauanleitung für das Programm-Image (erst übersetzen, dann winziges Laufzeit-Image ohne Shell). |
| `Caddyfile` | Der „Türsteher" Caddy (Variante B): macht HTTPS nach außen und lässt **nur Geräte mit gültigem Team-Ausweis** durch (mTLS). |
| `docker-compose.yml` | **Variante B** (DDNS + Port 443): Caddy + api. |
| `docker-compose.cloudflare.yml` | **Variante A** (empfohlen): Cloudflare Tunnel + api, **keine offenen Ports**. mTLS macht Cloudflare Access. Siehe `Cloudflare-Einrichtung.md`. |
| `Cloudflare-Einrichtung.md` | Schritt-für-Schritt: Tunnel + Access-mTLS + Start. |
| `katalog/` | Hier legt der Admin den verteilten Förder-Katalog ab (`foerderungen.json`). Upload-Werkzeug folgt in Etappe 4. |
| `.env.example` | Vorlage für deine Einstellungen. Kopieren zu `.env`. |
| `ca/` | Hier liegen die Zertifikate. Werden **nicht** versioniert. |

## Datenmodell (mandantenfähig von Anfang an)
- **konto** – ein Team/Abo (MVP: genau eines, „Team").
- **nutzer** – gehören zu einem Konto (MVP: ein „Team-Login").
- **geraet** – ein Gerät = ein Client-Zertifikat (per Fingerabdruck erkannt).
- **board_projekt** – die geteilten, unkritischen Projekt-Daten (als
  JSON-Block pro Projekt, mit Versionsnummer für „letzte Änderung gewinnt").

So lässt sich später leicht auf **mehrere Teams**, **echte Einzel-Logins**
und ein **Abo-Modell** erweitern, ohne das Schema umzubauen.

## Die Endpunkte (was der Client später nutzt)
- `GET  /api/health` – „läuft der Dienst?" (Antwort: `ok`).
- `GET  /api/board` – alle Board-Projekte des eigenen Teams holen.
- `PUT  /api/board/{projekt_id}` – ein Board-Projekt anlegen/aktualisieren.
- `DELETE /api/board/{projekt_id}` – ein Board-Projekt entfernen.
- `GET  /api/katalog` – den verteilten Förder-Katalog holen (Phase 3).
- `GET  /api/katalog/version` – nur Stand/Version (schnelle „Gibt's Neues?"-Abfrage).

Jede Anfrage wird über das **Geräte-Zertifikat** dem Team-Konto zugeordnet.
Der Dienst liest dazu den Header **`Cf-Client-Cert-Der-Base64`** (Cloudflare
Access, Variante A) **oder** `X-Client-Cert-DER` (Caddy, Variante B) und
bildet daraus den Fingerabdruck.

## Lokal übersetzen (zum Prüfen, ohne Docker)
Im Ordner `api/`:
```
cargo run
```
Dann antwortet `http://127.0.0.1:8080/api/health` mit `ok`.
(Ohne Caddy gibt es keinen Geräte-Ausweis – die Board-Endpunkte
antworten dann mit „401 Unauthorized". Das ist korrekt.)

## Skalierung später (Pilot → SaaS)
- Heute: SQLite (eine Datei) – ideal für 1–5 Personen auf der NAS.
- Später: nur das Feature `postgres` in `api/Cargo.toml` ergänzen und die
  Verbindungs-Adresse umstellen – der übrige Code bleibt gleich. Dazu
  echtes Login pro Nutzer (Schema ist schon da) und mehrere Kopien des
  Dienstes hinter einem Lastverteiler.
