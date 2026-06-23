# Eine neue App-Version bauen & signiert veröffentlichen

*(Phase 3 / Etappe 5 – signiertes Selbstupdate. Für den Admin.)*

Die App aktualisiert sich selbst – aber **nur** mit Paketen, die mit
**deinem privaten Signatur-Schlüssel** unterschrieben sind. Der öffentliche
Gegenpart steckt fest in der App. Selbst eine kompromittierte NAS kann so
keinen Schadcode verteilen.

---

## Einmalig: den Signatur-Schlüssel erzeugen

> Das machst **nur du**, **einmal**. Privaten Schlüssel und Passwort danach
> sicher verwahren (Passwort-Manager / verschlüsseltes Backup). **Nie** ins
> Git, **nie** auf die NAS.

1. Schlüsselpaar erzeugen (fragt nach einem Passwort – merke es dir gut):
   ```powershell
   New-Item -ItemType Directory -Force "$HOME\.tauri"
   npm run tauri signer generate -- -w "$HOME\.tauri\antrag3000.key"
   ```
   Ergebnis: `antrag3000.key` (privat, geheim!) und `antrag3000.key.pub`
   (öffentlich).

2. Den **Inhalt** von `antrag3000.key.pub` in
   `src-tauri/tauri.conf.json` unter `plugins.updater.pubkey` eintragen
   (ersetzt den dort stehenden Platzhalter/Wegwerf-Schlüssel).

3. Einmal committen (nur die Konfiguration mit dem **öffentlichen**
   Schlüssel – der private bleibt außerhalb des Projekts!).

> **Schlüssel verloren?** Dann kannst du keine Updates mehr signieren. Du
> müsstest ein neues Paar erzeugen, den neuen öffentlichen Schlüssel in die
> App einbauen und diese neue App-Version **einmal von Hand** an alle
> verteilen (weil die alte App den neuen Schlüssel noch nicht kennt).

---

## Pro Release: bauen, signieren, hochladen

### 1. Versionsnummer erhöhen (an drei Stellen gleich)
- `src-tauri/tauri.conf.json` → `"version"`
- `src-tauri/Cargo.toml` → `version`
- `package.json` → `"version"`

z. B. von `0.1.0` auf `0.2.0`.

### 2. Signiert bauen
In PowerShell im Projektordner:
```powershell
$env:TAURI_SIGNING_PRIVATE_KEY = Get-Content "$HOME\.tauri\antrag3000.key" -Raw
$env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD = "DEIN-SCHLUESSEL-PASSWORT"
npm run tauri build
```
Das erzeugt unter `src-tauri/target/release/bundle/nsis/`:
- `… _x64-setup.exe` – der Installer
- `… _x64-setup.exe.sig` – die Signatur dazu

### 3. Manifest (`latest.json`) erzeugen
```powershell
node release/latest-json-bauen.mjs "Was ist neu in dieser Version"
```
Das liest Version + Signatur automatisch aus und schreibt
`server/updates/latest.json`. Der Text in Anführungszeichen erscheint der
Nutzer:in im Update-Dialog.

### 4. Auf die NAS hochladen
In den Ordner `updates/` auf der NAS (denselben, den Caddy unter `:8445`
ausliefert) **zwei Dateien** legen:
1. den `…-setup.exe`-Installer aus Schritt 2
2. die `latest.json` aus Schritt 3

> Die `.sig`-Datei **nicht** hochladen – ihr Inhalt steht schon im
> `latest.json`. Beim Hochladen wie immer **wirklich überschreiben**
> (alte `latest.json` ggf. erst zur Seite schieben).

### 5. Fertig
Beim nächsten Entsperren (oder per Knopf **„⬆ Update"**) sehen alle die neue
Version, bestätigen, und die App aktualisiert sich und startet neu.

---

## Sicherheits-Kurzfassung
- **Signatur = Echtheits-Siegel.** Nur dein privater Schlüssel erzeugt ein
  gültiges. Die App prüft jedes Paket gegen den eingebauten öffentlichen
  Schlüssel und verweigert alles Unsignierte/Manipulierte.
- **Transport:** Tailscale-WireGuard (verschlüsselt, nur im Tailnet).
- **Privater Schlüssel:** nur bei dir, mit Passwort – nicht im Git, nicht
  auf der NAS.
