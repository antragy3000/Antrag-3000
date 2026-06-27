# Abrechnungs-Modus – Spezifikation (Stand 2026-06-28)

> Entstanden aus dem Fragebogen (`Abrechnung-Fragebogen.md`). Hält fest, **was**
> gebaut wird, **wie** die Daten aussehen und in **welchen kleinen Schritten**
> wir vorgehen. Noch wird nicht programmiert – das ist die Bau-Grundlage.

---

## 1. Worum es geht

Ein neuer, eigener **Hauptbereich „Abrechnung"** neben Recherche/Antrag. Damit
kann man **laufend Belege erfassen** (schon während des Projekts) und später für
den **Verwendungsnachweis** jedem Förderer die passenden (Teil-)Beträge
zuordnen – so lange „verschoben", bis die Rechnung aufgeht.

Grundlage ist der bestehende **Kostenfinanzplan (KFP)**: seine Kosten-Posten
sind die **Kostenstellen**, seine Finanzierungs-Posten liefern die
**Soll-Beträge** der Förderer.

---

## 2. Getroffene Entscheidungen (aus dem Fragebogen)

| Thema | Entscheidung |
|---|---|
| **Ergebnis** | Pro Förderer: Belegliste + Kostenübersicht **plus Sachbericht-Text**. |
| **Beleg-Dateien** | Foto/Scan/PDF **verschlüsselt im Projektordner**, nur Verweis im Tresor. |
| **Kostenstelle** | Verknüpfung mit einem **einzelnen KFP-Kosten-Posten** (z. B. „1.2"). |
| **Beleg → Förderer** | **Direkte** Zuordnung, **anteilig** auf mehrere Quellen aufteilbar; zentrale **Verteil-Ansicht** zum „Verschieben, bis es aufgeht". |
| **Doppelförderung** | Programm **warnt** und zeigt je Beleg den noch freien Restbetrag. |
| **Soll je Förderer** | Vorschlag aus dem KFP-Finanzierungsplan, mit dem **real bewilligten Betrag überschreibbar**. |
| **Förderart** | Keine Automatik – Beträge werden **frei** verteilt. |
| **Betrag** | **Brutto** + optionaler **MwSt-Satz**, Netto wird berechnet. |
| **Beleg-Status** | Schlanke Kette: *erfasst → zugeordnet → abgerechnet*. |
| **Eigenmittel/Einnahmen** | Werden **wie eine Geldquelle** geführt (zählen beim Ausgleich mit). |
| **Beleg-Split auf mehrere Kostenstellen** | Vorerst **eine** pro Beleg (Architektur lässt Split später zu). |
| **Plan/Ist** | **Je Kostenstelle** anzeigen (geplant vs. tatsächlich vs. Rest). |
| **Warnungen** | Beleg überzogen · Förderer überzogen · Kostenstelle über Plan · Belege ohne Zuordnung. |
| **Export** | **PDF** als Standard, **Word** als Option. (Excel später möglich.) |
| **Verortung** | Eigener Hauptbereich „Abrechnung". |

### Noch zu bestätigen (meine Annahmen – bitte kurz nicken oder korrigieren)
- **A2** Belege/Abrechnung hängen immer an **einem Projekt**. ✔ angenommen
- **A3** Vorerst **eine Person** pflegt Belege (kein Team-Sync der Belege – sie
  sind sensibel und blieben lokal). ✔ angenommen
- **G1** Alles bleibt **lokal verschlüsselt**, nichts geht je ins Netz / auf die
  NAS. ✔ feste Regel
- **B1** Beleg-Felder (siehe §4) ok?
- **C2** Kostenstelle ist beim Erfassen **optional** und ein neuer KFP-Posten
  lässt sich direkt aus der Beleg-Maske anlegen. ✔ angenommen
- **B6** Mehrere Dateien je Beleg erlaubt. ✔ angenommen

---

## 3. Die zentrale Idee: die „Verteil-Ansicht"

Das Herzstück. Eine Tabelle/Matrix:

- **Zeilen = Belege** (mit Datum, Empfänger, Betrag, Kostenstelle).
- **Spalten = Geldquellen** (jeder Förderer + „Eigenmittel/Einnahmen").
- In den Zellen stehen die **zugeordneten Teilbeträge**.

Live mitlaufende Summen sorgen dafür, dass man sieht, ob „es aufgeht":
- **Je Beleg:** Betrag − Summe der Zuordnungen = **noch frei** (darf nicht
  negativ werden → Warnung Doppelförderung).
- **Je Förderer:** zugeordnet vs. **Soll** (bewilligt) → Rest oder Überzug.
- **Gesamt:** Kosten gedeckt? Förderbeträge ausgeschöpft? Was bleibt Eigenmittel?

So „verschiebt" man Beträge zwischen den Quellen, bis jeder Förderer gedeckt und
das Projekt ausgeglichen ist.

---

## 4. Datenmodell (Vorschlag)

Pro Projekt kommt ein neuer Block `abrechnung` in den Tresor (alles sensibel,
lokal verschlüsselt):

```
abrechnung: {
  belege: [
    {
      id,                 // eindeutige Kennung
      nr,                 // laufende Beleg-Nummer (automatisch)
      datum,              // "JJJJ-MM-TT"
      empfaenger,         // an wen gezahlt (Lieferant)
      zweck,              // kurze Beschreibung
      brutto,             // Betrag (Zahl)
      mwst_satz,          // optional, z. B. 19 / 7 / null
      zahlungsart,        // "bar" | "karte" | "ueberweisung" | null
      kostenstelle,       // Verweis auf einen KFP-Kosten-Posten (optional)
      status,             // "erfasst" | "zugeordnet" | "abgerechnet"
      dateien: [ { name, ref } ],   // verschlüsselte Beleg-Dateien (Verweise)
      zuordnungen: [ { quelleId, betrag } ],  // anteilig auf Geldquellen
      notiz
    }
  ],
  quellen: [
    {
      id,
      typ,                // "foerderung" | "eigenmittel"
      foerderId,          // bei Förderung: Verweis auf die Förderung (sonst null)
      name,               // Anzeigename (bei Förderung aus Katalog)
      soll,               // bewilligter Betrag (Vorschlag aus KFP, überschreibbar)
      sachbericht         // Fließtext je Förderer (für den Verwendungsnachweis)
    }
  ]
}
```

**Verknüpfungen zum bestehenden System:**
- `kostenstelle` zeigt auf einen Posten der KFP-`kosten`-Seite. **Plan** =
  Posten-Betrag aus dem KFP, **Ist** = Summe der `brutto` aller Belege darauf.
- `quellen` werden aus der KFP-`finanzierung`-Seite vorbefüllt (Positionen mit
  `foerderId`) plus eine Eigenmittel-Quelle; `soll` ist überschreibbar.
- `zuordnungen[].betrag` summiert pro Beleg ≤ `brutto`; pro Quelle aufsummiert =
  „Ist" gegen `soll`.

> **Tresor-Migration:** ältere Tresore bekommen `abrechnung` automatisch leer
> ergänzt (wie bei früheren Feld-Erweiterungen). Der **Wächter-Test** wird
> erweitert, damit garantiert nichts davon in die Sync-Ebene gelangt.

---

## 5. Beleg-Dateien (verschlüsselt im Projektordner)

- Beim Hinzufügen wird die Datei **kopiert, verschlüsselt** und im
  Projektordner abgelegt; im Tresor steht nur ein **Verweis** (Dateiname/Schlüssel).
- Ver-/Entschlüsselung übernimmt das **Rust-Backend** (gleiche Bausteine wie der
  Tresor: AES-GCM). Grund: Krypto und Dateizugriff gehören laut Projektregeln
  ins kleine Rust-Backend, nicht ins Frontend.
- Anzeige/Export entschlüsselt nur bei Bedarf in den Arbeitsspeicher.

---

## 6. Export: Verwendungsnachweis

Pro Förderer ein Dokument:
1. **Kopf** (Projekt, Förderer, Zeitraum, bewilligter Betrag).
2. **Sachbericht** (Fließtext aus `quellen[].sachbericht`).
3. **Belegliste** – Spalten: lfd. Nr. · Datum · Empfänger · Zweck ·
   Kostenstelle · Belegsumme · **diesem Förderer zugeordneter Betrag**; Summe.
4. **Kostenübersicht** (Plan/Ist je Kostenstelle, soweit diesem Förderer
   zugeordnet).
- Format: **PDF** (Standard), **Word** optional. (Nutzt die vorhandene
  PDF-/Word-Erzeugung.)
- Optional später: **alle Belege eines Förderers als ein PDF** anhängen (wie
  heute die Antrags-Anhänge).

---

## 7. Phasenplan (kleine Schritte, je testbar)

> Immer nur die aktuelle Phase bauen; nach jedem Schritt zeige ich dir, wie du
> es selbst testest, und schlage einen Commit vor.

- **A1 – Belege erfassen (ohne Dateien).** Datenmodell + neuer Bereich
  „Abrechnung" + Beleg-Liste + Erfassungs-Maske (Datum, Betrag/MwSt, Empfänger,
  Zweck, Zahlungsart, Status). Tresor-Migration + Wächter-Test.
  *Test: Beleg anlegen, speichern, nach Neustart noch da.*
- **A2 – Beleg-Dateien.** Foto/Scan/PDF verschlüsselt im Projektordner
  ablegen, anzeigen, löschen (Rust-Krypto).
  *Test: Foto anhängen, schließen, wieder öffnen, Datei lesbar.*
- **A3 – Kostenstellen + Plan/Ist.** Beleg mit KFP-Posten verknüpfen (optional,
  neu anlegbar); je Kostenstelle Plan/Ist/Rest anzeigen.
  *Test: zwei Belege auf „1.2", Summe stimmt mit KFP-Plan vergleichen.*
- **A4 – Geldquellen + Verteil-Ansicht.** Quellen aus KFP vorbefüllen, Soll
  überschreibbar; die Matrix zum anteiligen Zuordnen mit Live-Summen und allen
  vier Warnungen.
  *Test: 1.000-€-Beleg auf zwei Förderer aufteilen, Überzug erzeugt Warnung.*
- **A5 – Verwendungsnachweis-Export.** Pro Förderer PDF (optional Word) mit
  Sachbericht + Belegliste + Kostenübersicht.
  *Test: PDF erzeugen, Zahlen stimmen mit der Verteil-Ansicht überein.*

Spätere Ausbaustufen (bewusst zurückgestellt): Beleg-Split auf mehrere
Kostenstellen, Fremdwährung, Team-Sync der Belege, Excel-Export, Storno/
Gutschriften, eigener Belegtyp für Honorare, Verwendungsnachweis-Fristen im
Kalender, Rückzahlungs-Logik.

---

## 8. Unverhandelbar (zur Erinnerung)
- Belege und alle Beträge bleiben **lokal verschlüsselt**; nichts geht ins Netz.
- Keine selbstgebaute Krypto; Datei-/Krypto-Arbeit im Rust-Backend.
- Kleine Schritte, nach jedem Schritt testbar + Commit-Vorschlag.
