<script>
  // Detailansicht einer Förderung als Überlagerung.
  import { openUrl } from "@tauri-apps/plugin-opener";
  import { LAENDER, SPARTEN, PROJEKTARTEN, TRAEGERSCHAFT, fristText } from "$lib/begriffe";
  import { regionName } from "$lib/daten/orte.js";
  import AntragBlock from "./AntragBlock.svelte";

  let {
    foerderung: f,
    alle,
    hinweis,
    schliessen,
    gemerkt = null,
    umschalten = null,
    ordnerOeffnen = null,
    hochladen = null,
    pdfVorschau = null,
    pdfSpeichern = null,
    antrag = null,
    antragAendern = null,
    stand = null,
    geaenderteFelder = [],
  } = $props();

  function nameVon(id) {
    const x = alle.find((e) => e.id === id);
    return x ? `${x.name} (${x.foerdergeber})` : id;
  }

  // Wurde eines der angegebenen Felder durch ein Update geändert?
  function neu(...keys) {
    return keys.some((k) => geaenderteFelder.includes(k));
  }
</script>

<div class="schleier" onclick={schliessen} role="presentation">
  <article class="detail" onclick={(e) => e.stopPropagation()}>
    <header>
      <div>
        <span class="land land-{f.land}">{LAENDER[f.land] ?? f.land}</span>
        {#if neu("land")}<span class="neu-feld">NEU</span>{/if}
        <h3>{f.name}{#if neu("name")}<span class="neu-feld">NEU</span>{/if}</h3>
        {#if f.nichtMehrImKatalog}
          <span class="herkunft weg">⚠ nicht mehr im Katalog</span>
        {:else if f.eigen}
          <span class="herkunft selbst">✎ selbst eingetragen</span>
        {:else if f.geteilt}
          <span class="herkunft geteilt">👥 vom Team geteilt</span>
        {/if}
        <p class="geber">{f.foerdergeber}{#if neu("foerdergeber")}<span class="neu-feld">NEU</span>{/if}</p>
        {#if stand}<p class="stand">zuletzt aktualisiert: {stand}</p>{/if}
      </div>
      <button class="schliessen" onclick={schliessen} aria-label="Schließen">✕</button>
    </header>

    <p>{f.beschreibung}{#if neu("beschreibung")}<span class="neu-feld">NEU</span>{/if}</p>

    <dl>
      <dt>Förderhöhe</dt>
      <dd>{f.foerderhoehe_text}{#if neu("foerderhoehe_text")}<span class="neu-feld">NEU</span>{/if}</dd>

      <dt>Einreichung</dt>
      <dd>{fristText(f)}{#if neu("fristen", "weiche_kriterien.zeitpunkt")}<span class="neu-feld">NEU</span>{/if}</dd>

      <dt>Wohnsitz-Anforderung</dt>
      <dd>
        {f.harte_kriterien.wohnsitz.length
          ? f.harte_kriterien.wohnsitz.map((l) => LAENDER[l] ?? l).join(", ")
          : "keine Anforderung"}{#if f.harte_kriterien.wohnsitz_regionen?.length}
          · {f.harte_kriterien.wohnsitz_regionen.map((c) => regionName(f.harte_kriterien.wohnsitz[0] ?? f.land, c)).join(", ")}{/if}{#if f.harte_kriterien.wohnsitz_staedte?.length}
          · {f.harte_kriterien.wohnsitz_staedte.join(", ")}{/if}{#if neu("harte_kriterien.wohnsitz")}<span class="neu-feld">NEU</span>{/if}
      </dd>

      <dt>Durchführungsort</dt>
      <dd>
        {f.harte_kriterien.durchfuehrungsort.length
          ? f.harte_kriterien.durchfuehrungsort.map((l) => LAENDER[l] ?? l).join(", ")
          : "frei"}{#if f.harte_kriterien.durchfuehrungsort_regionen?.length}
          · {f.harte_kriterien.durchfuehrungsort_regionen.map((c) => regionName(f.harte_kriterien.durchfuehrungsort[0] ?? f.land, c)).join(", ")}{/if}{#if f.harte_kriterien.durchfuehrungsort_staedte?.length}
          · {f.harte_kriterien.durchfuehrungsort_staedte.join(", ")}{/if}{#if neu("harte_kriterien.durchfuehrungsort")}<span class="neu-feld">NEU</span>{/if}
      </dd>

      <dt>Wer kann beantragen?</dt>
      <dd>
        {f.harte_kriterien.traegerschaft.map((t) => TRAEGERSCHAFT[t] ?? t).join(", ")}
        · studentisch: {f.harte_kriterien.studentisch_erlaubt ? "ja" : "nein"}{#if neu("harte_kriterien.traegerschaft", "harte_kriterien.studentisch_erlaubt")}<span class="neu-feld">NEU</span>{/if}
      </dd>

      <dt>Sparten</dt>
      <dd>{f.weiche_kriterien.sparten.map((s) => SPARTEN[s] ?? s).join(", ") || "spartenoffen"}{#if neu("weiche_kriterien.sparten")}<span class="neu-feld">NEU</span>{/if}</dd>

      <dt>Projektarten</dt>
      <dd>{f.weiche_kriterien.projektarten.map((p) => PROJEKTARTEN[p] ?? p).join(", ")}{#if neu("weiche_kriterien.projektarten")}<span class="neu-feld">NEU</span>{/if}</dd>

      <dt>Budget-Rahmen</dt>
      <dd>
        {f.weiche_kriterien.budget_min ?? "—"} – {f.weiche_kriterien.budget_max ?? "—"}
        {f.weiche_kriterien.waehrung ?? ""}{#if neu("weiche_kriterien.budget_min", "weiche_kriterien.budget_max", "weiche_kriterien.waehrung")}<span class="neu-feld">NEU</span>{/if}
      </dd>

      {#if f.unvertraeglich_mit.length}
        <dt class="warn">Unverträglich mit</dt>
        <dd class="warn">{f.unvertraeglich_mit.map(nameVon).join("; ")}{#if neu("unvertraeglich_mit")}<span class="neu-feld">NEU</span>{/if}</dd>
      {/if}

      {#if !antrag}
        <dt>Übliche Unterlagen {#if neu("checkliste_vorschlag")}<span class="neu-feld">NEU</span>{/if}</dt>
        <dd>
          <ul>
            {#each f.checkliste_vorschlag as punkt}
              <li>{punkt}</li>
            {/each}
          </ul>
        </dd>
      {/if}
    </dl>

    {#if antrag}
      <AntragBlock {antrag} aendern={antragAendern} {hochladen} {pdfVorschau} {pdfSpeichern} />
    {/if}

    <p class="datenstand">{hinweis}</p>

    {#if ordnerOeffnen}
      <button class="zweit" onclick={ordnerOeffnen}>
        📁 Ordner zu dieser Förderung öffnen
      </button>
    {/if}
    {#if umschalten}
      <button class="zweit" onclick={() => umschalten(f.id)}>
        {gemerkt ? "★ Von der Merkliste entfernen" : "☆ Auf die Merkliste setzen"}
      </button>
    {/if}
    <button class="primaer" onclick={() => openUrl(f.webseite)}>
      Webseite des Fördergebers öffnen
    </button>
    {#if neu("webseite")}<span class="neu-feld">Webseite NEU</span>{/if}
  </article>
</div>

<style>
  .schleier {
    position: fixed;
    inset: 0;
    background: rgba(9, 30, 66, 0.45);
    display: grid;
    place-items: center;
    padding: 24px;
    z-index: 10;
  }
  .detail {
    background: #fff;
    border-radius: 12px;
    padding: 32px;
    max-width: 560px;
    width: 100%;
    max-height: 85vh;
    overflow-y: auto;
    box-shadow: 0 12px 40px rgba(9, 30, 66, 0.3);
  }
  .detail header {
    display: flex;
    justify-content: space-between;
    align-items: flex-start;
    gap: 12px;
    margin-bottom: 12px;
  }
  h3 {
    margin: 8px 0 2px;
    font-size: 1.25rem;
  }
  .detail p {
    line-height: 1.55;
    font-size: 0.95rem;
  }
  .geber {
    margin: 0;
    color: #5e6c84;
    font-size: 0.85rem;
  }
  .herkunft {
    display: inline-block;
    margin: 4px 0;
    font-size: 0.74rem;
    font-weight: 600;
    padding: 2px 9px;
    border-radius: 99px;
  }
  .herkunft.selbst { background: #eef1ff; color: #3b4fb0; }
  .herkunft.geteilt { background: #e6f4ff; color: #0c5a8f; }
  .herkunft.weg { background: #ffeceb; color: #ae2e24; }
  .neu-feld {
    display: inline-block;
    margin-left: 8px;
    font-size: 0.64rem;
    font-weight: 700;
    letter-spacing: 0.04em;
    vertical-align: middle;
    padding: 1px 7px;
    border-radius: 99px;
    background: #e9f0ff;
    color: #2b46c4;
    border: 1px solid #b9c7f7;
  }
  .stand {
    margin: 4px 0 0;
    font-size: 0.78rem;
    color: #a9b0bd;
  }

  .land {
    display: inline-block;
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

  .schliessen {
    background: none;
    border: none;
    font-size: 1.1rem;
    color: #5e6c84;
    cursor: pointer;
    padding: 6px 10px;
    border-radius: 8px;
  }
  .schliessen:hover {
    background: #f1f2f4;
    color: #172b4d;
  }

  dl {
    display: grid;
    grid-template-columns: 160px 1fr;
    gap: 10px 16px;
    margin: 20px 0;
    font-size: 0.9rem;
  }
  dt {
    color: #5e6c84;
    font-weight: 600;
  }
  dd {
    margin: 0;
  }
  dd ul {
    margin: 0;
    padding-left: 18px;
  }
  .warn {
    color: #ae2e24;
  }

  .datenstand {
    color: #8590a2;
    font-size: 0.8rem;
  }

  .zweit {
    width: 100%;
    margin-bottom: 10px;
    padding: 11px;
    font-size: 0.95rem;
    font-weight: 600;
    color: #172b4d;
    background: #fff;
    border: 2px solid #dfe1e6;
    border-radius: 8px;
    cursor: pointer;
  }
  .zweit:hover {
    border-color: #e2a400;
    background: #fffaf0;
  }

  .primaer {
    width: 100%;
    padding: 11px;
    font-size: 0.95rem;
    font-weight: 600;
    color: #fff;
    background: #4f6df5;
    border: none;
    border-radius: 8px;
    cursor: pointer;
  }
  .primaer:hover {
    background: #3d5bf0;
  }
</style>
