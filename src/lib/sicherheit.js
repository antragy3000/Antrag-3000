// ============================================================
// Sicherheits-Schleuse fuer Daten, die aus dem NETZ ins Programm
// kommen (Team-Sync von anderen Geraeten, Foerder-Katalog vom Server).
//
// WARUM: Solche Daten sind nicht vertrauenswuerdig. Ein anderes
// Team-Geraet – oder ein manipulierter Server – koennte gezielt
// boesartige Werte einschleusen. Der gefaehrlichste Weg ist ein
// praeparierter „Link": Wuerde er ungeprueft ans Betriebssystem zum
// Oeffnen gegeben (openUrl), koennte als „Webseite" z. B.
//   file:///C:/.../programm.exe   ein Programm starten,
//   \\server\evil.lnk            eine Datei aus dem Netz starten,
//   ms-msdt: / vbscript: / search-ms:  ein Windows-Spezial-Schema
//                                  missbrauchen.
// Statt eine Webseite zu zeigen, wuerde Windows dann etwas ausfuehren.
//
// Deshalb MUSS jeder Link, der aus (potenziell fremden) Daten stammt,
// vor dem Oeffnen durch sichereWebUrl()/sichereMailUrl(). Erlaubt sind
// nur gewoehnliche Web- und Mail-Links; alles andere wird verworfen.
// ============================================================

// Steuerzeichen (Code 0x00–0x1F und 0x7F, also Zeilenumbruch/Tab/usw.)
// haben in keinem Link etwas verloren. Sie dienen sonst dazu, versteckte
// Anweisungen anzuhaengen (z. B. weitere mailto-Kopfzeilen). Wir pruefen
// ueber die Zeichen-Codes statt ueber ein Regex mit unsichtbaren Zeichen.
function hatSteuerzeichen(s) {
  for (let i = 0; i < s.length; i++) {
    const c = s.charCodeAt(i);
    if (c < 0x20 || c === 0x7f) return true;
  }
  return false;
}

function ohneSteuerzeichen(s) {
  let out = "";
  for (let i = 0; i < s.length; i++) {
    const c = s.charCodeAt(i);
    if (c >= 0x20 && c !== 0x7f) out += s[i];
  }
  return out;
}

/// Prueft einen Link aus (moeglicherweise fremden) Daten. Gibt eine
/// normalisierte http/https-Adresse zurueck, wenn es ein gewoehnlicher
/// Web-Link ist – sonst null (dann NICHT oeffnen).
export function sichereWebUrl(roh) {
  let s = (roh ?? "").toString().trim();
  if (!s || hatSteuerzeichen(s)) return null;

  // Adressen ohne Schema (z. B. „www.foo.de") wie eine Web-Adresse
  // behandeln und https:// ergaenzen – aber nur, wenn wirklich KEIN
  // Schema da ist. So wird „file:..." nicht heimlich zu „https://file:...".
  if (!/^[a-zA-Z][a-zA-Z0-9+.-]*:/.test(s)) {
    s = "https://" + s.replace(/^\/+/, "");
  }

  let u;
  try {
    u = new URL(s);
  } catch {
    return null;
  }
  // NUR Web-Schemata ans Betriebssystem geben.
  if (u.protocol !== "http:" && u.protocol !== "https:") return null;
  return u.href;
}

/// Strengere Pruefung NUR fuer den macOS-Update-Download: erlaubt ist
/// ausschliesslich https://sync.antrag3000.de/updates/mac/<datei>.dmg
/// (ohne Port, Zugangsdaten, Query, Fragment, Unterordner). WARUM: Die
/// macOS-Builds sind unsigniert; zeigte ein manipuliertes Update-Manifest auf
/// eine fremde https-Seite, gaebe es sonst keine Bremse (Befund D-04, 09/2026).
/// Der Rust-Befehl prueft dasselbe – das hier ist die zweite Sicherung.
/// Gibt die normalisierte Adresse zurueck oder null (dann NICHT oeffnen).
export function macDownloadUrl(roh) {
  const s = (roh ?? "").toString().trim();
  if (!s || hatSteuerzeichen(s)) return null;
  let u;
  try {
    u = new URL(s);
  } catch {
    return null;
  }
  const pfad = "/updates/mac/";
  const datei = u.pathname.startsWith(pfad) ? u.pathname.slice(pfad.length) : "";
  const ok =
    u.protocol === "https:" &&
    u.hostname === "sync.antrag3000.de" &&
    u.port === "" &&
    u.username === "" &&
    u.password === "" &&
    u.search === "" &&
    u.hash === "" &&
    datei.length > 0 &&
    !datei.includes("/") &&
    datei.toLowerCase().endsWith(".dmg");
  return ok ? u.href : null;
}

/// Baut aus einer E-Mail-Adresse eine sichere mailto:-URL. Die Adresse
/// wird streng geprueft (genau eine Adresse, keine Steuerzeichen/Leer-
/// raeume) und kodiert, damit niemand ueber die Adresse zusaetzliche
/// mailto-Felder (cc/bcc/body) einschleusen kann. Gibt null zurueck,
/// wenn es keine plausible Einzeladresse ist.
export function sichereMailUrl(adresse) {
  const a = (adresse ?? "").toString().trim();
  if (!a || hatSteuerzeichen(a) || /\s/.test(a)) return null;
  // Eine einfache, bewusst strenge E-Mail-Form: x@y.z, keine Kommas,
  // Semikolons, ?, & (die mailto-Felder eroeffnen wuerden).
  if (!/^[^@\s,;?&<>"']+@[^@\s,;?&<>"']+\.[^@\s,;?&<>"']+$/.test(a)) return null;
  return "mailto:" + encodeURIComponent(a);
}

/// Kappt einen Text aus fremden Daten auf eine Hoechstlaenge (gegen
/// aufgeblaehte Datensaetze) und entfernt Steuerzeichen. Gibt immer
/// einen String zurueck.
export function kappen(roh, max = 500) {
  let s = ohneSteuerzeichen((roh ?? "").toString());
  if (s.length > max) s = s.slice(0, max);
  return s;
}

/// Kontaktperson eines Programms laut Foerderer (Feld `kontakt` im Katalog,
/// gepflegt in der Foerderer-App). Fremddaten: Texte werden gekappt, die
/// E-Mail bleibt nur, wenn sie die strenge mailto-Pruefung besteht (sonst
/// leer – kein Knopf, der ins Leere oder woanders hin fuehrt). Gibt
/// {name, email, telefon} oder null, wenn nichts Brauchbares da ist.
export function foerdererKontakt(f) {
  const k = f?.kontakt;
  if (!k || typeof k !== "object" || Array.isArray(k)) return null;
  const name = kappen(k.name, 120).trim();
  const rohMail = kappen(k.email, 200).trim();
  const email = sichereMailUrl(rohMail) ? rohMail : "";
  const telefon = kappen(k.telefon, 50).trim();
  if (!name && !email && !telefon) return null;
  return { name, email, telefon };
}
