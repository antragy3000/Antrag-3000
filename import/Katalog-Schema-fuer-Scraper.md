# Förder-Katalog – Schema für einen Scraper

> **Zweck.** Dieses Dokument beschreibt vollständig, wie ein Eintrag im
> Förder-Katalog von *Antrag 3000* aussieht. Gib es einem Scraper (z. B.
> einem Claude-Assistenten) als Anleitung mit: aus einer Förder-Webseite
> soll **gültiges JSON in genau diesem Format** entstehen, das ohne
> Nacharbeit in den Katalog übernommen werden kann.
>
> **Quelle der Wahrheit ist der Code**, nicht dieses Dokument. Erlaubte
> Werte stammen aus `src/lib/begriffe.js` (Sparten, Projektarten, Träger,
> Länder), `src/lib/daten/orte.js` (Regionen/Städte) und der
> Matching-Logik in `src/lib/matching.js`. Stand: Schema-Version **1**.

---

## 0. Wichtigste Regel für den Scraper

**Nichts erfinden.** Jeder Wert muss durch die Webseite belegt sein. Wenn
eine Information fehlt, gilt:

- Text-Felder → leerer String `""` oder ein kurzer Hinweis, **nicht raten**.
- Zahlen (Budget, Anteil) → `null`.
- Kriterien-Listen (Sparten, Regionen, …) → **eher weit fassen**. Eine leere
  Liste bedeutet „keine Einschränkung". Lieber eine Einschränkung weglassen,
  als eine zu erfinden, die nicht dasteht – sonst fällt die Förderung im
  Programm fälschlich unter „passt nicht".

Bei Unsicherheit: in `beschreibung` oder `frist_hinweis` als Klartext
notieren („genaue Frist siehe Webseite") statt strukturierte Felder zu raten.

---

## 1. Datei-Aufbau (oberste Ebene)

Der Katalog ist **eine JSON-Datei** mit diesem Rahmen:

```json
{
  "schema_version": 1,
  "stand": "2026-06-24",
  "hinweis": "Recherchierte Förderdaten. Förderhöhen, Fristen und Voraussetzungen können sich ändern – vor jeder Antragstellung auf der Webseite prüfen.",
  "foerderungen": [ { …Eintrag… }, { …Eintrag… } ]
}
```

| Feld | Typ | Bedeutung |
|---|---|---|
| `schema_version` | Zahl | Immer `1`. Der Server prüft das beim Hochladen. |
| `stand` | String `JJJJ-MM-TT` | Datum des Datenstands. |
| `hinweis` | String | Allgemeiner Haftungs-/Aktualitätshinweis. |
| `foerderungen` | Array | Die Liste der Förder-Einträge (siehe §2). |

**Für einen Scraper, der nur sammelt:** Es reicht, ein **Array von
Einträgen** zu liefern (nur der Inhalt von `foerderungen`). Der Rahmen wird
beim Zusammenführen in der Admin-App ergänzt.

---

## 2. Ein Förder-Eintrag – alle Felder

Vollständiges, kommentiertes Beispiel (alle Felder belegt):

```json
{
  "id": "daku-konzeptionsfoerderung",
  "name": "Konzeptionsförderung",
  "foerdergeber": "Fonds Darstellende Künste",
  "land": "DE",
  "beschreibung": "Dreijährige Konzeption mit drei Neuproduktionen … für professionelle Künstler:innen der Freien Darstellenden Künste mit Sitz in Deutschland.",
  "webseite": "https://www.fonds-daku.de/foerderung/foerderprogramme/konzeptionsfoerderung/",
  "foerderhoehe_text": "150.000 – 240.000 €",
  "max_anteil_prozent": null,
  "anteil_ausnahme": false,
  "frist_hinweis": "Förderzeitraum vsl. 01.04.2026 – 15.10.2028. Genaue Einreichfrist siehe Webseite.",
  "fristen": [],
  "harte_kriterien": {
    "wohnsitz": ["DE"],
    "durchfuehrungsort": ["DE"],
    "traegerschaft": ["einzelperson", "gruppe", "organisation"],
    "studentisch_erlaubt": true,
    "wohnsitz_regionen": [],
    "durchfuehrungsort_regionen": [],
    "wohnsitz_staedte": [],
    "durchfuehrungsort_staedte": []
  },
  "weiche_kriterien": {
    "sparten": ["theater", "tanz", "performance", "interdisziplinaer"],
    "projektarten": ["produktion", "recherche_entwicklung"],
    "budget_min": 150000,
    "budget_max": 240000,
    "waehrung": "EUR",
    "zeitpunkt": "fristen"
  },
  "unvertraeglich_mit": [],
  "checkliste_vorschlag": ["Konzeptpapier", "Kosten- und Finanzierungsplan", "Arbeitsbiografie"],
  "recherchiert": true,
  "geprueft_am": "2026-06-24"
}
```

### 2.1 Stamm-Felder

| Feld | Pflicht | Typ | Bedeutung & Scraper-Hinweise |
|---|---|---|---|
| `id` | ja | String | Eindeutiger Schlüssel, klein, nur `a–z 0–9 -`. Bildung siehe §5. |
| `name` | ja | String | Name **des Programms** (nicht des Gebers), z. B. „Konzeptionsförderung". |
| `foerdergeber` | ja | String | Die fördernde Institution, z. B. „Fonds Darstellende Künste". |
| `land` | ja | Enum | Hauptland, siehe §3.1. |
| `beschreibung` | ja | String | 1–3 Sätze, sachlich, was gefördert wird und für wen. Keine Werbung. |
| `webseite` | ja | URL | Direktlink zur Programm-Seite (nicht nur die Startseite). |
| `foerderhoehe_text` | ja | String | Frei lesbar, z. B. „bis 25.000 €", „150.000 – 240.000 €". Wenn unbekannt: `"—"`. |
| `max_anteil_prozent` | nein | Zahl/`null` | Max. Anteil am **Gesamtbudget** in Prozent (z. B. `80`). Sonst `null`. |
| `anteil_ausnahme` | nein | Bool | `true`, wenn „höherer Anteil mit Begründung möglich". Sonst `false`. |
| `frist_hinweis` | nein | String | Allgemeiner Fristhinweis, z. B. „mind. 3 Monate vor Projektstart". |
| `fristen` | ja | Array | Einreichfristen, siehe §4. Leer `[]`, wenn laufend / unbekannt. |
| `unvertraeglich_mit` | ja | Array<String> | `id`s anderer Förderungen, die sich gegenseitig ausschließen. Meist `[]`. |
| `checkliste_vorschlag` | ja | Array<String> | Typische Antrags-Unterlagen, z. B. „KFP", „Projektbeschreibung". |
| `recherchiert` | nein | Bool | `true` = aus Recherche, nicht selbst eingetragen. Für Scraper: `true`. |
| `geprueft_am` | nein | String `JJJJ-MM-TT` | Datum der letzten manuellen Prüfung. Scraper: heutiges Datum setzen. |

> **`name` vs. `foerdergeber`:** Wenn ein Geber nur ein einziges Programm
> hat, dürfen beide ähnlich klingen. Trotzdem beide füllen.

### 2.2 `harte_kriterien` – Ausschluss-Kriterien

Diese entscheiden **ob** eine Förderung grundsätzlich passt. Erfüllt ein:e
Nutzer:in sie nicht, landet die Förderung unter „weitere Vorschläge" mit
Hinweis. **Faustregel: leere Liste = keine Einschränkung.**

| Feld | Typ | Bedeutung |
|---|---|---|
| `wohnsitz` | Array<Land> | In welchem Land muss der/die Antragstellende wohnen? Siehe §3.1. Leer = egal. |
| `durchfuehrungsort` | Array<Land> | Wo muss das Projekt stattfinden? Leer = egal. |
| `traegerschaft` | Array<Träger> | Wer darf beantragen? Siehe §3.4. **Sollte nie leer sein** – sonst passt niemand. |
| `studentisch_erlaubt` | Bool | Dürfen studentische Projekte beantragen? Standard `true`. |
| `wohnsitz_regionen` | Array<Regions-Code> | Engt **innerhalb des Landes** ein (Bundesland/Kanton). Siehe §3.5. Leer = ganzes Land. |
| `durchfuehrungsort_regionen` | Array<Regions-Code> | Wie oben, für den Durchführungsort. |
| `wohnsitz_staedte` | Array<Stadtname> | Noch enger: nur diese Städte. Siehe §3.6. Meist leer. |
| `durchfuehrungsort_staedte` | Array<Stadtname> | Wie oben, für den Durchführungsort. |

> Die Regionen-/Städte-Felder sind **optional**. Nur setzen, wenn die
> Förderung wirklich regional/kommunal gebunden ist (z. B. „Stadt
> Wiesbaden", „Kulturland Brandenburg"). Eine Bundesförderung lässt sie leer.

### 2.3 `weiche_kriterien` – Rangfolge-Kriterien

Diese schließen **nicht aus**, sondern beeinflussen die **Reihenfolge** der
Treffer. Leere Sparten/Projektarten = „offen für alles" (zählt etwas
schwächer als ein echter Treffer).

| Feld | Typ | Bedeutung |
|---|---|---|
| `sparten` | Array<Sparte> | Geförderte Sparten, siehe §3.2. Leer = spartenoffen. |
| `projektarten` | Array<Projektart> | Geförderte Projektarten, siehe §3.3. Leer = offen. |
| `budget_min` | Zahl/`null` | Untere Budget-Grenze des Projekts (in Währungseinheiten). |
| `budget_max` | Zahl/`null` | Obere Budget-Grenze. `null` = nach oben offen. |
| `waehrung` | Enum | `"EUR"`, `"CHF"` (oder anderes ISO-Kürzel). Standard `"EUR"`. |
| `zeitpunkt` | Enum | `"fristen"`, `"laufend"` oder `"periodisch"`. Siehe §4. |

> **Budget vs. Förderhöhe:** `budget_min/max` beschreiben die **Projektgröße**,
> für die die Förderung gedacht ist (für das Matching). `foerderhoehe_text`
> ist die ausgezahlte **Fördersumme** (nur Anzeige). Nicht verwechseln. Wenn
> die Webseite nur die Fördersumme nennt, `budget_min/max` ruhig `null` lassen.

---

## 3. Erlaubte Werte (Enums)

**Nur diese Codes verwenden.** Großschreibung exakt wie angegeben.

### 3.1 `land`, `wohnsitz[]`, `durchfuehrungsort[]`

| Code | Bedeutung |
|---|---|
| `DE` | Deutschland |
| `AT` | Österreich |
| `CH` | Schweiz |
| `INT` | International |
| `ANDERES` | anderes Land |

### 3.2 `sparten[]`

| Code | Bedeutung |
|---|---|
| `musik` | Musik |
| `theater` | Theater |
| `tanz` | Tanz |
| `performance` | Performance |
| `bildende_kunst` | Bildende Kunst |
| `medienkunst` | Medienkunst |
| `literatur` | Literatur |
| `film` | Film |
| `interdisziplinaer` | Interdisziplinär |

### 3.3 `projektarten[]`

| Code | Bedeutung |
|---|---|
| `produktion` | Produktion |
| `recherche_entwicklung` | Recherche & Entwicklung |
| `residenz` | Residenz |
| `gastspiel_tournee` | Gastspiel / Tournee |
| `festival` | Festival |
| `veroeffentlichung` | Veröffentlichung |
| `vermittlung` | Vermittlung |
| `barrierefreiheit` | Förderung für Barrierefreiheit |

### 3.4 `traegerschaft[]`

| Code | Bedeutung |
|---|---|
| `einzelperson` | Einzelperson |
| `gruppe` | Gruppe / GbR |
| `organisation` | Verein / Organisation |

### 3.5 Regionen-Codes (`*_regionen[]`)

Nur gültig **passend zum Land**. (DE = Bundesländer, AT = Bundesländer,
CH = Kantone.)

**DE:** `BW` Baden-Württemberg · `BY` Bayern · `BE` Berlin · `BB` Brandenburg ·
`HB` Bremen · `HH` Hamburg · `HE` Hessen · `MV` Mecklenburg-Vorpommern ·
`NI` Niedersachsen · `NW` Nordrhein-Westfalen · `RP` Rheinland-Pfalz ·
`SL` Saarland · `SN` Sachsen · `ST` Sachsen-Anhalt · `SH` Schleswig-Holstein ·
`TH` Thüringen

**AT:** `BGL` Burgenland · `KTN` Kärnten · `NOE` Niederösterreich ·
`OOE` Oberösterreich · `SBG` Salzburg · `STMK` Steiermark · `TIR` Tirol ·
`VBG` Vorarlberg · `WIEN` Wien

**CH:** `AG` Aargau · `AI` Appenzell Innerrhoden · `AR` Appenzell Ausserrhoden ·
`BE` Bern · `BL` Basel-Landschaft · `BS` Basel-Stadt · `FR` Freiburg ·
`GE` Genf · `GL` Glarus · `GR` Graubünden · `JU` Jura · `LU` Luzern ·
`NE` Neuenburg · `NW` Nidwalden · `OW` Obwalden · `SG` St. Gallen ·
`SH` Schaffhausen · `SO` Solothurn · `SZ` Schwyz · `TG` Thurgau · `TI` Tessin ·
`UR` Uri · `VD` Waadt · `VS` Wallis · `ZG` Zug · `ZH` Zürich

### 3.6 Städte (`*_staedte[]`)

**Klartext-Stadtname als String**, exakt geschrieben, z. B. `"Wiesbaden"`,
`"Zürich"`, `"Wien"`. (Die App kennt eine kuratierte Städteliste; gängige
Städte ab ~50.000 Einwohner sind enthalten. Schreibweise wie im Duden/amtlich.)
Nur setzen, wenn die Förderung wirklich an eine Stadt gebunden ist.

---

## 4. Fristen (`fristen[]` + `zeitpunkt`)

`zeitpunkt` (in `weiche_kriterien`) bestimmt die Art, `fristen` liefert die
konkreten Termine:

| `zeitpunkt` | Bedeutung | `fristen` |
|---|---|---|
| `"laufend"` | Jederzeit einreichbar. | Leer `[]`. |
| `"fristen"` | Feste Einreichtermine. | Konkrete Daten (siehe unten). |
| `"periodisch"` | Regelmäßig wiederkehrend (z. B. halbjährlich). | Daten **ohne Jahr** möglich. |

**Ein Frist-Eintrag** ist entweder ein einfacher String **oder** ein Objekt
mit optionalem Hinweis:

```json
"fristen": [
  "2026-09-15",
  { "datum": "2027-03-01", "hinweis": "für das zweite Halbjahr" },
  "09-15"
]
```

- **`"JJJJ-MM-TT"`** – konkretes Datum (z. B. `"2026-09-15"`).
- **`"MM-TT"`** – wiederkehrendes Datum **ohne Jahr** (z. B. `"09-15"` =
  jeder 15. September). **Nur bei `zeitpunkt: "periodisch"`** verwenden.
- **Objekt `{ "datum": …, "hinweis": … }`** – wenn ein Termin einen
  Zusatztext braucht („für das erste Halbjahr").

Allgemeine Fristhinweise, die zu **keinem** einzelnen Termin gehören (z. B.
„mind. 3 Monate vor Projektstart", auch bei `laufend` sinnvoll), gehören in
das Feld **`frist_hinweis`** (§2.1), nicht in `fristen`.

---

## 5. `id` bilden

Stabiler, eindeutiger Schlüssel. Regel (entspricht der App):

1. Quell-Präfix wählen (z. B. Kürzel des Gebers): `daku-`, `bkm-`, `import-` …
2. `name` (ggf. + Geber) klein schreiben.
3. Umlaute ersetzen: `ä→ae`, `ö→oe`, `ü→ue`, `ß→ss`.
4. Alles außer `a–z 0–9` zu `-`; mehrfache/umschließende `-` entfernen.
5. Auf **max. 50 Zeichen** kürzen.
6. Bei Kollision `-2`, `-3`, … anhängen.

Beispiel: „Konzeptionsförderung" beim Fonds Darstellende Künste →
`daku-konzeptionsfoerderung`.

> **Wichtig:** `id` ist der dauerhafte Anker (Update-Erkennung, Merklisten,
> `unvertraeglich_mit`). Einmal vergeben, **nicht mehr ändern**.

---

## 6. Qualitäts-Checkliste für den Scraper

Vor der Ausgabe jedes Eintrags prüfen:

- [ ] Alle **Pflichtfelder** aus §2.1 vorhanden.
- [ ] `land`, `sparten`, `projektarten`, `traegerschaft`, Regionen nutzen
      **nur** erlaubte Codes (§3). Keine Klartext-Namen statt Codes.
- [ ] `traegerschaft` ist **nicht leer**.
- [ ] `zeitpunkt` passt zu `fristen` (laufend → leer; periodisch → ggf. ohne Jahr).
- [ ] Datums-Strings haben das Format `JJJJ-MM-TT` bzw. `MM-TT`.
- [ ] Zahlen sind echte Zahlen (`80`, nicht `"80%"`); Unbekanntes ist `null`.
- [ ] `webseite` ist ein vollständiger `https://`-Direktlink.
- [ ] Nichts erfunden – jede Einschränkung steht so auf der Webseite.
- [ ] `id` eindeutig und ≤ 50 Zeichen.
- [ ] JSON ist gültig (Kommas, Anführungszeichen, UTF-8 mit echten Umlauten).

---

## 7. Fertiger Prompt-Baustein (zum Kopieren)

> Du extrahierst Förderprogramme für Kunst & Kultur (DACH) aus Webseiten und
> gibst sie als JSON-Array im unten beschriebenen Schema aus. **Erfinde
> nichts** – nur belegbare Angaben. Unbekannte Zahlen = `null`, unbekannte
> Texte = `""`. Verwende ausschließlich die erlaubten Codes für `land`,
> `sparten`, `projektarten`, `traegerschaft` und Regionen. Leere
> Kriterien-Liste bedeutet „keine Einschränkung" – im Zweifel weglassen, nicht
> erfinden. Setze `recherchiert: true` und `geprueft_am` auf das heutige
> Datum. Bilde `id` als klein-geschriebenen Slug (Umlaute ae/oe/ue/ss,
> Sonderzeichen zu `-`, ≤ 50 Zeichen) mit einem kurzen Geber-Präfix. Gib **nur**
> das JSON-Array aus, ohne Erklärtext. [Danach §2–§5 dieses Dokuments anhängen.]

---

## 8. Daten in den Katalog übernehmen

1. Scraper-Ausgabe als JSON-Array sammeln (eine Datei, z. B. wie
   `import/fonds-daku.json`).
2. In der **Admin-App** den aktuellen Katalog laden, neue Einträge anhängen
   (oder per Import-Skript zusammenführen), auf doppelte `id`s prüfen.
3. Katalog **hochladen** – der Server prüft Schema-Version und dass jede
   Förderung `id` und `name` hat, und verteilt die neue Fassung an alle Geräte.

Referenz-Beispiel mit echten Einträgen: **`import/fonds-daku.json`**.
