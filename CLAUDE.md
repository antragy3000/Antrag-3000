# Antrag 3000 – Projektgedächtnis

## Was diese Anwendung ist
Desktop-Anwendung (Tauri) für freischaffende Künstler:innen und kleine Teams
(1–5 Personen) zur Recherche, Verwaltung und Vorbereitung von
Fördermittel-Anträgen für Kunst-/Kulturprojekte (DACH-Region plus dort
nutzbare internationale Förderungen).

## Unverhandelbare Grundprinzipien
1. DATENSOUVERÄNITÄT: Sensible Daten (Name, Adresse, IBAN, Budget,
   Kostenfinanzplan, Steuerangaben, Projektbeschriebe) werden AUSSCHLIESSLICH
   lokal gespeichert, verschlüsselt mit einem Nutzerpasswort. Sie verlassen
   das Gerät NIE über eine Netzwerkverbindung. Kein Feature darf das aufweichen.
2. ZWEI DATENEBENEN, strikt getrennt:
   - Lokale Ebene (sensibel): verschlüsselte lokale Datenbank.
   - Sync-Ebene (unkritisch): Förder-Datenbank, Merklisten, Status-Anzeigen,
     Deadlines/Kalender. Liegt auf einer Synology NAS, Zugriff über
     HTTPS + mTLS (Client-Zertifikat).
3. Verlorenes lokales Passwort = Daten per Design nicht wiederherstellbar.
   Dafür existiert eine "Neu aufsetzen"-Funktion (lokale DB zurücksetzen
   und neu eingeben).

## Technik-Stack
- Desktop: Tauri 2.x. Frontend: HTML/CSS/JS (React oder Svelte, einfach halten).
- Rust-Backend so KLEIN wie möglich: nur Dateizugriff, Verschlüsselung,
  mTLS-HTTP-Client, Word-Erzeugung. Alle übrige Logik im JS-Frontend.
- Lokale Speicherung: SQLite, verschlüsselt (z. B. SQLCipher) ODER
  dateibasiert mit etablierter Krypto-Bibliothek (age/AES-GCM über
  Standard-Crates). Keine selbstgebaute Kryptografie.
- Sync-Server (Phase 2): kleiner Dienst im Docker-Container auf Synology NAS,
  REST-API, HTTPS + mTLS. Nutzerkonten-Struktur von Beginn an im Schema
  (MVP nutzt gemeinsames Team-Login, Architektur ist Abo-fähig vorbereitet).
- Word-Ausgabe: .docx-Erzeugung pro Förderung (Rust-Crate docx-rs oder
  Erzeugung über Template-Engine). Nutzer konvertieren selbst zu PDF.
- Ziel-OS: Windows zuerst, macOS/Linux später (Tauri ermöglicht alle drei).

## Arbeitsweise mit mir (dem Nutzer)
- Ich habe sehr geringes technisches Vorwissen. Erkläre bei jedem Rust-Code
  und jeder Architektur-Entscheidung das WARUM in einfacher Sprache.
- Kleine Schritte. Nach jedem Schritt sagen, wie ich das Ergebnis selbst
  testen kann.
- Nach jedem funktionierenden Zwischenstand: Git-Commit vorschlagen.
- Keine zusätzlichen Abhängigkeiten ohne kurze Begründung einführen.

## Ordnerstruktur der Nutzdaten (vom Programm verwaltet)
[Programmordner]/[Projektname]/[Förderungsname]/
- Pro Projekt: maschinenlesbare Antworten-Datei (Quelle der Wahrheit,
  JSON) + daraus erzeugte menschenlesbare Kopie (Markdown/Docx) mit
  Warnhinweis am Dateianfang. Rück-Sync menschlicher Änderungen nur über
  klar markierte Felder, nie über freies Einlesen von Fliesstext.

## Status-Ketten (fest, plus frei beschriftbarer "anderer Status")
- Antrag: in Bearbeitung / abgeschickt / Zusage erhalten / Absage erhalten /
  muss nachgebessert werden / anderer Status (frei beschriftbar)
- Checklisten-Punkt: noch nicht bearbeitet / muss angefragt werden /
  angefragt, warte auf Antwort / alle Infos zusammen / in Bearbeitung /
  abgeschlossen / anderer Status (frei beschriftbar)
- Nur aktueller Stand sichtbar, keine Historie (MVP).

## Matching-Logik
- Eingangs-Fragebogen (Multiple Choice) filtert die Förder-Datenbank.
- HARTE Kriterien (Nichterfüllung => unter "weitere Vorschläge" mit
  Hinweis, was nicht passt): Wohnsitz-Anforderung, Durchführungsort,
  Trägerschaft (inkl. studentisch ja/nein).
- WEICHE Kriterien (beeinflussen Rangfolge): Sparte, Projektart,
  Budgetgrösse, Zeitpunkt.
- Ausgabe: Rangliste passender Förderungen + Abschnitt "weitere Vorschläge".
- Hinweis anzeigen, wenn sich gemerkte Förderungen gegenseitig ausschliessen
  (Feld "unverträglich mit").

## Phasen (immer nur die aktuelle Phase bauen)
- Phase 0: Tauri-Toolchain + leeres lauffähiges Grundgerüst.
- Phase 1: Einzelplatz-MVP, komplett offline, ohne Sync.
- Phase 2: Sync-Ebene (NAS, HTTPS+mTLS, Team-Kalender/Status).
- Phase 3: Halb-automatische Förder-DB-Aktualisierung (server-seitig,
  wöchentlich, mit menschlichem Freigabe-Schritt im Client).
