// ============================================================
// Projektnamen: Jedes Projekt braucht einen eigenen Ordner.
//
// Der Ordner eines Projekts ergibt sich allein aus seinem Namen
// (Dokumente\Antrag 3000\[Projektname]). Zwei Projekte mit demselben
// Ordnernamen würden sich also Belege und Antragsdateien teilen. Darum
// vergleicht diese Datei nicht die Namen selbst, sondern die Ordner, die
// daraus entstehen: „Konzert:Mai", „Konzert_Mai" und „konzert_mai" landen
// unter Windows alle im selben Ordner.
//
// ordnerName() muss zu bereinigen() in src-tauri/src/ordner.rs passen.
// Reine Logik ohne Svelte – darum mit `npm run projektname` testbar.
// ============================================================

const VERBOTEN = /[<>:"/\\|?*\u0000-\u001f]/g;
const RESERVIERT = /^(CON|PRN|AUX|NUL|COM[1-9]|LPT[1-9])$/i;

/// Der Ordnername, den der Rust-Kern aus einem freien Namen macht. Leer, wenn
/// daraus kein gültiger Ordnername entsteht (z. B. nur Punkte).
/** @param {any} name */
export function ordnerName(name) {
  const sauber = String(name ?? "")
    .replace(VERBOTEN, "_")
    .trim()
    .replace(/\.+$/, "")
    .trim();
  if (!sauber) return "";
  const basis = sauber.split(".")[0].trimEnd();
  return RESERVIERT.test(basis) ? `_${sauber}` : sauber;
}

/// Vergleichswert: Windows unterscheidet Groß- und Kleinschreibung bei
/// Ordnern nicht.
/** @param {any} name */
function ordnerSchluessel(name) {
  return ordnerName(name).toLowerCase();
}

/// Prüft einen Namen fürs Anlegen oder (mit `eigenes`) fürs Umbenennen.
/// Auch archivierte Projekte belegen ihren Ordner. Bleibt der Ordner beim
/// Umbenennen derselbe, ist der Name immer erlaubt, damit sich Projekte aus
/// älteren Ständen mit doppeltem Namen weiter bearbeiten lassen.
/// Liefert { ok, name, fehler }; `fehler` ist bei leerer Eingabe leer, damit
/// der Dialog vor dem ersten Tippen nicht schon meckert.
/**
 * @param {any} eingabe
 * @param {any[]} projekte
 * @param {any} [eigenes]
 */
export function projektNamePruefen(eingabe, projekte, eigenes = null) {
  const name = String(eingabe ?? "").trim();
  if (!name) return { ok: false, name, fehler: "" };
  const schluessel = ordnerSchluessel(name);
  if (!schluessel) {
    return { ok: false, name, fehler: "Aus diesem Namen entsteht kein gültiger Ordnername." };
  }
  if (eigenes && ordnerSchluessel(eigenes.name) === schluessel) {
    return { ok: true, name, fehler: "" };
  }
  const anderes = (projekte ?? []).find(
    (p) => p?.id !== eigenes?.id && ordnerSchluessel(p?.name) === schluessel
  );
  if (anderes) {
    const archiv = anderes.archiviert ? " im Archiv" : "";
    return {
      ok: false,
      name,
      fehler: `Das Projekt „${anderes.name}"${archiv} nutzt schon denselben Ordner. Bitte wähle einen anderen Namen.`,
    };
  }
  return { ok: true, name, fehler: "" };
}
