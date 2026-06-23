# Update-Ordner (signierte App-Pakete)

Hier liegen die Dateien für das **signierte App-Selbstupdate** (Phase 3 /
Etappe 5). Caddy liefert diesen Ordner Tailscale-intern unter
`http://<NAS-Tailscale-Adresse>:8445/updates/` aus.

Zwei Sorten Dateien gehören hier hinein – beide erzeugst du beim Release
(siehe `server/release-bauen.md`):

1. **`latest.json`** – das Manifest. Sagt der App, welche Version aktuell
   ist, wo der Installer liegt und wie seine Signatur lautet. Vorlage:
   `latest.json.beispiel`.
2. **Der signierte Installer**, z. B. `Antrag.3000_0.2.0_x64-setup.exe`
   (Windows). Daneben gehört KEINE `.sig`-Datei auf den Server – die
   Signatur steht im `latest.json`.

> Sicherheit: Selbst wenn jemand hier eine manipulierte Datei ablegt,
> installiert die App sie NICHT – sie prüft die Signatur gegen den fest
> eingebauten öffentlichen Schlüssel. Nur mit dem privaten Admin-Schlüssel
> signierte Pakete werden akzeptiert.

Die echten Dateien werden **nicht** ins Git eingecheckt (siehe
`server/.gitignore`); nur diese README und die Beispiel-Vorlage.
