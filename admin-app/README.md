# Antrag 3000 – Admin-App

Schlankes, **eigenes** Werkzeug (getrennt von der Nutzer-App) zum zentralen
Pflegen der Förder-Datenbank:

- **Meldungen** sichten (gemeldete falsche/veraltete Förderungen),
- **geteilte Förderer** einsehen,
- den **Förder-Katalog hochladen** (wird danach an alle Team-Geräte verteilt).

Die App speichert **keine** sensiblen Daten. Zum Anmelden lädt sie ein
vorhandenes Zugangs-Paket (`.a3kpaket`, dieselbe Datei wie die Nutzer-App;
liefert den Geräte-Ausweis für mTLS und die Team-Adresse). Zweiter Faktor ist
ein **TOTP-Code** aus einer Authenticator-App.

## Voraussetzung am Server (einmalig)

1. TOTP-Geheimnis erzeugen:
   ```
   docker compose -f docker-compose.tailscale.yml run --rm api totp
   ```
2. Das ausgegebene `ADMIN_TOTP_SECRET=…` in `server/.env` eintragen. Der
   Befehl zeigt außerdem einen **QR-Code direkt im Terminal** – einmal mit
   der Authenticator-App scannen (Google Authenticator, Aegis, 1Password …).
   Alternativ den base32-Schlüssel manuell in der App eintragen (Typ:
   zeitbasiert/TOTP). Die `otpauth://`-URL enthält das Geheimnis – nicht in
   einen Online-QR-Dienst geben.
3. Server neu bauen:
   ```
   docker compose -f docker-compose.tailscale.yml up -d --build
   ```

## Admin-App starten

Tauri-Toolchain wie bei der Haupt-App vorausgesetzt (Rust + Node).

```
cd admin-app
npm install
npm run tauri dev      # zum Entwickeln/Testen
npm run tauri build    # erzeugt die fertige Admin-App
```

## Bedienung

1. **Zugangs-Paket wählen** (`.a3kpaket`).
2. **Authenticator-Code** (6-stellig) eingeben → **Anmelden**.
   Der Server prüft den Code und gibt eine 30-Minuten-Sitzung zurück.
3. Tabs nutzen: **Vorschläge**, **Meldungen**, **Geteilte Förderer**,
   **Katalog hochladen**.

## Sammler (Hybrid-Update)

Der Sammler vergleicht eine Rohquelle (Kandidaten-Liste) mit dem aktuellen
Katalog und legt **Vorschläge** an (neu/geändert), die du im Tab
**Vorschläge** freigibst (→ in den Katalog übernehmen) oder verwirfst.

Sammler-Lauf auf dem Server (Quelldatei z. B. unter `server/katalog/`, dort
nach `/srv` gemountet):

```
docker compose -f docker-compose.tailscale.yml run --rm api sammeln /srv/rohquelle.json
```

Ein Beispiel liegt in `server/katalog/rohquelle-beispiel.json`. Für einen
**wöchentlichen** Lauf kann dieser Befehl per DSM-Aufgabenplaner (Synology)
oder Cron zeitgesteuert werden. Freigegebene Vorschläge landen sofort im
verteilten Katalog (`GET /api/katalog`).

## Sicherheit

- Zwei Faktoren: gültiges Team-Zertifikat (mTLS) **und** TOTP-Code.
- Ohne `ADMIN_TOTP_SECRET` am Server ist der Admin-Zugang deaktiviert.
- Läuft die 30-Minuten-Sitzung ab, einfach neu anmelden.
