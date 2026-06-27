# Abrechnungs-Modus – Fragebogen zur Funktionsweise

> **Wozu dieses Dokument?** Bevor wir den neuen, großen **Abrechnungs-Modus**
> bauen, halten wir gemeinsam fest, *wie* er funktionieren soll – genau wie wir
> am Anfang das ganze Projekt in `CLAUDE.md` geklärt haben. Du musst nichts
> programmieren verstehen. Zu jeder Frage gibt es **Optionen** und eine
> **Empfehlung**. Antworte einfach darunter (z. B. „Empfehlung passt" oder eine
> eigene Antwort). Aus deinen Antworten mache ich danach eine klare
> Bau-Spezifikation.
>
> **Du musst nicht alles auf einmal beantworten.** Fragen mit 🟢 sollten wir
> jetzt klären (sie bestimmen die Grundstruktur). 🟡 = wichtig, aber kann auch
> in der zweiten Runde kommen. ⚪ = Detail, später.

---

## Was schon klar ist (deine Vorgaben + bestehende Technik)

- Es wird ein **eigener Modus** der Anwendung („Abrechnung"), neben dem
  bisherigen Recherchieren/Beantragen.
- Man kann **laufend Belege hinterlegen** – also schon während des Projekts,
  bevor feststeht, welcher Förderer welchen Beleg bekommt.
- Belege werden später **einem Förderer zugeordnet** (= die Abrechnung).
- Verknüpfung läuft über eine **Kostenstelle im KFP**.

**Bestehende Bausteine, an die wir anknüpfen:**
- Der **Kostenfinanzplan (KFP)** existiert schon. Er hat zwei Seiten:
  *Kosten* (Kategorien mit nummerierten Posten, z. B. „1.2 Honorar Regie") und
  *Finanzierung* (Posten, die teils mit einer Förderung verknüpft sind). Ein
  Kosten-Posten ist faktisch deine **Kostenstelle**.
- Der **KFP ist der Plan** (was geplant ist). **Belege sind das Ist** (was
  wirklich ausgegeben wurde). Die Abrechnung vergleicht beides.
- Alles Sensible (Beträge, Lieferanten, Beleg-Fotos) liegt **lokal
  verschlüsselt im Tresor** und verlässt das Gerät nie – das gilt
  selbstverständlich auch für die ganze Abrechnung (unverhandelbar).

---

## A. Ziel & Rahmen

**A1 🟢 Was ist das Endergebnis der Abrechnung?**
In der DACH-Förderung heißt das meist **Verwendungsnachweis**: pro Förderer ein
Nachweis, wofür das Geld ausgegeben wurde – oft als **Belegliste**
(nummerierte Tabelle) plus ggf. ein kurzer Sachbericht.
- (a) Pro Förderer eine **Belegliste + Kostenübersicht** (Zahlen), die du
  exportierst.
- (b) Zusätzlich ein **Sachbericht-Textteil** (Fließtext) im selben Dokument.
- (c) Nur eine interne Übersicht für dich, kein Export.
- **Empfehlung:** (a) als Kern, (b) später optional. → _Deine Antwort:_

**A2 🟢 Bezieht sich die Abrechnung immer auf EIN Projekt?**
(Wie der KFP heute auch pro Projekt geführt wird.)
- **Empfehlung:** Ja, Belege und Abrechnung hängen immer an einem Projekt.
  → _Deine Antwort:_

**A3 🟡 Arbeiten mehrere Personen (1–5) an denselben Belegen?**
Heute ist der Tresor pro Gerät. Belege sind sensibel und bleiben lokal.
- (a) Nur du pflegst Belege.
- (b) Mehrere – dann müssten Belege über die Team-Ebene synchronisiert werden
  (Achtung: die ist bewusst „unkritisch"; Belege sind aber sensibel → das wäre
  ein größerer Architektur-Schritt).
- **Empfehlung:** (a) für den Anfang; Team-Belege als spätere Phase.
  → _Deine Antwort:_

---

## B. Beleg erfassen („laufend hinterlegen")

**B1 🟢 Welche Angaben hat ein Beleg?** (Vorschlag – bitte streichen/ergänzen.)
- **Datum** der Ausgabe
- **Betrag** (siehe B2 zu Brutto/MwSt)
- **Empfänger/Lieferant** (an wen gezahlt)
- **Zweck/Beschreibung** (kurz, z. B. „Mietscheinwerfer Premiere")
- **Beleg-Nr.** (eigene laufende Nummer – automatisch?)
- **Zahlungsart** (bar / Karte / Überweisung)
- **Kostenstelle** (Verknüpfung zum KFP-Kosten-Posten, siehe Abschnitt C)
- **Beleg-Datei** (Foto/Scan/PDF, siehe B4)
- **Status** (siehe B5)
- → _Deine Antwort (Felder ok? Etwas weglassen/ergänzen?):_

**B2 🟢 Wie genau beim Betrag – Brutto/Netto/MwSt?**
Manche Förderer verlangen Netto (wenn du vorsteuerabzugsberechtigt bist),
andere Brutto.
- (a) Nur **ein Betrag** (Brutto), einfach.
- (b) **Brutto + MwSt-Satz** erfassen, Netto wird berechnet (für beide Welten).
- **Empfehlung:** (b), weil Förderer das oft unterschiedlich wollen – aber als
  optionales Feld, damit es einfach bleibt. → _Deine Antwort:_

**B3 🟡 Fremdwährung?** (Internationale Förderungen sind ja Teil des Katalogs.)
- (a) Nur EUR/CHF, ein Feld Währung.
- (b) Fremdwährung + Umrechnung zu Projektwährung (Kurs erfassen).
- **Empfehlung:** (a) zuerst; (b) später bei Bedarf. → _Deine Antwort:_

**B4 🟢 Beleg-Datei (Foto/Scan): wie speichern?**
Belege sind oft Fotos – viele davon werden groß.
- (a) Datei **in den Tresor** legen (verschlüsselt wie das Logo). Sicher, aber
  der Tresor wächst stark (hunderte Fotos).
- (b) Dateien **verschlüsselt als separate Dateien** neben dem Tresor (im
  Projektordner), nur Verweis im Tresor. Skaliert besser für viele/große Fotos.
- (c) Nur **Verweis auf eine Datei** auf der Festplatte (keine Kopie). Am
  schlanksten, aber wenn du die Datei verschiebst, ist der Beleg „weg".
- **Empfehlung:** (b) – verschlüsselte Beleg-Dateien im Projektordner, Verweis
  im Tresor. (Technisch etwas mehr Aufwand, aber sauber und sicher.)
  → _Deine Antwort:_

**B5 🟡 Braucht ein Beleg einen Status?**
Analog zu den bestehenden Status-Ketten im Programm.
- Vorschlag: **erfasst** → **einem Förderer zugeordnet** → **abgerechnet/
  eingereicht**. Plus optional „geprüft".
- **Empfehlung:** Ja, schlanke Kette wie oben. → _Deine Antwort:_

**B6 ⚪ Mehrere Dateien pro Beleg?** (z. B. Rechnung + Zahlungsnachweis)
- **Empfehlung:** Ja, beliebig viele Dateien je Beleg erlauben.
  → _Deine Antwort:_

---

## C. Kostenstelle (Verknüpfung zum KFP)

**C1 🟢 Was genau ist die „Kostenstelle"?**
Im KFP gibt es Kategorien (z. B. „1 Personalkosten") und darunter Posten
(z. B. „1.2 Honorar Regie").
- (a) Beleg wird mit einem **Posten** verknüpft (fein, z. B. „1.2").
- (b) Beleg wird nur mit einer **Kategorie** verknüpft (grob, z. B. „1").
- **Empfehlung:** (a) Posten-Ebene – das ist die echte Kostenstelle und erlaubt
  später den Plan-/Ist-Vergleich je Posten. → _Deine Antwort:_

**C2 🟡 Was, wenn es noch keinen passenden Posten gibt?**
- (a) Beleg ohne Kostenstelle erfassen, später zuordnen (erlaubt „laufend").
- (b) Direkt aus der Beleg-Maske einen neuen KFP-Posten anlegen.
- **Empfehlung:** Beides: Kostenstelle ist **optional** beim Erfassen (a) und
  man kann **neu anlegen** (b). → _Deine Antwort:_

**C3 🟡 Kann EIN Beleg auf mehrere Kostenstellen aufgeteilt werden?**
(z. B. eine Baumarkt-Rechnung: 60 € Material + 40 € Werkzeug.)
- (a) Nein – ein Beleg = eine Kostenstelle (einfach).
- (b) Ja – ein Beleg kann in **Teilbeträge** auf mehrere Kostenstellen
  gesplittet werden (realistischer, aber komplexer).
- **Empfehlung:** Start mit (a), Architektur aber so anlegen, dass (b) später
  möglich ist. → _Deine Antwort:_

**C4 🟡 Plan-/Ist-Vergleich anzeigen?**
Je Kostenstelle: geplant (KFP) vs. tatsächlich (Summe der Belege), plus Rest.
- **Empfehlung:** Ja – das ist einer der größten Nutzen. → _Deine Antwort:_

---

## D. Förderer-Zuordnung (das Herz der Abrechnung)

**D1 🟢 Wie wird ein Beleg einem Förderer zugeordnet?**
- (a) **Direkt**: Beleg → Förderer (du wählst pro Beleg den Förderer).
- (b) **Über die Kostenstelle**: ein Förderer „bezahlt" bestimmte
  Kostenstellen; alle Belege darauf zählen automatisch zu ihm.
- (c) **Beides** möglich.
- **Empfehlung:** (a) als klare Grundregel (volle Kontrolle), Kostenstelle hilft
  beim Filtern/Vorschlagen. → _Deine Antwort:_

**D2 🟢 Kann ein Beleg auf MEHRERE Förderer aufgeteilt werden?**
Sehr häufig bei Ko-Finanzierung: 1.000 € Kosten, 500 € Förderer A, 500 €
Förderer B (oder Eigenmittel).
- (a) Nein – ein Beleg gehört genau einem Förderer (einfach, aber oft
  unrealistisch).
- (b) Ja – **anteilige Aufteilung** eines Belegs auf mehrere Förderer/
  Eigenmittel (Beträge müssen ≤ Belegsumme sein).
- **Empfehlung:** (b) – ohne anteilige Zuordnung ist eine echte Abrechnung kaum
  möglich. → _Deine Antwort:_

**D3 🟢 Doppelförderung verhindern?**
Fast alle Förderer verbieten, **denselben Euro zweimal** abzurechnen
(Doppelförderungsverbot).
- **Empfehlung:** Ja – das Programm soll **warnen**, wenn von einem Beleg mehr
  zugeordnet wird als sein Betrag, und je Beleg anzeigen, wie viel noch „frei"
  ist. → _Deine Antwort:_

**D4 🟡 Förderart berücksichtigen?**
Förderer finanzieren unterschiedlich:
- **Festbetrag** (fester Betrag, egal wie hoch die Kosten),
- **Anteilfinanzierung** (fester %-Satz der Kosten – passt zum neuen Feld
  *max. Anteil am Gesamtbudget*),
- **Fehlbedarfsfinanzierung** (deckt, was sonst übrig bleibt).
- (a) Ignorieren – du ordnest einfach Beträge zu.
- (b) Förderart je Förderung hinterlegen und die Abrechnung **prüft/rechnet**
  entsprechend (z. B. „max. 50 % – du hast 60 % zugeordnet").
- **Empfehlung:** (b) leichtgewichtig: Förderart + Höchstsumme/Anteil
  hinterlegen, Programm warnt bei Überschreitung. → _Deine Antwort:_

**D5 🟡 Nicht-förderfähige Kosten / nur bestimmte Kostenarten?**
Manche Förderer erlauben z. B. keine Bewirtung oder nur Honorare.
- (a) Ignorieren.
- (b) Pro Förderung markieren, welche Kostenstellen **förderfähig** sind;
  Abrechnung warnt bei nicht erlaubten.
- **Empfehlung:** (b) als optionale Einschränkung. → _Deine Antwort:_

**D6 🟡 Eigenmittel & sonstige Einnahmen** (Ticketverkauf, Eigenleistung)
müssen mit abgebildet werden (sie tauchen im KFP-Finanzierungsplan auf).
- **Empfehlung:** „Eigenmittel/Einnahmen" wie ein Förderer behandeln, dem man
  Beträge zuordnen kann. → _Deine Antwort:_

---

## E. Ergebnis / Export

**E1 🟢 In welchem Format soll der Verwendungsnachweis herauskommen?**
(Heute kann das Programm Word, PDF und Excel.)
- (a) **Excel** (Belegliste als Tabelle – am flexibelsten, viele Förderer
  wollen das).
- (b) **PDF** (fertig zum Einreichen/Hochladen).
- (c) **Word** (zum Nachbearbeiten).
- **Empfehlung:** (a) + (b): Excel-Belegliste und ein PDF. → _Deine Antwort:_

**E2 🟡 Was steht in der Belegliste je Förderer?**
Vorschlag der Spalten: lfd. Nr. · Datum · Empfänger · Zweck · Kostenstelle ·
Belegsumme · **diesem Förderer zugeordneter Betrag** · Summe unten.
- → _Deine Antwort (Spalten ok?):_

**E3 🟡 Gesamtübersicht zusätzlich zur Förderer-Liste?**
Eine Übersicht über das ganze Projekt: Kosten geplant vs. ist, je Förderer
zugeordnet, Rest/Eigenmittel, offen.
- **Empfehlung:** Ja. → _Deine Antwort:_

**E4 ⚪ Müssen die Beleg-Dateien mit exportiert werden?**
(z. B. ein PDF mit allen Belegen hintereinander, wie heute beim Antrag die
Anhänge angehängt werden.)
- **Empfehlung:** Optional – „alle Belege eines Förderers als ein PDF
  zusammenfügen". → _Deine Antwort:_

---

## F. Kontrolle & Warnungen

**F1 🟡 Welche automatischen Prüfungen sollen warnen?** (Vorschlag)
- Von einem Beleg ist **mehr zugeordnet als sein Betrag** (Doppelförderung).
- Einem Förderer sind **mehr Kosten zugeordnet als seine Fördersumme**.
- Eine **Kostenstelle ist überzogen** (Ist > Plan).
- Belege **ohne Kostenstelle** oder **ohne Zuordnung** (Resterfassung).
- → _Deine Antwort (welche willst du?):_

**F2 ⚪ Rückzahlung/Unterschreitung:** Wenn weniger ausgegeben wurde als
gefördert – nur anzeigen oder weiter behandeln?
- **Empfehlung:** zunächst nur anzeigen. → _Deine Antwort:_

---

## G. Daten & Architektur (kurz, zur Bestätigung)

**G1 🟢 Belege liegen vollständig im lokalen, verschlüsselten Tresor** (Beträge,
Lieferanten) bzw. als verschlüsselte Beleg-Dateien daneben – **nichts geht ins
Netz / auf die NAS.** Bestätigst du das als feste Regel? → _Deine Antwort:_

**G2 🟡 Wo im Programm lebt der Modus?**
- (a) Eigener Hauptbereich „Abrechnung" neben den bestehenden Bereichen.
- (b) Reiter innerhalb eines Projekts.
- **Empfehlung:** (a) eigener Hauptbereich, da es ein großer eigener Modus ist.
  → _Deine Antwort:_

**G3 ⚪ Ab wann ist ein Projekt „in Abrechnung"?**
- **Empfehlung:** Belege gehen jederzeit (auch parallel zum Antrag); die
  Abrechnung selbst machst du, wenn du willst. → _Deine Antwort:_

---

## H. Grenzfälle (später, nur zum Mitdenken)

- **Stornos/Gutschriften** (negative Belege)? ⚪
- **Anzahlungen / Teilrechnungen**? ⚪
- **Personalkosten/Honorare** mit Vertrag statt Kassenbon – eigener Belegtyp? 🟡
- **Unbare Eigenleistung** (Arbeitszeit als Eigenmittel)? ⚪
- **Verwendungsnachweis-Frist** je Förderer (knüpft an das bestehende
  Fristen-/Kalender-System an)? 🟡
- → _Notizen/Wünsche:_

---

## I. Was ist dir sonst wichtig?

Freitext – alles, was oben fehlt, Beispiele aus deinen echten Abrechnungen,
ein Förderer-Formular, das wir nachbilden sollen, usw.:

→ _Deine Antwort:_

---

### Nächster Schritt
Wenn du die 🟢-Fragen (und gern mehr) beantwortet hast, schreibe ich daraus
eine **Bau-Spezifikation** (wie ein CLAUDE.md-Kapitel) und einen **Phasenplan**
in kleinen Schritten. Erst danach beginnt das Programmieren.
