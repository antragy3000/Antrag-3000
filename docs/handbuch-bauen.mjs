// ============================================================
// Baut das Benutzerhandbuch-PDF aus handbuch.html – MIT Seitenzahlen
// in der Fusszeile UND echten Kapitel-Seiten im Inhaltsverzeichnis.
//
// Ablauf:
//  1. Druck (Pass 1) ueber die Edge-Druckschnittstelle (CDP) mit
//     Seitenzahl-Fusszeile in eine temporaere PDF.
//  2. Pro Seite den Text auslesen (Ghostscript) und erkennen, auf
//     welcher Seite jedes Kapitel beginnt.
//  3. Die Seitenzahlen ins Inhaltsverzeichnis von handbuch.html eintragen.
//  4. Endgueltigen Druck (Pass 2) -> Benutzerhandbuch Antrag 3000.pdf
//
// Aufruf:  node docs/handbuch-bauen.mjs
// Voraussetzung: Microsoft Edge (EdgeCore) und Ghostscript (PDF24).
// ============================================================

import { spawn, execFileSync } from "node:child_process";
import { setTimeout as sleep } from "node:timers/promises";
import { pathToFileURL, fileURLToPath } from "node:url";
import fs from "node:fs";
import path from "node:path";
import os from "node:os";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const HTML = path.join(__dirname, "handbuch.html");
const PDF = path.join(__dirname, "Benutzerhandbuch Antrag 3000.pdf");
const GS = "C:\\Program Files\\PDF24\\gs\\bin\\gswin64c.exe";

function findeEdge() {
  const base = "C:\\Program Files (x86)\\Microsoft\\EdgeCore";
  let best = null;
  if (fs.existsSync(base)) {
    for (const v of fs.readdirSync(base).sort()) {
      const p = path.join(base, v, "msedge.exe");
      if (fs.existsSync(p)) best = p; // sortiert -> neueste Version zuletzt
    }
  }
  for (const p of [
    "C:\\Program Files (x86)\\Microsoft\\Edge\\Application\\msedge.exe",
    "C:\\Program Files\\Microsoft\\Edge\\Application\\msedge.exe",
  ]) {
    if (!best && fs.existsSync(p)) best = p;
  }
  if (!best) throw new Error("Microsoft Edge nicht gefunden.");
  return best;
}

// Druckt eine HTML-Datei via Edge-CDP zu PDF, mit Seitenzahl-Fusszeile.
async function druckePDF(edge, htmlPfad, pdfPfad) {
  const port = 9100 + Math.floor(Math.random() * 800);
  const userDir = fs.mkdtempSync(path.join(os.tmpdir(), "edgepdf-"));
  const proc = spawn(edge, [
    "--headless=new", "--disable-gpu",
    `--remote-debugging-port=${port}`, `--user-data-dir=${userDir}`,
    "--no-first-run", "--no-default-browser-check", "--no-startup-window",
  ]);

  let wsUrl = null;
  for (let i = 0; i < 100; i++) {
    try {
      const r = await fetch(`http://127.0.0.1:${port}/json/version`);
      const j = await r.json();
      if (j.webSocketDebuggerUrl) { wsUrl = j.webSocketDebuggerUrl; break; }
    } catch {}
    await sleep(150);
  }
  if (!wsUrl) { proc.kill(); throw new Error("Edge-Druckschnittstelle nicht erreichbar."); }

  const ws = new WebSocket(wsUrl);
  await new Promise((res, rej) => { ws.onopen = res; ws.onerror = rej; });

  let nextId = 1;
  const pending = new Map();
  const evWaiters = [];
  ws.onmessage = (ev) => {
    const m = JSON.parse(ev.data);
    if (m.id && pending.has(m.id)) {
      const { res, rej } = pending.get(m.id);
      pending.delete(m.id);
      m.error ? rej(new Error(JSON.stringify(m.error))) : res(m.result);
      return;
    }
    for (const w of [...evWaiters]) {
      if (w.match(m)) { evWaiters.splice(evWaiters.indexOf(w), 1); w.res(m); }
    }
  };
  const send = (method, params = {}, sessionId) => {
    const id = nextId++;
    const msg = { id, method, params };
    if (sessionId) msg.sessionId = sessionId;
    ws.send(JSON.stringify(msg));
    return new Promise((res, rej) => pending.set(id, { res, rej }));
  };
  const waitEvent = (match) => new Promise((res) => evWaiters.push({ match, res }));

  const { targetId } = await send("Target.createTarget", { url: "about:blank" });
  const { sessionId } = await send("Target.attachToTarget", { targetId, flatten: true });
  await send("Page.enable", {}, sessionId);

  const loaded = waitEvent((m) => m.method === "Page.loadEventFired" && m.sessionId === sessionId);
  await send("Page.navigate", { url: pathToFileURL(htmlPfad).href }, sessionId);
  await loaded;
  await sleep(500); // Schriften/Layout setzen lassen

  const fuss =
    '<div style="width:100%;text-align:center;font-size:8px;color:#8590a2;' +
    'font-family:Segoe UI,Arial,sans-serif;"><span class="pageNumber"></span></div>';
  const { data } = await send("Page.printToPDF", {
    printBackground: true,
    preferCSSPageSize: true,
    displayHeaderFooter: true,
    headerTemplate: "<span></span>",
    footerTemplate: fuss,
  }, sessionId);

  fs.writeFileSync(pdfPfad, Buffer.from(data, "base64"));
  ws.close();
  proc.kill();
  try { fs.rmSync(userDir, { recursive: true, force: true }); } catch {}
}

// Liest den Text jeder Seite einer PDF (Ghostscript txtwrite, je Seite
// eine Datei).
function seitenTexte(pdfPfad) {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), "gstxt-"));
  execFileSync(GS, [
    "-q", "-dNOSAFER", "-dBATCH", "-dNOPAUSE", "-sDEVICE=txtwrite",
    `-sOutputFile=${path.join(dir, "p_%d.txt")}`, pdfPfad,
  ]);
  const texte = [];
  let i = 1;
  while (fs.existsSync(path.join(dir, `p_${i}.txt`))) {
    texte.push(fs.readFileSync(path.join(dir, `p_${i}.txt`)).toString("utf8"));
    i++;
  }
  try { fs.rmSync(dir, { recursive: true, force: true }); } catch {}
  return texte;
}

// Auf ASCII-Buchstaben/Ziffern reduzieren – robust gegen Umlaute und
// Encoding-Eigenheiten der Textauslese.
const norm = (s) => s.toLowerCase().replace(/&amp;/g, "&").replace(/[^a-z0-9]/g, "");

// Kapitel (Nummer + Titel) aus dem Inhaltsverzeichnis lesen.
function liesKapitel(html) {
  const re = /<li><span class="nr">(\d+)<\/span><span class="t">(.*?)<\/span><span class="pg" data-ch="\1">/g;
  const out = [];
  let m;
  while ((m = re.exec(html))) out.push({ num: m[1], key: norm(m[1] + m[2]) });
  return out;
}

// Sequentiell die Startseite jedes Kapitels finden (Ueberschrift steht
// jeweils am Seitenanfang).
function ermittleSeiten(kapitel, seiten) {
  const keys = seiten.map(norm);
  const map = {};
  let p = 0;
  for (const { num, key } of kapitel) {
    let found = -1;
    for (let i = p; i < keys.length; i++) { if (keys[i].startsWith(key)) { found = i; break; } }
    if (found < 0) for (let i = p; i < keys.length; i++) { if (keys[i].includes(key)) { found = i; break; } }
    if (found < 0) { console.warn(`  ! Kapitel ${num} nicht gefunden`); continue; }
    map[num] = found + 1;
    p = found + 1;
  }
  return map;
}

async function main() {
  const edge = findeEdge();
  console.log("Pass 1 (Seiten erkennen) ...");
  const tmpPdf = path.join(os.tmpdir(), "handbuch-pass1.pdf");
  await druckePDF(edge, HTML, tmpPdf);

  let html = fs.readFileSync(HTML, "utf8");
  const kapitel = liesKapitel(html);
  const map = ermittleSeiten(kapitel, seitenTexte(tmpPdf));
  console.log("  Seiten je Kapitel:", map);

  for (const { num } of kapitel) {
    if (!map[num]) continue;
    html = html.replace(
      new RegExp('(<span class="pg" data-ch="' + num + '">)[^<]*(</span>)'),
      `$1${map[num]}$2`
    );
  }
  fs.writeFileSync(HTML, html);

  console.log("Pass 2 (endgueltiges PDF) ...");
  await druckePDF(edge, HTML, PDF);
  try { fs.rmSync(tmpPdf, { force: true }); } catch {}
  console.log("Fertig:", PDF);
}

main().catch((e) => { console.error(e); process.exit(1); });
