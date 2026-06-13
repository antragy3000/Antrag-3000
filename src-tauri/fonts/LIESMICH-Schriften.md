# Eingebettete Schriften

Für die Erzeugung des Antrags-PDF wird die Schrift **Open Sans**
(Regular + Bold) in das Programm eingebettet (`include_bytes!` in
`src/pdf.rs`). Open Sans deckt deutsche Umlaute und Sonderzeichen ab.

- Schrift: Open Sans
- Designer: Steve Matteson
- Lizenz: Apache License 2.0 – erlaubt freie Weitergabe, auch eingebettet
  in andere Programme.

Diese Dateien werden bewusst mitversioniert, damit das PDF auf jedem
Rechner identisch und ohne externe Abhängigkeit erzeugt wird.
