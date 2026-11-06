// ============================================================
// Kostenfinanzplan: reine Rechenregeln (Betraege, Formeln, Summen).
//
// Eigene Datei ohne Katalog-Bezug, damit Node-Tests sie laden koennen
// (siehe teilantrag.js). kfp.js reicht alles weiter, darum aendert sich
// fuer die uebrige App nichts.
// ============================================================

/// "1.234,56" | "1234,56" | "1234.56" | "2.500" -> Zahl. Ungueltig -> 0.
export function betragParsen(text) {
  if (typeof text === "number") return text;
  let s = String(text ?? "").trim().replace(/[€\s]/g, "");
  if (s === "") return 0;
  if (s.includes(",")) {
    // Deutsches Format: Punkt = Tausender, Komma = Dezimal.
    s = s.replace(/\./g, "").replace(",", ".");
  } else if (/^\d{1,3}(\.\d{3})+$/.test(s)) {
    // Reine Tausendergruppen ohne Komma: 2.500 / 1.234.567
    s = s.replace(/\./g, "");
  }
  // sonst bleibt der Punkt ein Dezimalpunkt (z. B. 80.5)
  const zahl = Number(s);
  return Number.isFinite(zahl) ? zahl : 0;
}

// Zeichen, die eine Rechnung kennzeichnen (Mal/Geteilt/Plus/Minus/Klammer).
const RECHEN_ZEICHEN = /[+\-*/×·:()xX]/;

/// Enthaelt der Text eine Rechnung (und nicht nur eine reine Zahl)?
export function istFormel(text) {
  const s = String(text ?? "");
  return RECHEN_ZEICHEN.test(s.replace(/^\s*-/, "")) && /\d/.test(s);
}

/// Wertet einen Rechenausdruck aus, z. B. "50 × 4 × 5 × 3" -> 3000.
/// × · x X und à gelten als Mal, : und / als Geteilt, + und - normal,
/// Klammern erlaubt. Woerter, €-Zeichen und Leerraum werden ignoriert.
/// Komma/Punkt als Dezimaltrennzeichen (deutsche Schreibweise).
/// Gibt { wert, fehler } zurueck.
export function formelAuswerten(text) {
  if (typeof text === "number") return { wert: text, fehler: false };
  const roh = String(text ?? "").trim();
  if (roh === "") return { wert: 0, fehler: false };

  // 1. Tokenisieren
  const tokens = [];
  const istZiffer = (c) => c >= "0" && c <= "9";
  let i = 0;
  while (i < roh.length) {
    const c = roh[i];
    if (istZiffer(c) || c === "," || c === ".") {
      let s = "";
      while (i < roh.length && (istZiffer(roh[i]) || roh[i] === "," || roh[i] === ".")) {
        s += roh[i];
        i++;
      }
      tokens.push({ t: "zahl", v: betragParsen(s) });
      continue;
    }
    if (c === "+" || c === "-") tokens.push({ t: c });
    else if (c === "*" || c === "×" || c === "·" || c === "x" || c === "X" || c === "à")
      tokens.push({ t: "*" });
    else if (c === "/" || c === ":") tokens.push({ t: "/" });
    else if (c === "(") tokens.push({ t: "(" });
    else if (c === ")") tokens.push({ t: ")" });
    // alles andere (Buchstaben, €, Leerzeichen) ignorieren
    i++;
  }
  if (tokens.length === 0) return { wert: 0, fehler: false };

  // 2. Recursive-descent-Parser mit Punkt-vor-Strich
  let pos = 0;
  let fehler = false;
  const peek = () => tokens[pos];
  const next = () => tokens[pos++];

  function ausdruck() {
    let wert = term();
    while (peek() && (peek().t === "+" || peek().t === "-")) {
      const op = next().t;
      const r = term();
      wert = op === "+" ? wert + r : wert - r;
    }
    return wert;
  }
  function term() {
    let wert = faktor();
    while (peek() && (peek().t === "*" || peek().t === "/")) {
      const op = next().t;
      const r = faktor();
      if (op === "*") wert *= r;
      else if (r === 0) {
        fehler = true;
      } else wert /= r;
    }
    return wert;
  }
  function faktor() {
    const tok = peek();
    if (!tok) {
      fehler = true;
      return 0;
    }
    if (tok.t === "(") {
      next();
      const w = ausdruck();
      if (peek() && peek().t === ")") next();
      else fehler = true;
      return w;
    }
    if (tok.t === "-") {
      next();
      return -faktor();
    }
    if (tok.t === "+") {
      next();
      return faktor();
    }
    if (tok.t === "zahl") {
      next();
      return tok.v;
    }
    fehler = true;
    next();
    return 0;
  }

  const wert = ausdruck();
  if (pos < tokens.length) fehler = true;
  return { wert, fehler };
}

/// Berechneter Betrag einer Position (Formel oder Zahl).
export function postenBetrag(p) {
  return formelAuswerten(p?.betrag).wert;
}

const FORMAT = new Intl.NumberFormat("de-DE", {
  minimumFractionDigits: 2,
  maximumFractionDigits: 2,
});

export function betragFormat(zahl) {
  return FORMAT.format(zahl) + " €";
}

export function kategorieSumme(kategorie) {
  return kategorie.posten.reduce((s, p) => s + postenBetrag(p), 0);
}

export function seitenSumme(kategorien) {
  return kategorien.reduce((s, k) => s + kategorieSumme(k), 0);
}

/// Fehlbedarf (< 0) bzw. Ueberschuss (> 0): Finanzierung - Kosten.
export function differenz(kfp) {
  return seitenSumme(kfp.finanzierung) - seitenSumme(kfp.kosten);
}
