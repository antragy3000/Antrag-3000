# Code-Signing-Richtlinie / Code Signing Policy

*Stand: Juli 2026*

> Hinweis: Die Einrichtung des Code-Signings über die SignPath Foundation ist
> derzeit im Aufbau. Dieses Dokument beschreibt, wie die Windows-Installer von
> Antrag 3000 signiert werden.

## Warum werden die Programme signiert?

Eine digitale Signatur (Code-Signing) belegt, dass ein Installer tatsächlich vom
Projekt **Antrag 3000** stammt und nach dem Signieren nicht verändert wurde.
Windows und Virenschutz-Programme erkennen signierte Dateien als vertrauenswürdig
und zeigen keine Warnung vor einem „unbekannten Herausgeber".

## Welche Dateien werden signiert?

- Die Windows-Installer von Antrag 3000 (NSIS-Setup, `*-setup.exe`, 64 Bit).

Die App-eigenen Selbst-Updates sind zusätzlich mit einer eigenen Signatur
(minisign) abgesichert, die die App vor dem Einspielen prüft. Das ist von der hier
beschriebenen Windows-Signatur unabhängig.

## Wie entstehen die signierten Dateien?

- Der gesamte Quellcode ist öffentlich in diesem Repository.
- Die Installer werden nachvollziehbar aus diesem Quellcode gebaut
  (Tauri-Standard-Toolchain).
- Signiert wird ausschließlich mit dem von der **SignPath Foundation**
  bereitgestellten Zertifikat. Der private Schlüssel liegt auf einem gesicherten
  Hardware-Modul (HSM) bei SignPath und wird vom Projekt nie eingesehen oder lokal
  gespeichert.
- Jede Freigabe zum Signieren wird **manuell** durch die verantwortliche Person
  geprüft und bestätigt.

## Verantwortlich

Maintainer / Approver: **Antrag 3000 Team** — pilot@antrag3000.de

Zugänge zu Quellcode-Repository und Signier-Dienst sind mit
Zwei-Faktor-Authentifizierung geschützt.

## Signatur selbst prüfen

Unter Windows: Rechtsklick auf die Installer-Datei → **Eigenschaften** → Reiter
**Digitale Signaturen**. Dort muss Antrag 3000 als Unterzeichner erscheinen und die
Signatur als gültig ausgewiesen sein.

---

Free code signing provided by [SignPath.io](https://signpath.io), certificate by
[SignPath Foundation](https://signpath.org).
