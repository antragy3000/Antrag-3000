<script>
  // Detailansicht einer Förderung als Überlagerung.
  import { openUrl } from "@tauri-apps/plugin-opener";
  import { LAENDER, SPARTEN, PROJEKTARTEN, TRAEGERSCHAFT, fristText, anteilText } from "$lib/begriffe";
  import { regionName } from "$lib/daten/orte.js";
  import { sichereWebUrl } from "$lib/sicherheit";
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
    logoHerunterladen = null,
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

  // Die Webseite kann aus geteilten/synchronisierten Daten stammen und
  // ist deshalb nicht vertrauenswürdig: nur eine echte http/https-Adresse
  // wird ans Betriebssystem zum Öffnen gegeben (sonst null).
  let webseiteSicher = $derived(sichereWebUrl(f.webseite));
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
        {#if f.aktiv === false}
          <span class="herkunft weg">⏸ derzeit nicht aktiv</span>
        {/if}
        <p class="geber">{f.foerdergeber}{#if neu("foerdergeber")}<span class="neu-feld">NEU</span>{/if}</p>
        {#if stand}<p class="stand">zuletzt aktualisiert: {stand}</p>{/if}
        {#if f.logo_id && logoHerunterladen}
          <div class="foerder-logo">
            {#if f.logo_thumb}<img src={f.logo_thumb} alt="Logo {f.foerdergeber}" />{/if}
            <button class="logo-dl" onclick={() => logoHerunterladen(f)}>
              Logo herunterladen
            </button>
          </div>
        {/if}
      </div>
      <button class="schliessen" onclick={schliessen} aria-label="Schließen">✕</button>
    </header>

    <p>{f.beschreibung}{#if neu("beschreibung")}<span class="neu-feld">NEU</span>{/if}</p>

    <dl>
      <dt>Förderhöhe</dt>
      <dd>{f.foerderhoehe_text}{#if neu("foerderhoehe_text")}<span class="neu-feld">NEU</span>{/if}</dd>

      {#if anteilText(f)}
        <dt>Max. Anteil</dt>
        <dd>{anteilText(f)}{#if neu("max_anteil_prozent", "anteil_ausnahme")}<span class="neu-feld">NEU</span>{/if}</dd>
      {/if}

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
      <AntragBlock {antrag} aendern={antragAendern} {hochladen} {pdfVorschau} {pdfSpeichern}
        einreichOnline={f.einreichung_online} einreichUrl={f.einreich_url} />
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
    {#if webseiteSicher}
      <button class="primaer" onclick={() => openUrl(webseiteSicher)}>
        Webseite des Fördergebers öffnen
      </button>
    {:else if (f.webseite ?? "").trim()}
      <p class="unsichere-url">
        ⚠ Die hinterlegte Webseite ist kein normaler Web-Link und wird aus
        Sicherheitsgründen nicht geöffnet: <code>{f.webseite}</code>
      </p>
    {/if}
    {#if neu("webseite")}<span class="neu-feld">Webseite NEU</span>{/if}
  </article>
</div>

<style>
  .unsichere-url {
    margin-top: 8px;
    padding: 8px 10px;
    background: var(--warnung-bg3);
    border: 1px solid var(--warnung-rand);
    border-radius: 8px;
    font-size: 0.85rem;
    color: var(--warnung-text6);
    overflow-wrap: anywhere;
  }
  .unsichere-url code {
    overflow-wrap: anywhere;
  }
  .schleier {
    position: fixed;
    inset: 0;
    background: var(--schatten-xl);
    display: grid;
    place-items: center;
    padding: 24px;
    z-index: 10;
  }
  .detail {
    background: var(--weiss);
    border-radius: 12px;
    padding: 32px;
    max-width: 560px;
    width: 100%;
    max-height: 85vh;
    overflow-y: auto;
    box-shadow: 0 12px 40px var(--schatten-lg);
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
    color: var(--text-muted);
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
  .herkunft.selbst { background: var(--akzent-bg); color: var(--akzent-d4); }
  .herkunft.geteilt { background: var(--akzent-bg5); color: var(--link-d3); }
  .herkunft.weg { background: var(--gefahr-bg); color: var(--gefahr-text); }
  .neu-feld {
    display: inline-block;
    margin-left: 8px;
    font-size: 0.64rem;
    font-weight: 700;
    letter-spacing: 0.04em;
    vertical-align: middle;
    padding: 1px 7px;
    border-radius: 99px;
    background: var(--akzent-bg2);
    color: var(--akzent-d3);
    border: 1px solid var(--akzent-rand);
  }
  .stand {
    margin: 4px 0 0;
    font-size: 0.78rem;
    color: var(--grau-5);
  }
  .foerder-logo {
    display: flex;
    align-items: center;
    gap: 10px;
    margin: 8px 0 0;
  }
  .foerder-logo img {
    height: 34px;
    width: auto;
    max-width: 90px;
    object-fit: contain;
    border-radius: 4px;
  }
  .logo-dl {
    padding: 6px 12px;
    font-size: 0.82rem;
    font-weight: 600;
    color: var(--link);
    background: var(--akzent-bg3);
    border: 1px solid var(--akzent-rand);
    border-radius: 8px;
    cursor: pointer;
  }
  .logo-dl:hover {
    background: var(--akzent-bg2);
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
  .land-DE { background: var(--akzent-bg3); color: var(--link); }
  .land-AT { background: var(--gefahr-bg); color: var(--gefahr-text); }
  .land-CH { background: var(--warnung-bg); color: var(--warnung-text); }
  .land-INT { background: var(--lila-bg); color: var(--lila-d); }
  .land-ANDERES { background: var(--flaeche-2b); color: var(--text-2); }

  .schliessen {
    background: none;
    border: none;
    font-size: 1.1rem;
    color: var(--text-muted);
    cursor: pointer;
    padding: 6px 10px;
    border-radius: 8px;
  }
  .schliessen:hover {
    background: var(--flaeche-2b);
    color: var(--text);
  }

  dl {
    display: grid;
    grid-template-columns: 160px 1fr;
    gap: 10px 16px;
    margin: 20px 0;
    font-size: 0.9rem;
  }
  dt {
    color: var(--text-muted);
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
    color: var(--gefahr-text);
  }

  .datenstand {
    color: var(--text-leise);
    font-size: 0.8rem;
  }

  .zweit {
    width: 100%;
    margin-bottom: 10px;
    padding: 11px;
    font-size: 0.95rem;
    font-weight: 600;
    color: var(--text);
    background: var(--weiss);
    border: 2px solid var(--rand);
    border-radius: 8px;
    cursor: pointer;
  }
  .zweit:hover {
    border-color: var(--warnung);
    background: var(--warnung-bg2);
  }

  .primaer {
    width: 100%;
    padding: 11px;
    font-size: 0.95rem;
    font-weight: 600;
    color: var(--weiss);
    background: var(--akzent);
    border: none;
    border-radius: 8px;
    cursor: pointer;
  }
  .primaer:hover {
    background: var(--akzent-d);
  }
</style>
