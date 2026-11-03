// ============================================================
// Traegt die SHA-256-Pruefsummen der Installer als GitHub-Release ein.
//
// Warum: Die Installer sind noch nicht code-signiert. Damit man trotzdem pruefen
// kann, ob eine Datei echt ist, liegt ihr digitaler Fingerabdruck nicht nur auf
// dem Download-Server, sondern auch auf GitHub. Wer eine Datei faelschen will,
// muesste beide Orte zugleich manipulieren.
//
// Laeuft in GitHub Actions (.github/workflows/pruefsummen.yml, macos-pilot.yml).
// Vor dem Eintragen wird die Datei vom Server geladen und verglichen; stimmt sie
// nicht mit dem ueberein, was gebaut wurde, wird der Lauf rot und nichts eingetragen.
//
// Aufruf:
//   node release/pruefsummen-veroeffentlichen.mjs
//       alle Dateien aus release/pruefsummen/ eintragen (schon eingetragene bleiben)
//   node release/pruefsummen-veroeffentlichen.mjs --probelauf
//       nur Server pruefen und Text anzeigen; aendert nichts, braucht kein GitHub
//   node release/pruefsummen-veroeffentlichen.mjs --ohne-server <datei>
//       fuer den Mac-Bau, der die .dmg selbst gebaut hat
// ============================================================
import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

const SERVER = "https://sync.antrag3000.de/updates";
const ORDNER = path.join(path.dirname(fileURLToPath(import.meta.url)), "pruefsummen");

// Der Dateiname sagt, zu welcher App und Version die Pruefsumme gehoert. Nur die
// Nutzer-App darf auf GitHub als "neueste Version" gelten (neueste: true).
const ARTEN = [
  { muster: /^antrag-3000-(\d+\.\d+\.\d+)-windows\.sha256$/, tag: "v", titel: "Antrag 3000", ordner: "", neueste: true, system: "windows" },
  { muster: /^antrag-3000-(\d+\.\d+\.\d+)-macos\.sha256$/, tag: "v", titel: "Antrag 3000", ordner: "mac/", neueste: true, system: "macos" },
  { muster: /^antrag-3000-admin-(\d+\.\d+\.\d+)-windows\.sha256$/, tag: "admin-v", titel: "Antrag 3000 – Admin", ordner: "admin/", neueste: false, system: "windows" },
  { muster: /^antrag-3000-foerderer-(\d+\.\d+\.\d+)-windows\.sha256$/, tag: "foerderer-v", titel: "Antrag 3000 – Förderer", ordner: "foerderer/", neueste: false, system: "windows" },
];

/// Liest eine Pruefsummen-Datei (Format wie bei sha256sum: "<hash>  <dateiname>").
export function eintragLesen(dateiName, inhalt) {
  const art = ARTEN.find((a) => a.muster.test(dateiName));
  if (!art) throw new Error(`Unbekannter Dateiname: ${dateiName}`);
  const version = dateiName.match(art.muster)[1];

  const treffer = inhalt.replace(/\r/g, "").trim().match(/^([0-9a-f]{64}) {2}([^\n]+)$/);
  if (!treffer) throw new Error(`${dateiName}: Inhalt ist keine gueltige Pruefsummen-Zeile`);
  const [, hash, datei] = treffer;
  if (!datei.includes(version)) throw new Error(`${dateiName}: "${datei}" passt nicht zur Version ${version}`);

  return {
    dateiName,
    hash,
    datei,
    system: art.system,
    neueste: art.neueste,
    tag: art.tag + version,
    titel: `${art.titel} ${version}`,
    url: `${SERVER}/${art.ordner}${encodeURIComponent(datei)}`,
  };
}

/// Der Text der Release-Seite. Liest vor allem jemand ohne Technik-Vorwissen.
export function beschreibungBauen(eintraege) {
  const sortiert = [...eintraege].sort((a, b) => b.system.localeCompare(a.system));
  const befehle = sortiert.map((e) =>
    e.system === "macos"
      ? `**macOS** (im Programm „Terminal“):\n\n\`\`\`bash\nshasum -a 256 ~/Downloads/${e.datei}\n\`\`\``
      : `**Windows** (im Programm „PowerShell“):\n\n\`\`\`powershell\nGet-FileHash "$HOME\\Downloads\\${e.datei}" -Algorithm SHA256\n\`\`\``,
  );
  return [
    "Hier steht die **Prüfsumme** der Installationsdatei, eine Art digitaler Fingerabdruck. Damit kannst du prüfen, ob deine heruntergeladene Datei genau die ist, die das Antrag-3000-Team gebaut hat. Die Installationsdatei selbst liegt nicht hier.",
    "",
    "| Datei | SHA-256-Prüfsumme |",
    "|---|---|",
    ...sortiert.map((e) => `| \`${e.datei}\` | \`${e.hash}\` |`),
    "",
    "## So prüfst du deine Datei",
    "",
    ...befehle.flatMap((b) => [b, ""]),
    "Falls die Datei nicht im Ordner „Downloads“ liegt, passe den Pfad an. Die angezeigte Zeichenfolge muss mit der Tabelle übereinstimmen; Groß- und Kleinschreibung spielen keine Rolle.",
    "",
    "**Stimmt sie nicht überein:** Installiere die Datei nicht, lösche sie und melde es an pilot@antrag3000.de.",
    "",
    "## Warum hier?",
    "",
    "Die Prüfsumme liegt bewusst auch auf GitHub und nicht nur auf dem Download-Server. Wer eine Datei fälschen will, müsste beide Orte zugleich manipulieren.",
    "",
  ].join("\n");
}

function gh(...args) {
  return execFileSync("gh", args, { encoding: "utf8", stdio: ["ignore", "pipe", "pipe"] });
}

/// Alle Pruefsummen eines Release (Name -> Inhalt), oder null, wenn es noch keins gibt.
function releaseLesen(tag) {
  let info;
  try {
    info = JSON.parse(gh("release", "view", tag, "--json", "assets"));
  } catch (e) {
    if (String(e.stderr).includes("release not found")) return null;
    throw e;
  }
  const dateien = new Map();
  for (const a of info.assets) {
    if (a.name.endsWith(".sha256")) dateien.set(a.name, gh("release", "download", tag, "--pattern", a.name, "--output", "-"));
  }
  return dateien;
}

async function serverPruefen(eintrag) {
  const antwort = await fetch(eintrag.url);
  if (!antwort.ok) throw new Error(`Server antwortet mit ${antwort.status}: ${eintrag.url}`);
  const hash = createHash("sha256");
  for await (const teil of antwort.body) hash.update(teil);
  const ist = hash.digest("hex");
  if (ist !== eintrag.hash) {
    throw new Error(`Die Datei auf dem Server ist NICHT die gebaute!\n  gebaut: ${eintrag.hash}\n  Server: ${ist}\n  ${eintrag.url}`);
  }
  console.log("  Server-Datei stimmt mit der gebauten überein.");
}

function eintragen(eintrag, pfad, vorhanden) {
  if (!vorhanden) {
    // --target wirkt nur, wenn der Tag noch nicht existiert.
    const ziel = process.env.GITHUB_SHA ? ["--target", process.env.GITHUB_SHA] : [];
    gh("release", "create", eintrag.tag, "--title", eintrag.titel, "--notes", "", ...ziel, `--latest=${eintrag.neueste}`);
  }
  gh("release", "upload", eintrag.tag, pfad, "--clobber");
  // Den Text aus ALLEN Dateien des Release bauen: Windows und Mac kommen getrennt an.
  const alle = [...releaseLesen(eintrag.tag)].map(([name, inhalt]) => eintragLesen(name, inhalt));
  const textDatei = path.join(fs.mkdtempSync(path.join(os.tmpdir(), "pruefsummen-")), "text.md");
  fs.writeFileSync(textDatei, beschreibungBauen(alle));
  gh("release", "edit", eintrag.tag, "--notes-file", textDatei);
  console.log(`  Eingetragen im Release ${eintrag.tag}.`);
}

async function hauptprogramm() {
  const argumente = process.argv.slice(2);
  const probelauf = argumente.includes("--probelauf");
  const ohneServer = argumente.includes("--ohne-server");
  const angegeben = argumente.filter((a) => !a.startsWith("--"));
  const pfade = angegeben.length
    ? angegeben
    : fs
        .readdirSync(ORDNER)
        .filter((d) => d.endsWith(".sha256"))
        // Numerisch sortiert: So wird bei mehreren neuen Versionen die hoechste zuletzt angelegt und bleibt "neueste".
        .sort((a, b) => a.localeCompare(b, undefined, { numeric: true }))
        .map((d) => path.join(ORDNER, d));

  let fehler = 0;
  for (const pfad of pfade) {
    try {
      const inhalt = fs.readFileSync(pfad, "utf8");
      const eintrag = eintragLesen(path.basename(pfad), inhalt);
      console.log(`\n${eintrag.titel}: ${eintrag.datei}`);
      if (probelauf) {
        await serverPruefen(eintrag);
        continue;
      }
      const vorhanden = releaseLesen(eintrag.tag);
      if (vorhanden?.get(eintrag.dateiName)?.replace(/\r/g, "") === inhalt.replace(/\r/g, "")) {
        console.log("  Schon eingetragen, nichts zu tun.");
        continue;
      }
      if (!ohneServer) await serverPruefen(eintrag);
      eintragen(eintrag, pfad, vorhanden);
    } catch (e) {
      fehler++;
      console.log(`::error title=Pruefsumme ${path.basename(pfad)}::${String(e.stderr || e.message).replace(/\n/g, "%0A")}`);
    }
  }
  if (fehler > 0) {
    console.error(`\n${fehler} Datei(en) nicht eingetragen.`);
    process.exit(1);
  }
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  await hauptprogramm();
}
