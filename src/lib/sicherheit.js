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
