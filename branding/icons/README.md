# Antrag 3000 — Produkt-Icons

Dieselbe Marke (der Aktenstapel), pro Produkt umgefärbt. Die Farbe unterscheidet
die Anwendungen.

## Produkte & Farben

| Produkt        | Ordner          | Farbe (Hex) | Icon-Kachel        |
|----------------|-----------------|-------------|--------------------|
| Admin-App      | `admin-app/`    | `#4f6df5`   | Blaue Kachel, weiße Marke |
| Förderer       | `foerderer/`    | `#f5a623`   | Gold-Kachel, weiße Marke  |
| Antrag 3000    | `antrag-3000/`  | `#2fe0d1`   | Dunkle Kachel, Aqua-Marke (Referenz) |

## Dateien pro Ordner

- `icon.svg` — App-Icon (Kachel mit Farbe + Marke), vektoriell, beliebig skalierbar
- `mark.svg` — nur die Marke, transparenter Hintergrund, einfarbig (für Wortmarke / auf Papier)
- `icon-16 … icon-1024.png` — gerasterte App-Icons in Standardgrößen (16, 32, 48, 64, 128, 256, 512, 1024 px)

## Verwendung (Tauri / Web)

- **Tauri** (`src-tauri/icons/`): `icon-256.png` bzw. `icon-1024.png` als Quelle;
  für `.icns`/`.ico` z. B. `tauri icon icons/admin-app/icon-1024.png`.
- **Favicon**: `icon-32.png` / `icon-16.png`, oder `icon.svg` direkt.
- **In der UI**: `icon.svg` (Kachel) oder `mark.svg` (einfarbige Marke, per
  `currentColor`-Ersatz beliebig einfärbbar — im SVG steht die Produktfarbe fest).

Alle Größen sind aus derselben Vektorquelle gerendert, also pixelscharf.
