# Benutzerhandbuch

- **`handbuch.html`** – Quelle des Handbuchs (eigenständig, alle Stile und
  Abbildungen inline; die Bildschirm-Nachbauten sind im echten App-Look
  per HTML/CSS gezeichnet).
- **`Benutzerhandbuch Antrag 3000.pdf`** – das fertige A4-PDF (22 Seiten).

## PDF neu erzeugen
Aus dem HTML wird das PDF mit dem auf Windows vorhandenen Edge im
Hintergrund gedruckt (kein Zusatzprogramm nötig):

```powershell
$edge = Get-ChildItem "C:\Program Files (x86)\Microsoft\EdgeCore" -Recurse -Filter msedge.exe |
  Sort-Object FullName -Descending | Select-Object -First 1 -ExpandProperty FullName
$html = "$PSScriptRoot\handbuch.html"
$pdf  = "$PSScriptRoot\Benutzerhandbuch Antrag 3000.pdf"
& $edge --headless=new --disable-gpu --no-pdf-header-footer `
  --user-data-dir="$env:TEMP\edge-pdf" `
  --print-to-pdf="$pdf" ([System.Uri]$html).AbsoluteUri
```

Nach Änderungen an der App sollte das Handbuch entsprechend aktualisiert
werden (Texte und ggf. die nachgebauten Abbildungen in `handbuch.html`).
