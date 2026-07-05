# Förderer-Logos ausrollen (Etappe 3c-2)

Damit die Admin-App das **volle** Förderer-Logo hochladen und die Nutzer-App es
für die Danksagung abrufen kann, muss der Sync-Dienst auf der NAS **einmal neu
gebaut** werden. Grund: Der Server hat eine neue Tabelle (`logo`) und neue
Routen (`/api/admin/logos/…`, `/api/logos/…`), und Caddy bekommt eine neue
öffentliche Route (`/logos/…`).

Es gehen **keine Daten verloren**: Die Datenbank liegt im Docker-Volume
`api_data` und bleibt erhalten. Die neue `logo`-Tabelle legt der Dienst beim
Start automatisch an (kein manuelles „Datenbank ändern").

## Schritt für Schritt

1. **Auf die NAS verbinden** (Tailscale/SSH), in den Server-Ordner wechseln:
   ```sh
   cd /volume1/docker/antrag3000
   ```

2. **Neuen Stand holen.** Wenn der Ordner ein git-Klon ist:
   ```sh
   git pull
   ```
   Falls du die Dateien von Hand kopierst, müssen mindestens diese aktuell sein:
   - `api/` (der ganze Ordner – enthält den neuen Server-Code)
   - `Caddyfile.tailscale`

3. **Neu bauen und starten** (baut das `api`-Image neu, lädt Caddy neu):
   ```sh
   sudo docker compose -f docker-compose.tailscale.yml up -d --build
   ```
   (`sudo`, weil der Docker-Zugriff auf der Synology Root braucht.)

4. **Kurz prüfen**, dass alles läuft:
   ```sh
   sudo docker compose -f docker-compose.tailscale.yml ps
   ```
   Beide Dienste (`caddy`, `api`) sollen „Up" sein.

## Prüfen, dass die Logo-Route steht

Ein noch nicht vorhandenes Logo muss **404** liefern (nicht 502/„Fehler"):

```sh
curl -s -o /dev/null -w "%{http_code}\n" http://nas-yh.tail73a506.ts.net:8445/logos/gibt-es-nicht
```
Erwartet: `404`. Kommt `502` oder nichts, hat Caddy die neue Route noch nicht –
dann Schritt 3 wiederholen (Container wirklich neu erstellt?).

## Danach

- **Admin-App:** Beim „In den Katalog übernehmen" wird das volle Logo jetzt
  automatisch hochgeladen (der Eintrag bekommt `logo_id`). Schlägt der Upload
  fehl, erscheint nur ein Hinweis – die Übernahme läuft trotzdem durch, die
  kleine Kachel-Vorschau ist gesetzt.
- **Nutzer-App:** Holt das volle Logo bei Bedarf über `logo_id` (Danksagung).

## Rückgängig machen

Nur den alten Code auschecken und Schritt 3 erneut ausführen. Die `logo`-Tabelle
darf stehen bleiben – sie stört den alten Stand nicht.
