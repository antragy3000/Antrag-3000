# Rechercheplan – weitere Förderer für Antrag 3000

Stand: 2026-06-22 · Ziel: den Katalog vom aktuellen Anfangsbestand
(33 Förderer, stark Hessen/Frankfurt, darstellende Künste) zu einem
breiten DACH-Katalog ausbauen.

## 1. Ausgangslage & Lücken

Aktueller Bestand (33):
- **Geografisch:** 14× Hessen, 1× Thüringen, sonst bundesweit. → Fast
  alle anderen Bundesländer fehlen, **AT und CH komplett**.
- **Sparten:** Schwerpunkt darstellende Künste/interdisziplinär; dünn bei
  **Musik, Literatur, Film, bildende Kunst, Medienkunst** als eigene Töpfe.
- **Ebene:** Bund + Land Hessen + kommunal (Frankfurt/Gießen/Wiesbaden)
  gut; **andere Länder/Städte fehlen**.

Daraus die drei Stoßrichtungen: **(a) geografisch verbreitern**,
**(b) spartenspezifische Töpfe ergänzen**, **(c) Ebenen je Region
vervollständigen** (Bund → Land → Stadt → Stiftung).

## 2. Priorisierung (höchster Hebel zuerst)

Reihenfolge nach Reichweite – bundesweite Programme nützen allen
Nutzer:innen, regionale nur den passenden.

| Phase | Fokus | Warum zuerst |
|---|---|---|
| **A** | Bundesweite Programme (DE) | gelten für alle, größter Nutzen |
| **B** | Alle 16 Bundesländer (Landeskultur + Landeskulturstiftung) | schließt die geografische Hauptlücke |
| **C** | Große Städte (Kulturämter) | dichteste Nutzer:innen-Standorte |
| **D** | Überregionale/private Stiftungen, spartenspezifisch | füllt Sparten-Lücken |
| **E** | Österreich + Schweiz, dann EU/international | DACH komplettieren |

## 3. Quellen-Landkarte (konkrete Anlaufstellen)

### Phase A – Bund (DE)
- **Bundeskulturfonds** (die noch fehlenden): Musikfonds, Deutscher
  Literaturfonds, Deutscher Übersetzerfonds, Stiftung Kunstfonds
  (bildende Kunst). *(Fonds Darstellende Künste + Fonds Soziokultur sind
  bereits drin.)*
- **Kulturstiftung des Bundes** (allg. Projektförderung; Jupiter ist drin).
- **BKM** (Beauftragte der Bundesregierung für Kultur und Medien) – Programme.
- **Initiative Musik**, **Deutscher Musikrat** (Projektförderung).
- **Kultur macht stark** (BMBF) – weitere Programmpartner neben „Zirkus macht stark".
- **Filmförderung:** FFA, Deutscher Filmförderfonds (DFFF), Kuratorium junger deutscher Film.

### Phase B – Bundesländer (DE, je Land 1 Kulturförderung + 1 Stiftung)
Pro Land: Kulturministerium/-senat **und** Landeskulturstiftung. Beispiele:
- BW: Kunststiftung Baden-Württemberg · BY: Bayerischer Kulturfonds
- BE: Senatsverwaltung Kultur Berlin · BB: Ministerium f. Kultur BB
- HB: Senator f. Kultur Bremen · HH: Behörde f. Kultur und Medien HH
- MV · NI: Stiftung Niedersachsen · NW: Kunststiftung NRW / Ministerium
- RP · SL · SN: Kulturstiftung des Freistaates Sachsen · ST
- SH · TH (vorhanden: Kulturstiftung Thüringen → Landesprogramme ergänzen)

### Phase C – Städte (Kulturämter, ≥ ~Großstadt)
München, Berlin, Hamburg, Köln, Stuttgart, Leipzig, Dresden, Düsseldorf,
Nürnberg, Hannover, … (je Kulturamt: Projekt-/Spartenförderung, Stipendien).
→ jeweils mit **Stadt als hartem Kriterium** (`durchfuehrungsort_staedte`).

### Phase D – Private/überregionale Stiftungen (spartenspezifisch)
- Allgemein: Kulturstiftung der Länder, Alfred Toepfer Stiftung,
  Robert Bosch Stiftung, ZEIT-Stiftung, Allianz Kulturstiftung,
  Crespo Foundation, Aventis Foundation, Schering Stiftung.
- Musik: GVL, GEMA-Stiftung, Ernst von Siemens Musikstiftung.
- Bildende Kunst: Stiftung Kunstfonds, Schering Stiftung.
- Literatur/Film: s. Bundesebene.

### Phase E – Österreich, Schweiz, EU
- **AT:** BMKÖS (Bundesministerium für Kunst, Kultur), Bundeskanzleramt
  Kunst; Länder (9, je Kulturabteilung); **Stadt Wien (MA 7)**, Linz, Graz,
  Salzburg; SKE-Fonds, KulturKontakt-Nachfolge.
- **CH:** **Pro Helvetia**; Kantone (26, je Kulturförderung/Lotteriefonds);
  Städte (Zürich, Basel, Bern, Genf, Lausanne); **Migros-Kulturprozent**,
  Ernst Göhner Stiftung, Stanley Thomas Johnson Stiftung, SUISA-Stiftung.
- **EU/international:** Creative Europe (Culture), Goethe-Institut.

### Strukturierte Förder-Datenbanken (Recherche-Beschleuniger)
- **foerderdatenbank.de** (Bund/Länder/EU, amtlich) – Filter Kultur.
- **miz.org** (Deutsches Musikinformationszentrum) – Musikförderung.
- **kulturberatung-hessen.de** (Hessen, bereits genutzt).
- Landes-„Förderfinder" der einzelnen Bundesländer.
- **touring-artists.info**, **Creative Europe Desk** (mobil/international).

## 4. Pro-Förderer-Erfassung (Checkliste → Schema)

Für jeden neuen Förderer recherchieren und auf die Felder abbilden:

| Feld | Quelle/Hinweis |
|---|---|
| `name`, `foerdergeber` | offizielle Bezeichnung |
| `land` | DE/AT/CH (Anzeige/Suche) |
| `beschreibung` | 1–2 Sätze, neutral; Quelle nennen |
| `webseite` | offizielle Förderseite |
| `foerderhoehe_text` | kurzer Klartext (z. B. „bis 50.000 €") |
| `fristen` | ISO-Datum der **nächsten** Frist; sonst `zeitpunkt:"laufend"` |
| `weiche_kriterien.sparten` | aus fester Liste (leer = spartenoffen) |
| `weiche_kriterien.projektarten` | aus fester Liste |
| `weiche_kriterien.budget_min/max` | wenn genannt |
| `harte_kriterien.wohnsitz` / `durchfuehrungsort` | Länder-Codes |
| `…_regionen` / `…_staedte` | nur wenn der Geber es **eindeutig** verlangt (sonst weglassen → keine Fehlausschlüsse) |
| `traegerschaft`, `studentisch_erlaubt` | wenn eingeschränkt |
| `checkliste_vorschlag` | typische einzureichende Unterlagen |

**Region-/Stadt-Codes** müssen zu `src/lib/daten/orte.js` passen
(Validierung siehe unten). Fehlt eine Stadt dort, ergänzen.

## 5. Arbeitsweise mit der bestehenden Pipeline

1. **Neue Förderer** als vollständige Einträge sammeln (am besten in einer
   `import/zusatz-foerderer.json` im selben Schema) **oder** bestehende
   per `import/anreicherung.json`-Patch verbessern.
2. **Dedupe** gegen den aktuellen Katalog (Name/id) vor dem Zusammenführen.
3. **Validieren:** Schema (schema_version, id+name), Region-/Stadt-Codes
   gegen `orte.js` (Skript in `import/` vorhanden), Fristen als ISO-Datum.
4. **Report** aktualisieren (offene Angaben je Förderer).
5. **Einspielen:** als neuen App-Standard (`src/lib/daten/foerderungen.json`)
   oder per **Admin-App → Katalog hochladen** verteilen.
6. **Anbindung an den Sammler (Etappe 4):** Die recherchierten Quellen
   eignen sich als Rohquelle für `server sammeln <quelle.json>` – der
   Sammler legt Vorschläge an, die der Admin freigibt.

## 6. Qualitätssicherung & Aktualisierung

- **Verifikation:** jede Angabe gegen die **offizielle Seite** prüfen;
  Best-effort-Recherche kennzeichnen.
- **Fristen sind verderblich:** mind. **jährlich** prüfen (viele Programme
  haben feste Jahres-Zyklen). „Stand" pro Eintrag mitführen.
- **Doppelprogramme:** Große Geber haben mehrere Töpfe (z. B. Projekt- vs.
  Konzeptionsförderung) – als **getrennte** Einträge führen.
- **Realismus:** Pro Batch ~10–20 Förderer; viele Seiten blocken
  automatisierte Abrufe → ggf. WebSearch statt direktem Abruf.

## 7. Grobe Mengenschätzung

- Phase A: ~10–15 · Phase B: ~32 (16 Länder × 2) · Phase C: ~15–25 Städte ·
  Phase D: ~15–25 Stiftungen · Phase E: AT ~15, CH ~20, EU ~3–5.
- Realistischer erster Ausbau auf **~120–150 Förderer** (DACH-Grundstock),
  danach laufend über den Sammler erweitern.
