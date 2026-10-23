# Datenschutz bei Antrag 3000

*Stand: Oktober 2026 · gilt ab App-Version 0.5.0*

**Kurz gesagt:** Deine Daten bleiben auf deinem Gerät. Antrag 3000 holt aus dem
Internet nur App-Updates und die öffentliche Förder-Datenbank – und schickt dabei
nichts von dir mit. Es gibt keine Werbung, keine Tracker und keine Nutzungsstatistik.

---

## 1. Was auf deinem Gerät bleibt

**Im Tresor (verschlüsselt):** Stammdaten (Name, Adresse, Bankverbindung,
Steuerangaben), Projekte, Formular-Texte, Kosten- und Finanzierungsplan, Angaben zu
Belegen, Notizen und Fragebogen-Antworten.

- Der Tresor ist mit **AES-256-GCM** verschlüsselt. Der Schlüssel wird mit
  **Argon2id** aus deinem Passwort abgeleitet.
- Ohne dein Passwort kann niemand die Daten lesen – auch das Antrag-3000-Team nicht.
  Darum lässt sich ein vergessenes Passwort auch nicht „zurücksetzen".
- „Auf diesem Gerät merken" legt den Schlüssel geschützt durch Windows (DPAPI) ab.
  Er funktioniert nur in deinem Windows-Konto auf diesem Computer.

**Als normale Dateien (nicht im Tresor):** Dokumente, die du hochlädst oder die die
App erzeugt (Word, PDF, Belege), liegen im Ordner `Dokumente\Antrag 3000`. Sie sind
durch dein Windows-Konto geschützt, aber nicht zusätzlich verschlüsselt.

Diese Daten verlassen dein Gerät nie über eine Netzwerkverbindung der App.

## 2. Wann die App ins Internet geht

### Einzelplatz (allein arbeiten)

Die App fragt nur beim Antrag-3000-Server `sync.antrag3000.de` an:

| Wozu | Adresse | Wann |
|---|---|---|
| Gibt es ein App-Update? | `/updates/latest.json` (macOS: `/updates/mac.json`) | beim Start\* oder per „⬆ Update" |
| Update herunterladen | `/updates/…` | nur wenn du das Update bestätigst |
| Förder-Datenbank holen | `/katalog` | beim Start\* oder per „Förder-Datenbank aktualisieren" |
| Logo eines Förderers | `/logos/…` | nur wenn du es herunterlädst |

\* **Du entscheidest:** Beim ersten Start fragt die App, ob sie *automatisch beim
Start* oder *nur auf Knopfdruck* nachsehen soll. Ändern kannst du das jederzeit unter
**Stammdaten & Team → Internet-Verbindungen**. Bis du gewählt hast, verbindet sich die
App mit nichts.

**Was dabei mitgeschickt wird:** nichts von dir – kein Konto, keine Geräte-Kennung,
keine Cookies, keine Inhalte aus deinem Tresor. Wie bei jeder Internet-Verbindung
sieht der Server technisch die IP-Adresse deines Anschlusses und den Zeitpunkt der
Anfrage (siehe Abschnitt 3: er speichert sie nicht).

**Fehler in der Förder-Datenbank melden:** Im Einzelplatz öffnet die App dein
E-Mail-Programm mit einer vorbereiteten Nachricht an `pilot@antrag3000.de`. Du siehst
den Text und schickst ihn selbst ab – oder eben nicht.

### Team (zu mehreren arbeiten)

Im Team-Modus gleicht die App zusätzlich mit eurem Team-Server
(`team.antrag3000.de`) ab – erst, wenn du die Synchronisation startest. Die Verbindung
ist verschlüsselt, und jedes Gerät weist sich mit einem eigenen Zertifikat aus
(gegenseitiges TLS).

- **Geteilt werden:** Projektnamen, gemerkte Förderungen mit Status, Fristen,
  Förderer-Kontakte, Dokument-Checklisten und eure eigenen Förderer.
- **Nie geteilt werden:** Stammdaten, Formular-Texte, Kostenplan, Fragebogen,
  Notizen und die Dateien selbst.
- Unter **Team-Synchronisation → Prüfen & Protokoll** siehst du vorab genau, was
  gesendet würde (Sende-Vorschau).
- Meldungen zur Förder-Datenbank gehen im Team-Modus über den Team-Server.

### Links, die du selbst öffnest

Webseiten von Förderern, CryptPad-Dokumente oder diese Erklärung öffnet die App in
deinem Browser. Dort gelten die Datenschutzregeln des jeweiligen Anbieters. In
CryptPad landet nur, was du selbst dorthin kopierst.

## 3. Was der Server speichert

*Geprüft am 21.10.2026 auf dem laufenden Server.*

- **Keine Zugriffs-Protokolle:** Der Webserver (Caddy) ist so eingerichtet, dass er
  Abrufe nicht mitschreibt – weder IP-Adressen noch abgerufene Adressen.
- **Keine IP-Adressen in der Datenbank:** Der Sync-Dienst speichert keine
  IP-Adressen. In seine Betriebsmeldungen schreibt er nur Start-Informationen.
- **Technische Betriebsmeldungen:** Der Webserver notiert z. B. die Erneuerung seiner
  Zertifikate. Nur wenn eine Anfrage an einem technischen Fehler scheitert, kann
  darin die IP-Adresse dieser Anfrage stehen. Diese Meldungen dienen allein der
  Fehlersuche; sie werden nicht ausgewertet und nicht weitergegeben.
- **Im Team-Modus speichert der Server, was ihr teilt:** die geteilten Angaben aus
  Abschnitt 2, den Team-Namen, die Anzeigenamen der Mitglieder, je Gerät eine
  Bezeichnung, den Fingerabdruck seines Zertifikats und wann es zuletzt verbunden war,
  sowie eure Meldungen. Damit dein Team diese Angaben sieht, liegen sie auf dem Server
  im Klartext (nicht Ende-zu-Ende-verschlüsselt).
- **Sicherungen:** Die Team-Datenbank wird jede Nacht gesichert; die Sicherungen
  werden 14 Tage aufbewahrt. Eine weitere Kopie liegt beim Betreiber.
- **Standort:** Der Server steht in einem Rechenzentrum der netcup GmbH in Nürnberg
  (Deutschland).

## 4. Keine Tracker, keine fremden Inhalte

- Keine Werbung, keine Analyse-Werkzeuge, keine Nutzungsstatistik, keine
  automatischen Absturzberichte.
- Die Oberfläche lädt keine Schriften, Bilder oder Skripte aus dem Internet; eine
  Sicherheitsregel im App-Fenster (Content Security Policy) verbietet das.

## 5. Deine Rechte und Löschen

Fast alles liegt nur bei dir – du kannst es selbst löschen: über **„Neu aufsetzen"**
(Tresor leeren), durch Deinstallieren der App und durch Löschen des Ordners
`Dokumente\Antrag 3000`.

Für Team-Daten auf dem Server, für Auskunft oder Löschung schreib an
`pilot@antrag3000.de`.

## 6. Verantwortlich

<!-- VOR DER ÖFFENTLICHEN FREIGABE (Nutzer ausserhalb des Bekanntenkreises,
     Website): Name und ladungsfähige Anschrift des Betreibers ergänzen
     (Pflichtangabe nach Art. 13 DSGVO). -->
Antrag 3000 läuft derzeit als **Pilotprojekt im persönlichen Umfeld** des Entwicklers.
Kontakt: `pilot@antrag3000.de`

Antrag 3000 ist freie Software (GPLv3). Den Quellcode – und damit genau, was die App
tut – kannst du auf GitHub einsehen: <https://github.com/antragy3000/Antrag-3000>
