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

## Wichtig: Downgrade-Schutz (Audit-Befund F1)

Die Signatur deckt die **Bytes des Pakets** ab, NICHT die im `latest.json`
behauptete Versionsnummer. Theoretisch könnte also jemand, der in diesen
Ordner schreiben darf, ein **älteres, gültig signiertes** Paket mit einer
hoch gesetzten Manifest-Version als „Update" ausliefern – die App würde es
als neuer ansehen, die Signatur wäre gültig, und man landete auf einer
alten (evtl. verwundbaren) Version.

Einordnung und Schutz:

- Der Update-Kanal `:8445` läuft **nur in der Tailscale-Variante** und damit
  ausschließlich im privaten Tailnet. WireGuard authentifiziert die
  Verbindung Gerät↔NAS Punkt-zu-Punkt – ein normales Team-Mitglied kann sie
  **nicht** abhören oder umleiten (kein klassisches MITM möglich).
- Es bleibt: Wer in diesen `updates/`-Ordner schreiben kann, hat ohnehin
  Release-/Admin-Befugnis. **Halte den Ordner schreibgeschützt** (nur die
  Release-Person darf hinein; im Container ist er bereits `:ro` gemountet).
- **Räume beim Release alte Installer weg:** Es soll immer nur das AKTUELLE
  Paket im Ordner liegen, damit kein altes signiertes Paket zum
  Wiedereinspielen bereitsteht.
- Vollständige Absicherung (separate, bewusste Aufgabe): das `latest.json`
  selbst signieren bzw. die Versionsnummer in die Paket-Signatur binden –
  erst dann ist auch ein böswilliger Schreibzugriff auf den Ordner gegen
  Downgrade abgesichert.

Die echten Dateien werden **nicht** ins Git eingecheckt (siehe
`server/.gitignore`); nur diese README und die Beispiel-Vorlage.
