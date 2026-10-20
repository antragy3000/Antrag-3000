// ============================================================
// Arbeitsdokumente: Links zu Texten, die ein Team gemeinsam außerhalb
// der App schreibt (z. B. in CryptPad), plus ein geteilter Ablageort
// für Dateien. Reine Funktionen (ohne Svelte), damit sie mit
// `npm run sicherheit` testbar sind.
//
// Datenmodell je Projekt (im verschlüsselten Tresor):
//   projekt.arbeitsdokumente = [{ id, titel, url, foerderungen: [id…] }]
//   projekt.ablageUrl        = ""   (Link zum geteilten Ablageort)
// Jedes Dokument steht genau EINMAL in der Projekt-Liste und kann
// einzelnen Förderungen ZUGEORDNET werden (keine Kopien).
//
// DATENSOUVERÄNITÄT: Die App speichert nur Titel + Link; die Texte und
// Dateien selbst laufen nie durch Antrag 3000. Die Links gehen auch NICHT
// ins Team-Board (sync.js liest sie nicht): ein CryptPad-Link enthält den
// Schlüssel zum Dokument – geteilt wird direkt im jeweiligen Dienst.
// ============================================================

import { sichereWebUrl, kappen } from "./sicherheit.js";

export const TITEL_MAX = 120;
export const LINK_MAX = 2000;

function neueDokId() {
  return "dok-" + (globalThis.crypto?.randomUUID
    ? globalThis.crypto.randomUUID()
    : Date.now().toString(36) + Math.random().toString(36).slice(2, 10));
}

/// Prüft einen eingegebenen Link: nur https (Adressen ohne Schema
/// bekommen https:// vorangestellt). Gibt die bereinigte Adresse oder null.
export function linkPruefen(roh) {
  const s = (roh ?? "").toString().trim();
  if (!s || s.length > LINK_MAX) return null;
  const url = sichereWebUrl(s);
  return url && url.startsWith("https://") ? url : null;
}

/// Prüft Titel + Link aus dem Dialog. {ok, titel, url} oder {ok:false, fehler}.
export function dokumentPruefen(titel, link) {
  const t = kappen(titel, TITEL_MAX).trim();
  if (!t) return { ok: false, fehler: "Bitte einen Titel angeben." };
  if (!(link ?? "").toString().trim()) return { ok: false, fehler: "Bitte einen Link angeben." };
  const url = linkPruefen(link);
  if (!url) {
    return { ok: false, fehler: "Bitte einen vollständigen https-Link angeben (z. B. https://cryptpad.fr/…)." };
  }
  return { ok: true, titel: t, url };
}

function liste(projekt) {
  if (!Array.isArray(projekt.arbeitsdokumente)) projekt.arbeitsdokumente = [];
  return projekt.arbeitsdokumente;
}

/// Legt ein Dokument in der Projekt-Liste an (optional gleich einer
/// Förderung zugeordnet) und gibt es zurück.
export function dokumentAnlegen(projekt, titel, url, foerderungId = null) {
  const dok = { id: neueDokId(), titel, url, foerderungen: foerderungId ? [foerderungId] : [] };
  liste(projekt).push(dok);
  return dok;
}

export function dokumentAendern(projekt, id, titel, url) {
  const dok = liste(projekt).find((d) => d.id === id);
  if (!dok) return false;
  dok.titel = titel;
  dok.url = url;
  return true;
}

/// Löscht ein Dokument ganz – damit auch alle seine Zuordnungen.
export function dokumentLoeschen(projekt, id) {
  const l = liste(projekt);
  const i = l.findIndex((d) => d.id === id);
  if (i < 0) return false;
  l.splice(i, 1);
  return true;
}

export function zuordnen(projekt, id, foerderungId) {
  const dok = liste(projekt).find((d) => d.id === id);
  if (!dok || !foerderungId) return false;
  if (!Array.isArray(dok.foerderungen)) dok.foerderungen = [];
  if (!dok.foerderungen.includes(foerderungId)) dok.foerderungen.push(foerderungId);
  return true;
}

/// Löst nur die Zuordnung – das Dokument bleibt in der Projekt-Liste.
export function zuordnungLoesen(projekt, id, foerderungId) {
  const dok = liste(projekt).find((d) => d.id === id);
  if (!dok || !Array.isArray(dok.foerderungen)) return false;
  const vorher = dok.foerderungen.length;
  dok.foerderungen = dok.foerderungen.filter((f) => f !== foerderungId);
  return dok.foerderungen.length !== vorher;
}

/// Dokumente, die einer Förderung zugeordnet sind.
export function fuerFoerderung(projekt, foerderungId) {
  return (projekt?.arbeitsdokumente ?? []).filter((d) => (d.foerderungen ?? []).includes(foerderungId));
}

/// Dokumente des Projekts, die dieser Förderung noch NICHT zugeordnet sind
/// (Auswahl für „Aus dem Projekt übernehmen").
export function nochNichtZugeordnet(projekt, foerderungId) {
  return (projekt?.arbeitsdokumente ?? []).filter((d) => !(d.foerderungen ?? []).includes(foerderungId));
}

/// Kurzer Host-Name für die Anzeige („cryptpad.fr"), sonst "".
export function hostVon(url) {
  try {
    return new URL(url).hostname.replace(/^www\./, "");
  } catch {
    return "";
  }
}
