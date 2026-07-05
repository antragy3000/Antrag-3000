<script>
  // Fristen / Kalender: die Einreichfristen der gemerkten Förderungen,
  // nach Datum sortiert, mit Countdown ("noch X Tage") und Antrag-
  // Status. Klick auf eine Zeile öffnet die Detailansicht.
  import FoerderDetail from "./FoerderDetail.svelte";
  import { LAENDER, fristText, fristNormalisieren, fristAlsDatum } from "$lib/begriffe";
  import {
    ANTRAG_STATUS,
    ANTRAG_STANDARD,
    statusLabel,
    statusFarbe,
  } from "$lib/status";

  let {
    foerderungen = [],
    hinweis = "",
    merkliste,
    umschalten,
    ordnerOeffnen = null,
    dokumentHochladen = null,
    antragsPdfVorschau = null,
    antragsPdfSpeichern = null,
    antraege = {},
    antragHolen = null,
    antragSpeichern = null,
    interneFristen = [],
    interneAnlegen = null,
    interneEntfernen = null,
    // Team-Kalender: die Fristen der anderen Team-Geräte (aus dem Board).
    teamBoard = null,
    meineProjektIds = [],
    foerderungLabel = null,
    logoHerunterladen = null,
  } = $props();

  let ausgewaehlt = $state(null);
  let aktuellerAntrag = $state(null);

  // Eingabe für interne Fristen (ohne Förderung)
  let interneOffen = $state(false);
  let neuDatum = $state("");
  let neuTitel = $state("");

  async function interneSpeichern(event) {
    event.preventDefault();
    if (!neuDatum) return;
    await interneAnlegen({ datum: neuDatum, titel: neuTitel });
    neuDatum = "";
    neuTitel = "";
    interneOffen = false;
  }

  const HEUTE = new Date();
  HEUTE.setHours(0, 0, 0, 0);

  function tageBis(d) {
    return Math.round((new Date(d) - HEUTE) / 86400000);
  }
  function fmtDatum(d) {
    return new Date(d).toLocaleDateString("de-DE");
  }
  function tag(d) {
    return new Date(d).toLocaleDateString("de-DE", { day: "2-digit" });
  }
  function monat(d) {
    return new Date(d).toLocaleDateString("de-DE", { month: "short" });
  }
  function jahr(d) {
    return new Date(d).getFullYear();
  }

  let gemerkte = $derived(
    merkliste
      .map((id) => foerderungen.find((f) => f.id === id))
      .filter(Boolean)
  );

  // Datum -> "JJJJ-MM-TT" (lokal), damit tag()/monat()/jahr() es parsen können.
  function isoVon(dt) {
    const m = String(dt.getMonth() + 1).padStart(2, "0");
    const t = String(dt.getDate()).padStart(2, "0");
    return `${dt.getFullYear()}-${m}-${t}`;
  }

  // Die OFFIZIELLEN Frist-Datumswerte einer Förderung (editierbare Übernahme
  // aus der Datenbank). Wiederkehrende Daten ohne Jahr werden auf das nächste
  // konkrete Vorkommen aufgelöst. Diese bestimmen die Hauptfrist der Zeile.
  function fristenVon(f) {
    const a = antraege[f.id];
    const roh = a?.offizielleFristen ?? f.fristen ?? [];
    return roh
      .map((e) => fristAlsDatum(fristNormalisieren(e).datum))
      .filter(Boolean)
      .map(isoVon);
  }

  // Die in der Detailansicht eingetragenen EIGENEN Fristen einer Förderung,
  // nach Datum sortiert – werden als Unterfristen unter der Zeile gezeigt.
  function eigeneFristenVon(f) {
    const a = antraege[f.id];
    return (a?.eigeneFristen ?? [])
      .map((e) => {
        const datum = typeof e === "string" ? e : e.datum;
        const titel = typeof e === "string" ? "" : e.titel || "";
        return datum ? { datum, titel, tage: tageBis(datum) } : null;
      })
      .filter(Boolean)
      .sort((x, y) => new Date(x.datum) - new Date(y.datum));
  }

  // Dringlichkeitsfarbe einer Unterfrist (vergangene = grau).
  function ufFarbe(tage) {
    return tage < 0 ? "grau" : dringlichkeit(tage);
  }

  // Pro Förderung: nächste zukünftige bzw. letzte vergangene Frist.
  function fristInfo(f) {
    // laufend ODER periodisch (wiederkehrend) = es gibt immer eine nächste
    // Möglichkeit; ohne konkretes zukünftiges Datum landen beide im
    // „immer offen"-Topf statt unter „vergangen".
    const z = f.weiche_kriterien.zeitpunkt;
    const immerOffen = z === "laufend" || z === "periodisch";
    const mitTagen = fristenVon(f).map((d) => ({ d, t: tageBis(d) }));
    const zukunft = mitTagen.filter((x) => x.t >= 0).sort((a, b) => a.t - b.t);
    const verg = mitTagen.filter((x) => x.t < 0).sort((a, b) => b.t - a.t);
    if (zukunft.length) return { typ: "anstehend", frist: zukunft[0].d, tage: zukunft[0].t };
    if (immerOffen) return { typ: "laufend" };
    if (verg.length) return { typ: "vergangen", frist: verg[0].d, tage: verg[0].t };
    return { typ: "offen" }; // keine feste Frist hinterlegt
  }

  // Gehört eine Board-Projekt-ID zu DIESEM Gerät? (Dann nicht doppelt zeigen –
  // die eigenen Termine kommen schon aus dem lokalen Tresor.)
  function istEigenes(projektId) {
    return (meineProjektIds ?? []).includes(projektId);
  }

  // Team-Termine: alle konkreten Fristen der ANDEREN Team-Geräte aus dem
  // Board – offizielle und eigene Fristen je Förderung sowie interne Fristen.
  // Jede Frist wird ein eigener (flacher) Termin mit Projekt-Kontext.
  let teamTermine = $derived.by(() => {
    const liste = [];
    for (const e of teamBoard ?? []) {
      if (istEigenes(e.projekt_id)) continue;
      const projektName = e.inhalt?.name || "Projekt";
      for (const f of e.inhalt?.eintraege ?? []) {
        const fname = foerderungLabel ? foerderungLabel(f) : f.eigenesLabel || "Förderung";
        for (const d of f.offizielleFristen ?? []) {
          const dt = fristAlsDatum(fristNormalisieren(d).datum);
          if (!dt) continue;
          const iso = isoVon(dt);
          liste.push({ kind: "team", id: `${e.projekt_id}|${f.foerderungId}|of|${iso}`, projektName, titel: fname, untertitel: "", frist: iso, tage: tageBis(iso) });
        }
        for (const ef of f.eigeneFristen ?? []) {
          const datum = typeof ef === "string" ? ef : ef?.datum;
          if (!datum) continue;
          const titel = typeof ef === "string" ? "" : ef.titel || "";
          liste.push({ kind: "team", id: `${e.projekt_id}|${f.foerderungId}|ef|${datum}|${titel}`, projektName, titel: fname, untertitel: titel || "Eigene Frist", frist: datum, tage: tageBis(datum) });
        }
      }
      for (const t of e.inhalt?.interneFristen ?? []) {
        if (!t?.datum) continue;
        liste.push({ kind: "team", id: `${e.projekt_id}|int|${t.id || t.datum}`, projektName, titel: t.titel || "Interne Frist", untertitel: "", frist: t.datum, tage: tageBis(t.datum) });
      }
    }
    return liste;
  });

  // Alle Termine: Förderungs-Fristen plus interne Fristen plus Team-Termine.
  let termine = $derived.by(() => {
    const liste = [];
    for (const f of gemerkte) {
      const info = fristInfo(f);
      if (info.typ === "anstehend" || info.typ === "vergangen") {
        liste.push({ kind: "foerderung", id: f.id, f, frist: info.frist, tage: info.tage });
      }
    }
    for (const t of interneFristen) {
      liste.push({ kind: "intern", id: t.id, titel: t.titel, frist: t.datum, tage: tageBis(t.datum) });
    }
    for (const tt of teamTermine) liste.push(tt);
    return liste;
  });

  let anstehend = $derived(termine.filter((t) => t.tage >= 0).sort((a, b) => a.tage - b.tage));
  let vergangen = $derived(termine.filter((t) => t.tage < 0).sort((a, b) => b.tage - a.tage));
  let ohneFrist = $derived(
    gemerkte
      .map((f) => ({ f, info: fristInfo(f) }))
      .filter((x) => x.info.typ === "laufend" || x.info.typ === "offen")
  );

  function dringlichkeit(tage) {
    if (tage <= 14) return "rot";
    if (tage <= 30) return "gelb";
    return "gruen";
  }
  function countdownText(tage) {
    if (tage === 0) return "heute!";
    if (tage === 1) return "morgen";
    return `noch ${tage} Tage`;
  }

  function badgeFuer(id) {
    const a = antraege[id];
    const key = a?.status ?? ANTRAG_STANDARD;
    return {
      label: statusLabel(ANTRAG_STATUS, key, a?.statusFrei),
      farbe: statusFarbe(ANTRAG_STATUS, key),
    };
  }

  function oeffnen(f) {
    aktuellerAntrag = antragHolen ? antragHolen(f) : null;
    ausgewaehlt = f;
  }
</script>

<div class="bereich">
  <div class="kopfzeile">
    <h2>Fristen <span class="anzahl">{anstehend.length} anstehend</span></h2>
    {#if interneAnlegen}
      <button class="ordner" onclick={() => (interneOffen = !interneOffen)}>
        + Interne Frist
      </button>
    {/if}
  </div>

  {#if interneOffen}
    <form class="interne-form" onsubmit={interneSpeichern}>
      <input type="date" bind:value={neuDatum} />
      <input type="text" placeholder="Titel (z. B. Teamabgabe Entwurf)" bind:value={neuTitel} />
      <button type="submit" class="primaer" disabled={!neuDatum}>Hinzufügen</button>
      <button type="button" class="leise" onclick={() => (interneOffen = false)}>Abbrechen</button>
    </form>
  {/if}

  {#if anstehend.length === 0 && ohneFrist.length === 0 && vergangen.length === 0}
    <p class="leer">
      Noch keine Fristen. Merke Förderungen mit Einreichfrist oder lege oben
      eine interne Frist an.
    </p>
  {/if}

  {#snippet unterfristen(eigene)}
    {#if eigene.length}
      <ul class="unterfristen">
        {#each eigene as uf (uf.datum + uf.titel)}
          <li class="farbe-{ufFarbe(uf.tage)}">
            <span class="uf-pin">↳</span>
            <span class="uf-datum">{fmtDatum(uf.datum)}</span>
            <span class="uf-titel">{uf.titel || "Eigene Frist"}</span>
            <span class="uf-cd">{uf.tage >= 0 ? countdownText(uf.tage) : "vor " + Math.abs(uf.tage) + " Tagen"}</span>
          </li>
        {/each}
      </ul>
    {/if}
  {/snippet}

  {#if anstehend.length}
    <div class="liste">
      {#each anstehend as t (t.kind + t.id)}
        {#if t.kind === "foerderung"}
          {@const badge = badgeFuer(t.f.id)}
          <div
            class="zeile"
            role="button"
            tabindex="0"
            onclick={() => oeffnen(t.f)}
            onkeydown={(e) => {
              if (e.key === "Enter" || e.key === " ") {
                e.preventDefault();
                oeffnen(t.f);
              }
            }}
          >
            <div class="datum farbe-{dringlichkeit(t.tage)}">
              <span class="tag">{tag(t.frist)}</span>
              <span class="monat">{monat(t.frist)}</span>
              <span class="jahr">{jahr(t.frist)}</span>
            </div>
            <div class="haupt">
              <div class="kopf">
                <span class="land land-{t.f.land}">{LAENDER[t.f.land] ?? t.f.land}</span>
                <h3>{t.f.name}</h3>
                <span class="status-badge farbe-{badge.farbe}">
                  <span class="punkt"></span>{badge.label}
                </span>
              </div>
              <p class="meta">{t.f.foerdergeber}</p>
              {@render unterfristen(eigeneFristenVon(t.f))}
            </div>
            <div class="countdown farbe-{dringlichkeit(t.tage)}">{countdownText(t.tage)}</div>
          </div>
        {:else if t.kind === "team"}
          <div class="zeile team schlicht">
            <div class="datum farbe-{dringlichkeit(t.tage)}">
              <span class="tag">{tag(t.frist)}</span>
              <span class="monat">{monat(t.frist)}</span>
              <span class="jahr">{jahr(t.frist)}</span>
            </div>
            <div class="haupt">
              <div class="kopf">
                <span class="team-tag">👥 Team</span>
                <h3>{t.titel}{#if t.untertitel} – {t.untertitel}{/if}</h3>
              </div>
              <p class="meta">{t.projektName}</p>
            </div>
            <div class="countdown farbe-{dringlichkeit(t.tage)}">{countdownText(t.tage)}</div>
          </div>
        {:else}
          <div class="zeile intern">
            <div class="datum farbe-{dringlichkeit(t.tage)}">
              <span class="tag">{tag(t.frist)}</span>
              <span class="monat">{monat(t.frist)}</span>
              <span class="jahr">{jahr(t.frist)}</span>
            </div>
            <div class="haupt">
              <div class="kopf">
                <span class="intern-tag">📌 Intern</span>
                <h3>{t.titel}</h3>
              </div>
            </div>
            <div class="countdown farbe-{dringlichkeit(t.tage)}">{countdownText(t.tage)}</div>
            {#if interneEntfernen}
              <button class="entfernen" title="Interne Frist entfernen" onclick={() => interneEntfernen(t.id)}>✕</button>
            {/if}
          </div>
        {/if}
      {/each}
    </div>
  {/if}

  {#if ohneFrist.length}
    <h3 class="gruppe">Laufend / ohne feste Frist</h3>
    <div class="liste">
      {#each ohneFrist as { f } (f.id)}
        <div class="zeile schlicht" role="button" tabindex="0"
          onclick={() => oeffnen(f)}
          onkeydown={(e) => { if (e.key === "Enter" || e.key === " ") { e.preventDefault(); oeffnen(f); } }}
        >
          <div class="datum farbe-grau"><span class="infinity">∞</span></div>
          <div class="haupt">
            <div class="kopf">
              <span class="land land-{f.land}">{LAENDER[f.land] ?? f.land}</span>
              <h3>{f.name}</h3>
            </div>
            <p class="meta">{f.foerdergeber} · {fristText(f)}</p>
            {@render unterfristen(eigeneFristenVon(f))}
          </div>
        </div>
      {/each}
    </div>
  {/if}

  {#if vergangen.length}
    <h3 class="gruppe">Bereits vergangen</h3>
    <div class="liste">
      {#each vergangen as t (t.kind + t.id)}
        {#if t.kind === "foerderung"}
          <div class="zeile schlicht verg" role="button" tabindex="0"
            onclick={() => oeffnen(t.f)}
            onkeydown={(e) => { if (e.key === "Enter" || e.key === " ") { e.preventDefault(); oeffnen(t.f); } }}
          >
            <div class="datum farbe-grau">
              <span class="tag">{tag(t.frist)}</span>
              <span class="monat">{monat(t.frist)}</span>
              <span class="jahr">{jahr(t.frist)}</span>
            </div>
            <div class="haupt">
              <div class="kopf">
                <span class="land land-{t.f.land}">{LAENDER[t.f.land] ?? t.f.land}</span>
                <h3>{t.f.name}</h3>
              </div>
              <p class="meta">{t.f.foerdergeber} · vor {Math.abs(t.tage)} Tagen</p>
              {@render unterfristen(eigeneFristenVon(t.f))}
            </div>
          </div>
        {:else if t.kind === "team"}
          <div class="zeile team schlicht verg">
            <div class="datum farbe-grau">
              <span class="tag">{tag(t.frist)}</span>
              <span class="monat">{monat(t.frist)}</span>
              <span class="jahr">{jahr(t.frist)}</span>
            </div>
            <div class="haupt">
              <div class="kopf">
                <span class="team-tag">👥 Team</span>
                <h3>{t.titel}{#if t.untertitel} – {t.untertitel}{/if}</h3>
              </div>
              <p class="meta">{t.projektName} · vor {Math.abs(t.tage)} Tagen</p>
            </div>
          </div>
        {:else}
          <div class="zeile schlicht verg intern">
            <div class="datum farbe-grau">
              <span class="tag">{tag(t.frist)}</span>
              <span class="monat">{monat(t.frist)}</span>
              <span class="jahr">{jahr(t.frist)}</span>
            </div>
            <div class="haupt">
              <div class="kopf">
                <span class="intern-tag">📌 Intern</span>
                <h3>{t.titel}</h3>
              </div>
              <p class="meta">vor {Math.abs(t.tage)} Tagen</p>
            </div>
            {#if interneEntfernen}
              <button class="entfernen" title="Interne Frist entfernen" onclick={() => interneEntfernen(t.id)}>✕</button>
            {/if}
          </div>
        {/if}
      {/each}
    </div>
  {/if}
</div>

{#if ausgewaehlt}
  <FoerderDetail
    foerderung={ausgewaehlt}
    alle={foerderungen}
    hinweis={hinweis}
    gemerkt={merkliste.includes(ausgewaehlt.id)}
    umschalten={umschalten}
    ordnerOeffnen={ordnerOeffnen ? () => ordnerOeffnen(ausgewaehlt.name) : null}
    hochladen={dokumentHochladen ? (art) => dokumentHochladen(ausgewaehlt.name, art) : null}
    pdfVorschau={antragsPdfVorschau ? () => antragsPdfVorschau(ausgewaehlt) : null}
    pdfSpeichern={antragsPdfSpeichern ? () => antragsPdfSpeichern(ausgewaehlt) : null}
    {logoHerunterladen}
    antrag={aktuellerAntrag}
    antragAendern={antragSpeichern}
    schliessen={() => {
      ausgewaehlt = null;
      aktuellerAntrag = null;
    }}
  />
{/if}

<style>
  .bereich {
    max-width: 1080px;
    margin: 0 auto;
    padding: 32px 24px 64px;
  }
  h2 {
    margin: 0 0 20px;
    font-size: 1.35rem;
    font-weight: 600;
  }
  .anzahl {
    color: var(--text-muted);
    font-size: 0.9rem;
    font-weight: 400;
    margin-left: 8px;
  }
  .gruppe {
    margin: 32px 0 12px;
    font-size: 1.05rem;
    font-weight: 600;
    color: var(--text-2);
  }
  .leer {
    color: var(--text-muted);
  }

  .kopfzeile {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 16px;
    flex-wrap: wrap;
    margin-bottom: 20px;
  }
  .kopfzeile h2 {
    margin: 0;
  }
  .ordner {
    padding: 9px 16px;
    font-size: 0.9rem;
    font-weight: 600;
    font-family: inherit;
    color: var(--text);
    background: var(--weiss);
    border: 2px solid var(--rand);
    border-radius: 8px;
    cursor: pointer;
  }
  .ordner:hover {
    border-color: var(--akzent);
  }

  .interne-form {
    display: flex;
    gap: 10px;
    flex-wrap: wrap;
    align-items: center;
    background: var(--weiss);
    border-radius: 12px;
    box-shadow: 0 1px 3px var(--schatten-sm);
    padding: 14px 18px;
    margin-bottom: 20px;
  }
  .interne-form input {
    padding: 9px 12px;
    font-size: 0.92rem;
    font-family: inherit;
    border: 2px solid var(--rand);
    border-radius: 8px;
    background: var(--flaeche);
  }
  .interne-form input[type="text"] {
    flex: 1;
    min-width: 180px;
  }
  .interne-form input:focus {
    outline: none;
    border-color: var(--akzent);
    background: var(--weiss);
  }
  .interne-form .primaer {
    padding: 9px 18px;
    font-size: 0.92rem;
    font-weight: 600;
    font-family: inherit;
    color: var(--weiss);
    background: var(--akzent);
    border: none;
    border-radius: 8px;
    cursor: pointer;
  }
  .interne-form .primaer:disabled {
    background: var(--grau-3);
    cursor: default;
  }
  .interne-form .leise {
    background: none;
    border: none;
    color: var(--text-muted);
    font-size: 0.9rem;
    font-family: inherit;
    cursor: pointer;
  }

  .intern-tag {
    font-size: 0.72rem;
    font-weight: 700;
    letter-spacing: 0.03em;
    padding: 3px 9px;
    border-radius: 99px;
    background: var(--lila-bg4);
    color: var(--lila-d2);
  }
  .zeile.intern .entfernen {
    background: none;
    border: none;
    color: var(--text-leise);
    font-size: 0.95rem;
    cursor: pointer;
    padding: 4px 8px;
    border-radius: 6px;
    flex-shrink: 0;
    align-self: center;
  }
  .zeile.intern .entfernen:hover {
    background: var(--gefahr-bg);
    color: var(--gefahr-text);
  }

  /* Team-Termine (aus dem Board anderer Geräte – nur Ansicht) */
  .team-tag {
    font-size: 0.72rem;
    font-weight: 700;
    letter-spacing: 0.03em;
    padding: 3px 9px;
    border-radius: 99px;
    background: var(--akzent-bg5);
    color: var(--link-d3);
  }
  .zeile.team {
    cursor: default;
  }
  .zeile.team:hover {
    box-shadow: 0 1px 3px var(--schatten-sm);
  }

  .liste {
    display: flex;
    flex-direction: column;
    gap: 12px;
  }
  .zeile {
    display: flex;
    align-items: center;
    gap: 16px;
    background: var(--weiss);
    border-radius: 12px;
    box-shadow: 0 1px 3px var(--schatten-sm);
    padding: 14px 18px;
    cursor: pointer;
    transition: box-shadow 0.15s;
  }
  .zeile:hover {
    box-shadow: 0 4px 14px var(--schatten-md);
  }
  .zeile:focus-visible {
    outline: 2px solid var(--akzent);
    outline-offset: 2px;
  }
  .zeile.verg {
    opacity: 0.7;
  }

  /* Datumsblock links */
  .datum {
    flex-shrink: 0;
    width: 60px;
    height: 60px;
    border-radius: 10px;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    line-height: 1.1;
  }
  .datum .tag {
    font-size: 1.4rem;
    font-weight: 700;
  }
  .datum .monat {
    font-size: 0.72rem;
    font-weight: 600;
    text-transform: uppercase;
  }
  .datum .jahr {
    font-size: 0.66rem;
    opacity: 0.8;
  }
  .datum .infinity {
    font-size: 1.6rem;
    font-weight: 700;
  }
  .datum.farbe-rot { background: var(--gefahr-bg); color: var(--gefahr-text); }
  .datum.farbe-gelb { background: var(--warnung-bg); color: var(--warnung-text); }
  .datum.farbe-gruen { background: var(--erfolg-bg); color: var(--erfolg-text); }
  .datum.farbe-grau { background: var(--flaeche-2b); color: var(--text-muted); }

  .haupt {
    flex: 1;
    min-width: 0;
  }
  .kopf {
    display: flex;
    align-items: center;
    gap: 10px;
    flex-wrap: wrap;
  }
  .kopf h3 {
    margin: 0;
    font-size: 1.02rem;
    font-weight: 600;
  }
  .meta {
    margin: 4px 0 0;
    color: var(--text-muted);
    font-size: 0.85rem;
  }

  /* Unterfristen (eigene Fristen aus der Detailansicht) */
  .unterfristen {
    list-style: none;
    margin: 8px 0 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .unterfristen li {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 0.8rem;
  }
  .uf-pin {
    color: var(--text-leise);
  }
  .uf-datum {
    font-variant-numeric: tabular-nums;
    color: var(--text-2);
    font-weight: 600;
    white-space: nowrap;
  }
  .uf-titel {
    flex: 1;
    min-width: 0;
    color: var(--text);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .uf-cd {
    white-space: nowrap;
    padding: 1px 9px;
    border-radius: 99px;
    font-weight: 700;
    font-size: 0.72rem;
  }
  .unterfristen li.farbe-rot .uf-cd { background: var(--gefahr-bg); color: var(--gefahr-text); }
  .unterfristen li.farbe-gelb .uf-cd { background: var(--warnung-bg); color: var(--warnung-text); }
  .unterfristen li.farbe-gruen .uf-cd { background: var(--erfolg-bg); color: var(--erfolg-text); }
  .unterfristen li.farbe-grau .uf-cd { background: var(--flaeche-2b); color: var(--text-muted); }

  .countdown {
    flex-shrink: 0;
    font-size: 0.88rem;
    font-weight: 700;
    padding: 6px 12px;
    border-radius: 99px;
    white-space: nowrap;
  }
  .countdown.farbe-rot { background: var(--gefahr-bg); color: var(--gefahr-text); }
  .countdown.farbe-gelb { background: var(--warnung-bg); color: var(--warnung-text); }
  .countdown.farbe-gruen { background: var(--erfolg-bg); color: var(--erfolg-text); }

  .land {
    font-size: 0.72rem;
    font-weight: 700;
    letter-spacing: 0.03em;
    padding: 3px 9px;
    border-radius: 99px;
    text-transform: uppercase;
  }
  .land-DE { background: var(--akzent-bg3); color: var(--link); }
  .land-AT { background: var(--gefahr-bg); color: var(--gefahr-text); }
  .land-CH { background: var(--warnung-bg); color: var(--warnung-text); }
  .land-INT { background: var(--lila-bg); color: var(--lila-d); }
  .land-ANDERES { background: var(--flaeche-2b); color: var(--text-2); }

  .status-badge {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 3px 10px 3px 8px;
    border-radius: 99px;
    font-size: 0.78rem;
    font-weight: 600;
  }
  .status-badge .punkt {
    width: 8px;
    height: 8px;
    border-radius: 50%;
  }
  .status-badge.farbe-blau { background: var(--akzent-bg2); color: var(--akzent-d3); }
  .status-badge.farbe-blau .punkt { background: var(--akzent); }
  .status-badge.farbe-lila { background: var(--lila-bg2); color: var(--lila-d2); }
  .status-badge.farbe-lila .punkt { background: var(--lila); }
  .status-badge.farbe-gruen { background: var(--erfolg-bg); color: var(--erfolg-text); }
  .status-badge.farbe-gruen .punkt { background: var(--erfolg); }
  .status-badge.farbe-rot { background: var(--gefahr-bg); color: var(--gefahr-text); }
  .status-badge.farbe-rot .punkt { background: var(--gefahr); }
  .status-badge.farbe-gelb { background: var(--warnung-bg); color: var(--warnung-text); }
  .status-badge.farbe-gelb .punkt { background: var(--warnung); }
  .status-badge.farbe-grau { background: var(--flaeche-2b); color: var(--text-2); }
  .status-badge.farbe-grau .punkt { background: var(--grau-4); }
</style>
