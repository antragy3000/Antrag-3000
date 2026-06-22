// ============================================================
// Orte der DACH-Region: Bundesländer/Kantone (vollständig) und Städte
// (kuratiert, ≥ ~50.000 Einw., leicht erweiterbar).
//
// Genutzt vom Fragebogen (engere Auswahl bei Wohnsitz / Durchführungsort)
// und vom Matching. Die Stadt-Liste ist bewusst als einfaches Array
// aufgebaut – neue Städte einfach unter dem passenden Regions-Code
// ergänzen. Region-Codes folgen den üblichen Kürzeln (DE: ISO-Land-
// teile, AT: 1–9, CH: Kantonskürzel).
// ============================================================

/** Bundesländer (DE, AT) und Kantone (CH). Vollständig. */
export const REGIONEN = {
  DE: [
    { code: "BW", name: "Baden-Württemberg" },
    { code: "BY", name: "Bayern" },
    { code: "BE", name: "Berlin" },
    { code: "BB", name: "Brandenburg" },
    { code: "HB", name: "Bremen" },
    { code: "HH", name: "Hamburg" },
    { code: "HE", name: "Hessen" },
    { code: "MV", name: "Mecklenburg-Vorpommern" },
    { code: "NI", name: "Niedersachsen" },
    { code: "NW", name: "Nordrhein-Westfalen" },
    { code: "RP", name: "Rheinland-Pfalz" },
    { code: "SL", name: "Saarland" },
    { code: "SN", name: "Sachsen" },
    { code: "ST", name: "Sachsen-Anhalt" },
    { code: "SH", name: "Schleswig-Holstein" },
    { code: "TH", name: "Thüringen" },
  ],
  AT: [
    { code: "BGL", name: "Burgenland" },
    { code: "KTN", name: "Kärnten" },
    { code: "NOE", name: "Niederösterreich" },
    { code: "OOE", name: "Oberösterreich" },
    { code: "SBG", name: "Salzburg" },
    { code: "STMK", name: "Steiermark" },
    { code: "TIR", name: "Tirol" },
    { code: "VBG", name: "Vorarlberg" },
    { code: "WIEN", name: "Wien" },
  ],
  CH: [
    { code: "AG", name: "Aargau" },
    { code: "AI", name: "Appenzell Innerrhoden" },
    { code: "AR", name: "Appenzell Ausserrhoden" },
    { code: "BE", name: "Bern" },
    { code: "BL", name: "Basel-Landschaft" },
    { code: "BS", name: "Basel-Stadt" },
    { code: "FR", name: "Freiburg" },
    { code: "GE", name: "Genf" },
    { code: "GL", name: "Glarus" },
    { code: "GR", name: "Graubünden" },
    { code: "JU", name: "Jura" },
    { code: "LU", name: "Luzern" },
    { code: "NE", name: "Neuenburg" },
    { code: "NW", name: "Nidwalden" },
    { code: "OW", name: "Obwalden" },
    { code: "SG", name: "St. Gallen" },
    { code: "SH", name: "Schaffhausen" },
    { code: "SO", name: "Solothurn" },
    { code: "SZ", name: "Schwyz" },
    { code: "TG", name: "Thurgau" },
    { code: "TI", name: "Tessin" },
    { code: "UR", name: "Uri" },
    { code: "VD", name: "Waadt" },
    { code: "VS", name: "Wallis" },
    { code: "ZG", name: "Zug" },
    { code: "ZH", name: "Zürich" },
  ],
};

// Städte je Region-Code (kuratiert, überwiegend ≥ 50.000 Einw.).
// Ergänzen: einfach einen Namen unter dem passenden Code hinzufügen.
const STAEDTE_DE = {
  BW: ["Stuttgart", "Mannheim", "Karlsruhe", "Freiburg im Breisgau", "Heidelberg",
    "Heilbronn", "Ulm", "Pforzheim", "Reutlingen", "Esslingen am Neckar",
    "Ludwigsburg", "Tübingen", "Villingen-Schwenningen", "Konstanz", "Sindelfingen",
    "Aalen", "Friedrichshafen", "Offenburg", "Göppingen", "Schwäbisch Gmünd"],
  BY: ["München", "Nürnberg", "Augsburg", "Regensburg", "Ingolstadt", "Würzburg",
    "Fürth", "Erlangen", "Bamberg", "Bayreuth", "Landshut", "Aschaffenburg",
    "Kempten (Allgäu)", "Rosenheim", "Schweinfurt", "Neu-Ulm", "Passau"],
  BE: ["Berlin"],
  BB: ["Potsdam", "Cottbus", "Brandenburg an der Havel"],
  HB: ["Bremen", "Bremerhaven"],
  HH: ["Hamburg"],
  HE: ["Frankfurt am Main", "Wiesbaden", "Kassel", "Darmstadt", "Offenbach am Main",
    "Hanau", "Gießen", "Marburg", "Fulda", "Rüsselsheim am Main", "Wetzlar"],
  MV: ["Rostock", "Schwerin", "Neubrandenburg", "Stralsund", "Greifswald"],
  NI: ["Hannover", "Braunschweig", "Oldenburg", "Osnabrück", "Wolfsburg",
    "Göttingen", "Salzgitter", "Hildesheim", "Delmenhorst", "Wilhelmshaven",
    "Lüneburg", "Celle", "Hameln"],
  NW: ["Köln", "Düsseldorf", "Dortmund", "Essen", "Duisburg", "Bochum", "Wuppertal",
    "Bielefeld", "Bonn", "Münster", "Mönchengladbach", "Gelsenkirchen", "Aachen",
    "Krefeld", "Oberhausen", "Hagen", "Hamm", "Mülheim an der Ruhr", "Leverkusen",
    "Solingen", "Herne", "Neuss", "Paderborn", "Bottrop", "Recklinghausen",
    "Bergisch Gladbach", "Remscheid", "Moers", "Siegen", "Gütersloh", "Witten",
    "Iserlohn", "Düren", "Ratingen", "Lünen", "Marl", "Velbert", "Minden",
    "Dorsten", "Castrop-Rauxel", "Gladbeck", "Bocholt", "Detmold", "Lüdenscheid"],
  RP: ["Mainz", "Ludwigshafen am Rhein", "Koblenz", "Trier", "Kaiserslautern",
    "Worms", "Neuwied"],
  SL: ["Saarbrücken", "Neunkirchen"],
  SN: ["Leipzig", "Dresden", "Chemnitz", "Zwickau", "Plauen", "Görlitz"],
  ST: ["Halle (Saale)", "Magdeburg", "Dessau-Roßlau", "Wittenberg", "Halberstadt"],
  SH: ["Kiel", "Lübeck", "Flensburg", "Neumünster", "Norderstedt", "Elmshorn"],
  TH: ["Erfurt", "Jena", "Gera", "Weimar", "Gotha", "Nordhausen"],
};

const STAEDTE_AT = {
  WIEN: ["Wien"],
  STMK: ["Graz", "Leoben"],
  OOE: ["Linz", "Wels", "Steyr"],
  SBG: ["Salzburg"],
  TIR: ["Innsbruck"],
  KTN: ["Klagenfurt am Wörthersee", "Villach"],
  VBG: ["Dornbirn", "Feldkirch", "Bregenz"],
  NOE: ["St. Pölten", "Wiener Neustadt", "Krems an der Donau", "Baden"],
  BGL: ["Eisenstadt"],
};

const STAEDTE_CH = {
  ZH: ["Zürich", "Winterthur", "Uster", "Dübendorf"],
  GE: ["Genf"],
  BS: ["Basel"],
  VD: ["Lausanne", "Yverdon-les-Bains", "Montreux"],
  BE: ["Bern", "Biel/Bienne", "Thun", "Köniz"],
  LU: ["Luzern", "Emmen"],
  SG: ["St. Gallen", "Rapperswil-Jona"],
  TI: ["Lugano", "Bellinzona", "Locarno"],
  AG: ["Aarau", "Wettingen", "Baden"],
  SO: ["Olten", "Solothurn"],
  ZG: ["Zug"],
  FR: ["Freiburg", "Bulle"],
  NE: ["Neuenburg", "La Chaux-de-Fonds"],
  TG: ["Frauenfeld", "Kreuzlingen"],
  VS: ["Sitten", "Siders"],
  SH: ["Schaffhausen"],
  GR: ["Chur"],
};

/** Flache Liste aller Städte: { name, land, region }. */
export const STAEDTE = Object.entries({
  ...mitLand(STAEDTE_DE, "DE"),
  ...mitLand(STAEDTE_AT, "AT"),
  ...mitLand(STAEDTE_CH, "CH"),
}).flatMap(([key, eintraege]) => eintraege);

function mitLand(tabelle, land) {
  const out = {};
  for (const [region, namen] of Object.entries(tabelle)) {
    out[`${land}:${region}`] = namen.map((name) => ({ name, land, region }));
  }
  return out;
}

/** Regionen eines Landes (oder leere Liste). */
export function regionenFuer(land) {
  return REGIONEN[land] ?? [];
}

/** Klartext-Name einer Region (Land + Code). */
export function regionName(land, code) {
  return (REGIONEN[land] ?? []).find((r) => r.code === code)?.name ?? code ?? "";
}

// Hilfsfunktion für die normalisierte Suche (ohne Umlaut-/Akzent-Stolpern).
function norm(s) {
  return (s ?? "")
    .toLowerCase()
    .normalize("NFD")
    .replace(/[̀-ͯ]/g, "");
}

/**
 * Sucht Regionen passend zum Land und Suchtext. Leeres Land => leer
 * (eine Region ergibt nur mit gewähltem Land Sinn).
 */
export function sucheRegionen(land, text) {
  const t = norm(text);
  return regionenFuer(land).filter((r) => !t || norm(r.name).includes(t));
}

/**
 * Sucht Städte – immer eingeschränkt auf das gewählte Land und (falls
 * gesetzt) die gewählte Region. So erscheint z. B. bei Österreich nie
 * Berlin. Begrenzt die Trefferzahl für eine flüssige Anzeige.
 */
export function sucheStaedte(land, regionCode, text, grenze = 30) {
  const t = norm(text);
  return STAEDTE.filter(
    (s) =>
      (!land || s.land === land) &&
      (!regionCode || s.region === regionCode) &&
      (!t || norm(s.name).includes(t)),
  )
    .sort((a, b) => a.name.localeCompare(b.name, "de"))
    .slice(0, grenze);
}
