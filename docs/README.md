# Benutzerhandbuch

- **`handbuch.html`** – Quelle des Handbuchs (eigenständig, alle Stile und
  Abbildungen inline; die Bildschirm-Nachbauten sind im echten App-Look
  per HTML/CSS gezeichnet).
- **`handbuch-bauen.mjs`** – Bau-Skript: druckt das PDF mit
  **Seitenzahlen** und trägt die **Kapitel-Seiten ins Inhaltsverzeichnis**
  ein.
- **`Benutzerhandbuch Antrag 3000.pdf`** – das fertige A4-PDF (25 Seiten).

## PDF neu erzeugen
Im Projekt-Stammordner ausführen (braucht nur Edge + Ghostscript, beide
auf dem Rechner vorhanden – kein Internet):

```
node docs/handbuch-bauen.mjs
```

Das Skript druckt zweimal: Pass 1 erkennt über die ausgelesenen
Seitentexte, auf welcher Seite jedes Kapitel beginnt, trägt diese Zahlen
ins Inhaltsverzeichnis von `handbuch.html` ein und druckt dann das
endgültige PDF (mit Seitenzahl-Fußzeile). Die Seitenzahlen im
Inhaltsverzeichnis werden dabei automatisch aktualisiert.

Nach Änderungen am Handbuch (`handbuch.html`) einfach das Skript erneut
laufen lassen.
