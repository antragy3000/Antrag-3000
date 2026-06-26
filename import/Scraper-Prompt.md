# Scraper-Prompt – Förderprogramme als Katalog-JSON

> **So benutzt du das:** Kopiere den Block zwischen den Linien in eine neue
> Claude-Unterhaltung und füge darunter **eine oder mehrere URLs** von
> Förder-Webseiten ein (oder den kopierten Seitentext). Claude gibt dir dann
> ein JSON-Array zurück, das direkt in den Katalog von *Antrag 3000* passt.
> Volle Feld-Erklärung bei Bedarf: `Katalog-Schema-fuer-Scraper.md`.

---

````text
ROLLE
Du extrahierst Förderprogramme für Kunst und Kultur (Schwerpunkt DACH:
Deutschland, Österreich, Schweiz, plus dort nutzbare internationale
Förderungen) aus Webseiten und gibst sie als JSON-Array im unten definierten
Schema aus.

GRUNDREGELN (wichtig)
- Erfinde NICHTS. Jeder Wert muss durch die Seite belegt sein.
- Unbekannte Zahl -> null. Unbekannter Text -> "" (leer).
- Eine leere Kriterien-Liste bedeutet "keine Einschränkung". Im Zweifel
  WEGLASSEN statt eine Einschränkung zu erfinden – sonst fällt die Förderung
  im Programm fälschlich durchs Raster.
- Verwende NUR die unten erlaubten Codes (klein/groß exakt wie angegeben).
- Trenne "name" (Name des Programms) von "foerdergeber" (Institution).
- "budget_min/max" = Projektgröße fürs Matching; "foerderhoehe_text" =
  ausgezahlte Summe (nur Anzeige). Nicht verwechseln.
- Setze "recherchiert": true und "geprueft_am" auf das heutige Datum.
- Gib NUR das JSON-Array aus, ohne Erklärtext davor oder danach.

AUSGABE-FORMAT (ein Objekt pro Programm)
{
  "id": "geberkuerzel-programmname",        // slug, siehe ID-REGEL
  "name": "Programmname",
  "foerdergeber": "Institution",
  "land": "DE",                              // DE|AT|CH|INT|ANDERES
  "beschreibung": "1–3 sachliche Sätze: was wird gefördert, für wen.",
  "webseite": "https://… (Direktlink zur Programmseite)",
  "foerderhoehe_text": "z. B. bis 25.000 € oder — wenn unbekannt",
  "max_anteil_prozent": null,                // Zahl (% am Gesamtbudget) oder null
  "anteil_ausnahme": false,                  // true = mehr mit Begründung möglich
  "frist_hinweis": "",                       // z. B. mind. 3 Monate vor Start
  "fristen": [],                             // siehe FRISTEN
  "harte_kriterien": {
    "wohnsitz": ["DE"],                      // Länder; leer = egal
    "durchfuehrungsort": ["DE"],             // Länder; leer = egal
    "traegerschaft": ["einzelperson","gruppe","organisation"], // NIE leer
    "studentisch_erlaubt": true,
    "wohnsitz_regionen": [],                 // Regions-Codes; leer = ganzes Land
    "durchfuehrungsort_regionen": [],
    "wohnsitz_staedte": [],                  // Klartext-Stadtnamen; meist leer
    "durchfuehrungsort_staedte": []
  },
  "weiche_kriterien": {
    "sparten": [],                           // leer = spartenoffen
    "projektarten": [],                      // leer = offen
    "budget_min": null,
    "budget_max": null,
    "waehrung": "EUR",                       // EUR|CHF|…
    "zeitpunkt": "fristen"                   // fristen|laufend|periodisch
  },
  "unvertraeglich_mit": [],
  "checkliste_vorschlag": [],                // typische Unterlagen, z. B. "KFP"
  "recherchiert": true,
  "geprueft_am": "JJJJ-MM-TT"
}

ERLAUBTE CODES
land / wohnsitz / durchfuehrungsort:
  DE=Deutschland  AT=Österreich  CH=Schweiz  INT=International  ANDERES=anderes Land
sparten:
  musik, theater, tanz, performance, bildende_kunst, medienkunst,
  literatur, film, interdisziplinaer
projektarten:
  produktion, recherche_entwicklung, residenz, gastspiel_tournee, festival,
  veroeffentlichung, vermittlung, barrierefreiheit
traegerschaft:
  einzelperson, gruppe, organisation
regionen (NUR passend zum Land verwenden):
  DE: BW BY BE BB HB HH HE MV NI NW RP SL SN ST SH TH
  AT: BGL KTN NOE OOE SBG STMK TIR VBG WIEN
  CH: AG AI AR BE BL BS FR GE GL GR JU LU NE NW OW SG SH SO SZ TG TI UR VD VS ZG ZH
staedte: Klartext-Name als String, exakt geschrieben (z. B. "Wiesbaden",
  "Zürich", "Wien"). Nur wenn die Förderung an eine Stadt gebunden ist.

FRISTEN
- zeitpunkt "laufend": jederzeit einreichbar -> "fristen": []
- zeitpunkt "fristen": feste Termine -> Daten als "JJJJ-MM-TT"
- zeitpunkt "periodisch": wiederkehrend (z. B. halbjährlich) -> Daten OHNE
  Jahr erlaubt: "MM-TT" (z. B. "09-15" = jeder 15. September)
- Ein Frist-Eintrag ist ein String ODER ein Objekt mit Hinweis:
  "fristen": [ "2026-09-15", { "datum": "03-01", "hinweis": "1. Halbjahr" } ]
- Hinweise, die zu KEINEM einzelnen Termin gehören (z. B. "mind. 3 Monate
  vor Start") -> Feld "frist_hinweis", NICHT in "fristen".

ID-REGEL
- klein schreiben; Umlaute: ä->ae ö->oe ü->ue ß->ss
- alles außer a–z 0–9 wird zu "-", mehrfache/Rand-"-" entfernen
- kurzes Geber-Präfix voranstellen, auf max. 50 Zeichen kürzen
- Beispiel: Fonds Darstellende Künste, "Konzeptionsförderung"
  -> "daku-konzeptionsfoerderung"
- id ist der dauerhafte Anker -> eindeutig halten.

VOR DER AUSGABE PRÜFEN
- Pflichtfelder vorhanden; nur erlaubte Codes; traegerschaft nicht leer;
  zeitpunkt passt zu fristen; Datumsformat korrekt; Zahlen sind Zahlen
  (80, nicht "80%"); webseite ist https-Direktlink; nichts erfunden;
  gültiges JSON mit echten Umlauten (UTF-8); NUR das JSON-Array ausgeben.

Hier ist/sind die Webseite(n):
[URL ODER SEITENTEXT EINFÜGEN]
````

---

## Varianten

- **Mehrere Programme einer Übersichtsseite:** „Gib für *jedes* einzelne
  Programm auf dieser Seite ein eigenes Objekt aus."
- **Bestehenden Eintrag aktualisieren:** Hänge das alte JSON an und schreibe:
  „Aktualisiere nur belegbare Änderungen, behalte die vorhandene `id`."
- **Nur prüfen statt extrahieren:** „Stimmen Fristen und Förderhöhe in diesem
  Eintrag noch mit der Webseite überein? Nenne nur Abweichungen."
