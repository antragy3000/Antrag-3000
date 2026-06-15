<script>
  // Merkliste des aktiven Projekts als Listenansicht. Jede Zeile zeigt
  // die Förderung samt Antrag-Status UND der benötigten Dokumente mit
  // ihrem Status. Klick auf eine Zeile öffnet die Detailansicht mit dem
  // Status-/Checklisten-Block. Warnung bei unverträglichen Förderungen.
  import { openUrl } from "@tauri-apps/plugin-opener";
  import FoerderDetail from "./FoerderDetail.svelte";
  import EigeneFoerderung from "./EigeneFoerderung.svelte";
  import { LAENDER, SPARTEN, fristText } from "$lib/begriffe";
  import {
    ANTRAG_STATUS,
    ANTRAG_STANDARD,
    CHECK_STATUS,
    CHECK_STANDARD,
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
    eigeneAnlegen = null,
    oeffneKatalog = null,
    aktualisierteIds = [],
    hinweisVerwerfen = null,
    standFuer = null,
  } = $props();

  let ausgewaehlt = $state(null);
  let aktuellerAntrag = $state(null);
  let eigeneOffen = $state(false);

  let gemerkte = $derived(
    merkliste
      .map((id) => foerderungen.find((f) => f.id === id))
      .filter(Boolean)
  );

  // Hinweis-Zähler: gemerkte Förderungen, die nicht mehr im Katalog sind
  // bzw. deren Angaben ein Update geändert hat (nur solche, die noch
  // gemerkt sind).
  let entferntAnzahl = $derived(gemerkte.filter((f) => f.nichtMehrImKatalog).length);
  let aktualisiertAnzahl = $derived(
    (aktualisierteIds ?? []).filter((id) => merkliste.includes(id)).length
  );

  // Schließen sich zwei Förderungen gegenseitig aus?
  function unvertraeglich(a, b) {
    return (
      (a.unvertraeglich_mit ?? []).includes(b.id) ||
      (b.unvertraeglich_mit ?? []).includes(a.id)
    );
  }

  // Alle Paare gemerkter Förderungen, die sich gegenseitig ausschließen.
  let konflikte = $derived.by(() => {
    const paare = [];
    for (let i = 0; i < gemerkte.length; i++) {
      for (let j = i + 1; j < gemerkte.length; j++) {
        if (unvertraeglich(gemerkte[i], gemerkte[j])) {
          paare.push([gemerkte[i], gemerkte[j]]);
        }
      }
    }
    return paare;
  });

  // Namen der mit f unverträglichen, ebenfalls gemerkten Förderungen.
  function konfliktNamen(f) {
    return gemerkte.filter((g) => g.id !== f.id && unvertraeglich(f, g)).map((g) => g.name);
  }

  // Antrag-Status-Etikett einer Förderung.
  function badgeFuer(id) {
    const a = antraege[id];
    const key = a?.status ?? ANTRAG_STANDARD;
    return {
      label: statusLabel(ANTRAG_STATUS, key, a?.statusFrei),
      farbe: statusFarbe(ANTRAG_STATUS, key),
    };
  }

  // Checkliste zum Anzeigen: bearbeiteter Stand oder Vorschlag der Förderung.
  function checklisteFuer(f) {
    const a = antraege[f.id];
    if (a?.checkliste?.length) return a.checkliste;
    return (f.checkliste_vorschlag ?? []).map((t) => ({
      text: t,
      status: CHECK_STANDARD,
      statusFrei: "",
    }));
  }

  function oeffnen(f) {
    aktuellerAntrag = antragHolen ? antragHolen(f) : null;
    ausgewaehlt = f;
  }

  // Kontaktperson / E-Mail einer Förderung (aus dem Antrag-Eintrag).
  function kontaktName(id) {
    return (antraege[id]?.kontakt?.ansprechpartner ?? "").trim();
  }
  function kontaktEmail(id) {
    return (antraege[id]?.kontakt?.email ?? "").trim();
  }
  function mailSchreiben(id) {
    const adr = kontaktEmail(id);
    if (adr) openUrl("mailto:" + adr);
  }

  // Fristen zur Anzeige in der Zeile: offizielle (editierbare Übernahme
  // aus der Datenbank) plus benannte eigene Fristen. Jede einzeln, mit
  // Resttagen für die Rotfärbung.
  const HEUTE = new Date();
  HEUTE.setHours(0, 0, 0, 0);
  function tageBis(d) {
    return Math.round((new Date(d) - HEUTE) / 86400000);
  }
  function fmtDatum(d) {
    return new Date(d).toLocaleDateString("de-DE");
  }
  function fristenFuerListe(f) {
    const a = antraege[f.id];
    const offiziell = (a?.offizielleFristen ?? f.fristen ?? []).filter(Boolean);
    const eigene = a?.eigeneFristen ?? [];
    const eintraege = [];
    for (const d of offiziell) eintraege.push({ label: "Frist", datum: d, tage: tageBis(d) });
    for (const e of eigene) {
      const datum = typeof e === "string" ? e : e.datum;
      const titel = typeof e === "string" ? "" : e.titel;
      eintraege.push({ label: titel || "Eigene Frist", datum, tage: tageBis(datum) });
    }
    return eintraege;
  }
</script>

<div class="bereich">
  <div class="kopfzeile">
    <h2>Merkliste <span class="anzahl">{gemerkte.length}</span></h2>
    <div class="kopf-knoepfe">
      {#if oeffneKatalog}
        <button class="ordner" onclick={oeffneKatalog}>
          🗂 Förder-Datenbank
        </button>
      {/if}
      {#if eigeneAnlegen}
        <button class="ordner" onclick={() => (eigeneOffen = true)}>
          + Eigene Förderung
        </button>
      {/if}
      {#if ordnerOeffnen}
        <button class="ordner" onclick={() => ordnerOeffnen(null)}>
          📁 Projektordner öffnen
        </button>
      {/if}
    </div>
  </div>

  {#if entferntAnzahl > 0 || aktualisiertAnzahl > 0}
    <div class="katalog-hinweis">
      <div class="kh-text">
        {#if entferntAnzahl > 0}
          <div class="kh-zeile weg">
            <strong>⚠ {entferntAnzahl}</strong>
            {entferntAnzahl === 1 ? "gemerkte Förderung ist" : "gemerkte Förderungen sind"}
            <strong>nicht mehr im Katalog</strong> (unten rot markiert).
          </div>
        {/if}
        {#if aktualisiertAnzahl > 0}
          <div class="kh-zeile akt">
            <strong>ℹ {aktualisiertAnzahl}</strong>
            {aktualisiertAnzahl === 1 ? "gemerkte Förderung wurde" : "gemerkte Förderungen wurden"}
            durch ein Update <strong>aktualisiert</strong> – prüfe die Angaben.
          </div>
        {/if}
      </div>
      {#if aktualisiertAnzahl > 0 && hinweisVerwerfen}
        <button class="kh-ok" onclick={hinweisVerwerfen}>OK, verstanden</button>
      {/if}
    </div>
  {/if}

  {#each konflikte as [a, b]}
    <div class="konflikt">
      <strong>⚠ Unverträglich:</strong>
      „{a.name}" ({a.foerdergeber}) und „{b.name}" ({b.foerdergeber})
      schließen sich gegenseitig aus. Für die Antragstellung musst du
      dich für eine der beiden entscheiden.
    </div>
  {/each}

  {#if gemerkte.length === 0}
    <p class="leer">
      Noch nichts gemerkt. Markiere Förderungen mit dem ☆-Stern –
      in der Übersicht oder im Matching-Ergebnis – oder lege oben eine
      <strong>eigene Förderung</strong> an.
    </p>
  {:else}
    <div class="liste">
      {#each gemerkte as f (f.id)}
        {@const badge = badgeFuer(f.id)}
        <div
          class="zeile"
          class:weg={f.nichtMehrImKatalog}
          class:akt={aktualisierteIds.includes(f.id)}
          role="button"
          tabindex="0"
          onclick={() => oeffnen(f)}
          onkeydown={(e) => {
            if (e.key === "Enter" || e.key === " ") {
              e.preventDefault();
              oeffnen(f);
            }
          }}
        >
          <div class="aktionen">
            <button
              class="stern"
              title="Von der Merkliste entfernen"
              aria-label="Von der Merkliste entfernen"
              onclick={(e) => {
                e.stopPropagation();
                umschalten(f.id);
              }}
            >
              ★
            </button>
            <button
              class="mail"
              class:aus={!kontaktEmail(f.id)}
              title={kontaktEmail(f.id)
                ? "E-Mail an " + (kontaktName(f.id) || "Kontaktperson") + " schreiben"
                : "noch keine Emailadresse eingetragen"}
              aria-label="E-Mail an Kontaktperson"
              onclick={(e) => {
                e.stopPropagation();
                mailSchreiben(f.id);
              }}
            >
              ✉
            </button>
          </div>

          <div class="haupt">
            <div class="kopf">
              <span class="land land-{f.land}">{LAENDER[f.land] ?? f.land}</span>
              <h3>{f.name}</h3>
              <span class="status-badge farbe-{badge.farbe}">
                <span class="punkt"></span>{badge.label}
              </span>
              {#if f.nichtMehrImKatalog}
                <span class="herkunft weg">⚠ nicht mehr im Katalog</span>
              {:else if f.eigen}
                <span class="herkunft selbst">✎ selbst eingetragen</span>
              {/if}
              {#if aktualisierteIds.includes(f.id)}
                <span class="herkunft akt">NEU</span>
              {/if}
            </div>

            <p class="meta">
              <span class="meta-links">{f.foerdergeber} · <span class="hoehe">{f.foerderhoehe_text}</span></span>
              {#if standFuer && standFuer(f.id)}
                <span class="stand">aktualisiert {standFuer(f.id)}</span>
              {/if}
            </p>

            <div class="tag-zeile">
              <div class="chips">
                {#each f.weiche_kriterien.sparten as sp}
                  <span class="chip">{SPARTEN[sp] ?? sp}</span>
                {/each}
              </div>
              <div class="fristen-rechts">
                {#each fristenFuerListe(f) as fr (fr.label + fr.datum)}
                  <span class="frist" class:dringend={fr.tage <= 14}>
                    {fr.label}: {fmtDatum(fr.datum)}
                  </span>
                {:else}
                  <span class="frist">{fristText(f)}</span>
                {/each}
              </div>
            </div>

            <p class="kontaktperson" class:leer-kontakt={!kontaktName(f.id)}>
              👤 {kontaktName(f.id) || "keine Kontaktperson"}
            </p>

            {#if konfliktNamen(f).length}
              <p class="konflikt-zeile">
                ⚠ Unverträglich mit: {konfliktNamen(f).join(", ")}
              </p>
            {/if}

            <div class="dokumente">
              <span class="dok-titel">Benötigte Dokumente:</span>
              <ul>
                {#each checklisteFuer(f) as p (p.text)}
                  <li>
                    <span class="punkt farbe-{statusFarbe(CHECK_STATUS, p.status)}"></span>
                    <span class="dok-text" class:fertig={p.status === "abgeschlossen"}>
                      {p.text}
                    </span>
                    <span class="dok-status">{statusLabel(CHECK_STATUS, p.status, p.statusFrei)}</span>
                  </li>
                {:else}
                  <li class="dok-leer">noch keine Dokumente</li>
                {/each}
              </ul>
            </div>
          </div>
        </div>
      {/each}
    </div>
  {/if}
</div>

{#if eigeneOffen}
  <EigeneFoerderung
    anlegen={eigeneAnlegen}
    schliessen={() => (eigeneOffen = false)}
  />
{/if}

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
    antrag={aktuellerAntrag}
    antragAendern={antragSpeichern}
    stand={standFuer ? standFuer(ausgewaehlt.id) : null}
    neu={aktualisierteIds.includes(ausgewaehlt.id)}
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

  .kopfzeile {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 16px;
    flex-wrap: wrap;
    margin-bottom: 20px;
  }
  h2 {
    margin: 0;
    font-size: 1.35rem;
    font-weight: 600;
  }
  .anzahl {
    color: #5e6c84;
    font-size: 0.9rem;
    font-weight: 400;
    margin-left: 8px;
  }
  .kopf-knoepfe {
    display: flex;
    gap: 10px;
    flex-wrap: wrap;
  }
  .ordner {
    padding: 9px 16px;
    font-size: 0.9rem;
    font-weight: 600;
    font-family: inherit;
    color: #172b4d;
    background: #fff;
    border: 2px solid #dfe1e6;
    border-radius: 8px;
    cursor: pointer;
  }
  .ordner:hover {
    border-color: #4f6df5;
  }

  .konflikt {
    background: #ffeceb;
    border-left: 4px solid #ca3521;
    border-radius: 8px;
    padding: 14px 16px;
    margin-bottom: 16px;
    font-size: 0.92rem;
    line-height: 1.5;
    color: #5d1f1a;
  }

  .leer {
    color: #5e6c84;
  }

  /* Listenansicht */
  .liste {
    display: flex;
    flex-direction: column;
    gap: 12px;
  }
  .zeile {
    display: flex;
    gap: 14px;
    background: #fff;
    border-radius: 12px;
    box-shadow: 0 1px 3px rgba(9, 30, 66, 0.12);
    padding: 16px 20px;
    cursor: pointer;
    transition: box-shadow 0.15s;
  }
  .zeile:hover {
    box-shadow: 0 4px 14px rgba(9, 30, 66, 0.18);
  }
  .zeile:focus-visible {
    outline: 2px solid #4f6df5;
    outline-offset: 2px;
  }
  .zeile.weg {
    border-left: 5px solid #ca3521;
    background: #fff8f7;
  }
  .zeile.akt {
    border-left: 5px solid #4f6df5;
    background: #f7f9ff;
  }

  .katalog-hinweis {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 14px;
    flex-wrap: wrap;
    background: #fff7ef;
    border: 1px solid #ffd9b0;
    border-radius: 10px;
    padding: 12px 16px;
    margin-bottom: 16px;
  }
  .kh-text { display: flex; flex-direction: column; gap: 4px; }
  .kh-zeile { font-size: 0.92rem; line-height: 1.45; }
  .kh-zeile.weg { color: #ae2e24; }
  .kh-zeile.akt { color: #7a4a00; }
  .kh-ok {
    background: #fff;
    border: 2px solid #e7c9a3;
    border-radius: 8px;
    padding: 7px 13px;
    font-size: 0.85rem;
    font-weight: 600;
    font-family: inherit;
    color: #7a4a00;
    cursor: pointer;
    white-space: nowrap;
  }
  .kh-ok:hover { background: #fdf1e3; }

  .aktionen {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 6px;
    flex-shrink: 0;
  }
  .stern {
    background: none;
    border: none;
    font-size: 1.2rem;
    line-height: 1;
    color: #e2a400;
    cursor: pointer;
    padding: 2px 4px;
    border-radius: 6px;
    height: fit-content;
  }
  .stern:hover {
    background: #fffaf0;
  }
  .mail {
    background: none;
    border: none;
    font-size: 1.05rem;
    line-height: 1;
    color: #4f6df5;
    cursor: pointer;
    padding: 3px 4px;
    border-radius: 6px;
    height: fit-content;
  }
  .mail:hover {
    background: #eef1ff;
  }
  .mail.aus {
    color: #c1c7d0;
    cursor: not-allowed;
  }
  .mail.aus:hover {
    background: none;
  }

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
    flex: 1;
    min-width: 0;
  }

  .meta {
    margin: 6px 0 0;
    color: #5e6c84;
    font-size: 0.85rem;
    display: flex;
    justify-content: space-between;
    align-items: baseline;
    gap: 12px;
  }
  .meta-links { min-width: 0; }
  .hoehe {
    color: #216e4e;
    font-weight: 600;
  }

  .tag-zeile {
    display: flex;
    align-items: flex-start;
    gap: 12px;
    margin-top: 8px;
  }
  .chips {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    flex: 1;
    min-width: 0;
  }
  .chip {
    font-size: 0.75rem;
    background: #f1f2f4;
    color: #44546f;
    padding: 3px 9px;
    border-radius: 99px;
  }
  .fristen-rechts {
    display: flex;
    flex-direction: column;
    align-items: flex-end;
    gap: 2px;
    flex-shrink: 0;
    padding-top: 2px;
  }
  .frist {
    font-size: 1rem;
    font-weight: 600;
    color: #5e6c84;
    white-space: nowrap;
  }
  .frist.dringend {
    color: #ae2e24;
    font-weight: 700;
  }
  .kontaktperson {
    margin: 8px 0 0;
    font-size: 0.82rem;
    color: #44546f;
  }
  .kontaktperson.leer-kontakt {
    color: #b3bac5;
  }
  .konflikt-zeile {
    margin: 8px 0 0;
    font-size: 0.82rem;
    font-weight: 600;
    color: #ae2e24;
  }

  .land {
    font-size: 0.72rem;
    font-weight: 700;
    letter-spacing: 0.03em;
    padding: 3px 9px;
    border-radius: 99px;
    text-transform: uppercase;
  }
  .land-DE { background: #e9f2ff; color: #0055cc; }
  .land-AT { background: #ffeceb; color: #ae2e24; }
  .land-CH { background: #fff7d6; color: #7f5f01; }
  .land-INT { background: #f3f0ff; color: #5e4db2; }
  .land-ANDERES { background: #f1f2f4; color: #44546f; }

  /* Dokumente mit Status */
  .dokumente {
    margin-top: 12px;
    padding-top: 10px;
    border-top: 1px solid #f1f2f4;
  }
  .dok-titel {
    font-size: 0.78rem;
    font-weight: 600;
    color: #8590a2;
    text-transform: uppercase;
    letter-spacing: 0.03em;
  }
  .dokumente ul {
    list-style: none;
    margin: 8px 0 0;
    padding: 0;
    display: grid;
    gap: 5px;
  }
  .dokumente li {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 0.85rem;
  }
  .dok-text {
    color: #172b4d;
  }
  .dok-text.fertig {
    color: #5e6c84;
    text-decoration: line-through;
  }
  .dok-status {
    color: #5e6c84;
    font-size: 0.8rem;
  }
  .dok-status::before {
    content: "– ";
  }
  .dok-leer {
    color: #8590a2;
    font-size: 0.82rem;
  }

  /* farbiger Statuspunkt */
  .punkt {
    width: 9px;
    height: 9px;
    border-radius: 50%;
    flex-shrink: 0;
    display: inline-block;
  }
  .farbe-blau { background: #4f6df5; }
  .farbe-lila { background: #8270db; }
  .farbe-gruen { background: #22a06b; }
  .farbe-rot { background: #ca3521; }
  .farbe-gelb { background: #e2a400; }
  .farbe-grau { background: #b3bac5; }

  /* Antrag-Status-Etikett */
  .status-badge {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 3px 10px 3px 8px;
    border-radius: 99px;
    font-size: 0.78rem;
    font-weight: 600;
  }
  .herkunft {
    font-size: 0.72rem;
    font-weight: 600;
    padding: 2px 8px;
    border-radius: 99px;
  }
  .herkunft.selbst { background: #eef1ff; color: #3b4fb0; }
  .herkunft.akt {
    font-size: 0.76rem;
    font-weight: 700;
    padding: 3px 10px;
    background: #e9f0ff;
    color: #2b46c4;
    border: 1px solid #b9c7f7;
  }
  .herkunft.weg {
    font-size: 0.76rem;
    font-weight: 700;
    padding: 3px 10px;
    background: #ffeceb;
    color: #ae2e24;
    border: 1px solid #f4b1a8;
  }
  .stand { color: #a9b0bd; font-size: 0.82rem; white-space: nowrap; }
  .status-badge.farbe-blau { background: #e9f0ff; color: #2b46c4; }
  .status-badge.farbe-lila { background: #f1edff; color: #5e44b0; }
  .status-badge.farbe-gruen { background: #dcfff1; color: #216e4e; }
  .status-badge.farbe-rot { background: #ffeceb; color: #ae2e24; }
  .status-badge.farbe-gelb { background: #fff7d6; color: #7f5f01; }
  .status-badge.farbe-grau { background: #f1f2f4; color: #44546f; }
  .status-badge .punkt { width: 8px; height: 8px; }
  .status-badge.farbe-blau .punkt { background: #4f6df5; }
  .status-badge.farbe-lila .punkt { background: #8270db; }
  .status-badge.farbe-gruen .punkt { background: #22a06b; }
  .status-badge.farbe-rot .punkt { background: #ca3521; }
  .status-badge.farbe-gelb .punkt { background: #e2a400; }
  .status-badge.farbe-grau .punkt { background: #b3bac5; }
</style>
