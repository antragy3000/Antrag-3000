// ============================================================
// Pruefsummen-Test: prueft, dass die Dateien aus release/pruefsummen/ richtig
// gelesen werden und die Release-Skripte passende Namen erzeugen – ohne GitHub.
//
// Ausfuehren:  npm run pruefsummen   (oder: node tools/pruefsummen-test.mjs)
// ============================================================

import fs from "node:fs";
import { beschreibungBauen, eintragLesen } from "../release/pruefsummen-veroeffentlichen.mjs";

let fehler = 0;
function pruefe(bedingung, beschreibung) {
  if (bedingung) {
    console.log(`  ok   ${beschreibung}`);
  } else {
    console.error(`  FEHL ${beschreibung}`);
    fehler++;
  }
}
function wirftFehler(fn) {
  try {
    fn();
    return false;
  } catch {
    return true;
  }
}

const HASH = "a".repeat(64);
const BASIS = "https://sync.antrag3000.de/updates";

console.log("Dateinamen und Adressen");
const nutzer = eintragLesen("antrag-3000-0.5.0-windows.sha256", `${HASH}  Antrag 3000_0.5.0_x64-setup.exe\n`);
pruefe(nutzer.tag === "v0.5.0", "Nutzer-App bekommt den Tag v0.5.0");
pruefe(nutzer.neueste === true, "Nutzer-App gilt als neueste Version");
pruefe(nutzer.url === `${BASIS}/Antrag%203000_0.5.0_x64-setup.exe`, "Leerzeichen in der Adresse sind kodiert");

const admin = eintragLesen("antrag-3000-admin-0.2.7-windows.sha256", `${HASH}  Antrag 3000 – Admin_0.2.7_x64-setup.exe\n`);
pruefe(admin.tag === "admin-v0.2.7" && admin.neueste === false, "Admin-App hat eigenen Tag und gilt nie als neueste");
pruefe(admin.url === `${BASIS}/admin/Antrag%203000%20%E2%80%93%20Admin_0.2.7_x64-setup.exe`, "Admin-Adresse mit Gedankenstrich stimmt");

const foerderer = eintragLesen("antrag-3000-foerderer-0.3.3-windows.sha256", `${HASH}  Antrag 3000 – Förderer_0.3.3_x64-setup.exe\n`);
pruefe(foerderer.tag === "foerderer-v0.3.3" && foerderer.neueste === false, "Förderer-App hat eigenen Tag und gilt nie als neueste");
pruefe(foerderer.url.includes("/foerderer/") && foerderer.url.includes("F%C3%B6rderer"), "Förderer-Adresse mit ö stimmt");
pruefe(foerderer.titel === "Antrag 3000 – Förderer 0.3.3", "Förderer-Titel stimmt");

const mac = eintragLesen("antrag-3000-0.5.0-macos.sha256", `${HASH}  Antrag-3000_0.5.0_universal.dmg\n`);
pruefe(mac.tag === nutzer.tag, "Mac und Windows derselben Version landen im selben Release");

console.log("Fehlerhafte Dateien werden abgelehnt");
pruefe(wirftFehler(() => eintragLesen("sonstwas.sha256", `${HASH}  a_1.0.0.exe`)), "unbekannter Dateiname");
pruefe(wirftFehler(() => eintragLesen("antrag-3000-0.5.0-windows.sha256", "kaputt")), "kaputter Inhalt");
pruefe(wirftFehler(() => eintragLesen("antrag-3000-0.5.0-windows.sha256", `${"a".repeat(63)}  a_0.5.0.exe`)), "zu kurze Pruefsumme");
pruefe(wirftFehler(() => eintragLesen("antrag-3000-0.5.0-windows.sha256", `${HASH}  Antrag 3000_0.4.19_x64-setup.exe`)), "Datei einer anderen Version");
pruefe(
  eintragLesen("antrag-3000-0.5.0-windows.sha256", `${HASH}  Antrag 3000_0.5.0_x64-setup.exe\r\n`).datei === "Antrag 3000_0.5.0_x64-setup.exe",
  "Windows-Zeilenende stört nicht",
);

console.log("Text der Release-Seite");
const text = beschreibungBauen([mac, nutzer]);
pruefe(text.includes(`\`${HASH}\``), "enthält die Prüfsumme");
pruefe(text.includes('Get-FileHash "$HOME\\Downloads\\Antrag 3000_0.5.0_x64-setup.exe" -Algorithm SHA256'), "Windows-Befehl stimmt");
pruefe(text.includes("shasum -a 256 ~/Downloads/Antrag-3000_0.5.0_universal.dmg"), "Mac-Befehl stimmt");
pruefe(text.indexOf("Get-FileHash") < text.indexOf("shasum"), "Windows steht vor Mac");
pruefe(text.includes("pilot@antrag3000.de"), "sagt, wohin man eine Abweichung meldet");

console.log("Festgehaltene Dateien und Release-Skripte passen zusammen");
const ORDNER = new URL("../release/pruefsummen/", import.meta.url);
for (const name of fs.readdirSync(ORDNER)) {
  const inhalt = fs.readFileSync(new URL(name, ORDNER), "utf8");
  pruefe(!wirftFehler(() => eintragLesen(name, inhalt)) && !inhalt.includes("?"), `${name} ist lesbar`);
}
for (const skript of ["release", "admin-app/release", "foerderer-app/release"]) {
  const quelle = fs.readFileSync(new URL(`../${skript}/veroeffentlichen.ps1`, import.meta.url), "utf8");
  const name = quelle.match(/pruefsumme-festhalten\.ps1"?\s+-SumDatei \$sumDatei -Name "([^"]+)"/)?.[1];
  pruefe(!!name && !wirftFehler(() => eintragLesen(`${name.replace("$version", "1.2.3")}.sha256`, `${HASH}  x_1.2.3.exe`)), `${skript}: erzeugt einen gültigen Namen`);
  pruefe(!quelle.includes("Set-Content -Path $sumDatei -Encoding ascii"), `${skript}: schreibt Umlaute korrekt`);
}

if (fehler > 0) {
  console.error(`\n❌ Pruefsummen-Test: ${fehler} Fehler.`);
  process.exit(1);
}
console.log("\n✅ Pruefsummen-Test bestanden.");
