# Antrag 3000

Desktop-Anwendung für freischaffende Künstler:innen und kleine Teams (1–5 Personen)
zur Recherche, Verwaltung und Vorbereitung von Fördermittel-Anträgen für Kunst- und
Kulturprojekte im DACH-Raum (plus dort nutzbare internationale Förderungen).

> Status: in aktiver Entwicklung / Pilotphase.

## Grundgedanke: Datensouveränität

Sensible Daten (Name, Adresse, IBAN, Budgets, Kostenpläne, Projektbeschriebe, Belege)
werden **ausschließlich lokal** und **verschlüsselt** auf dem Gerät gespeichert. Sie
verlassen das Gerät **nie** über eine Netzwerkverbindung. Ein verlorenes lokales
Passwort bedeutet per Design, dass diese Daten nicht wiederherstellbar sind
(dafür gibt es eine „Neu aufsetzen"-Funktion).

Zwei strikt getrennte Ebenen:

- **Lokal (sensibel):** verschlüsselte lokale Ablage (Argon2id zur
  Schlüsselableitung, AES-256-GCM zur Verschlüsselung).
- **Sync (unkritisch):** Förder-Datenbank, Merklisten, Status-Anzeigen,
  Deadlines/Kalender – optional über eine eigene NAS (HTTPS + mTLS).

## Funktionen (Auswahl)

- Eingangs-Fragebogen → passende Förderungen (Matching mit harten und weichen Kriterien)
- Anträge vorbereiten und als Word-Dokument (`.docx`) exportieren
- Abrechnung: Belege erfassen, anteilig Geldquellen zuordnen, Verwendungsnachweis (PDF/Word)
- Fristen-Kalender, Merkliste und Status-Verwaltung
- Signiertes App-Selbstupdate (optional über eigene NAS)

## Technik

- **Desktop:** Tauri 2
- **Frontend:** SvelteKit / Svelte 5
- **Backend:** bewusst kleiner Rust-Kern (Dateizugriff, Verschlüsselung,
  mTLS-Client, Word-/PDF-Erzeugung); alle übrige Logik im Frontend
- **Ziel-Plattformen:** Windows (primär), macOS (Pilot), Linux perspektivisch

## Bauen

Voraussetzungen: Node.js, Rust-Toolchain und die
[Tauri-Systemvoraussetzungen](https://tauri.app/start/prerequisites/).

```bash
npm install
npm run tauri build
```

## Lizenz

[GPL-3.0-or-later](LICENSE) — GNU General Public License, Version 3 oder später.
