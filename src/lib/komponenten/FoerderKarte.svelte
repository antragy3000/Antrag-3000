<script>
  // Eine einzelne Förderkarte. Optional mit Matching-Zusatz
  // (punkte/treffer bzw. gruende) und Merklisten-Stern.
  import { LAENDER, SPARTEN, fristText } from "$lib/begriffe";

  let {
    foerderung: f,
    auswaehlen,
    punkte = null,
    treffer = [],
    gruende = [],
    gemerkt = null,
    merken = null,
    statusBadge = null,
    stand = null,
    geaenderteFelder = [],
  } = $props();

  // Wurde eines der angegebenen Felder durch ein Update geändert?
  function neu(...keys) {
    return keys.some((k) => geaenderteFelder.includes(k));
  }
</script>

<div
  class="foerderkarte"
  class:gedimmt={gruende.length > 0}
  role="button"
  tabindex="0"
  onclick={() => auswaehlen(f)}
  onkeydown={(e) => {
    if (e.key === "Enter" || e.key === " ") {
      e.preventDefault();
      auswaehlen(f);
    }
  }}
>
  <div class="oben">
    <span class="land land-{f.land}">{LAENDER[f.land] ?? f.land}</span>
    <span class="rechts">
      {#if punkte !== null}
        <span class="punkte">Passung {punkte} %</span>
      {/if}
      {#if merken}
        <button
          class="stern"
          class:aktiv={gemerkt}
          title={gemerkt ? "Von der Merkliste entfernen" : "Auf die Merkliste"}
          aria-label={gemerkt ? "Von der Merkliste entfernen" : "Auf die Merkliste"}
          onclick={(e) => {
            e.stopPropagation();
            merken(f.id);
          }}
        >
          {gemerkt ? "★" : "☆"}
        </button>
      {/if}
    </span>
  </div>
  <h3>{f.name}{#if neu("name")}<span class="neu-feld">NEU</span>{/if}</h3>
  {#if f.nichtMehrImKatalog}
    <span class="herkunft weg">⚠ nicht mehr im Katalog</span>
  {:else if f.eigen}
    <span class="herkunft selbst">✎ selbst eingetragen</span>
  {:else if f.geteilt}
    <span class="herkunft geteilt">👥 vom Team geteilt</span>
  {:else if f.herkunft && f.herkunft.typ === "foerderer-signiert"}
    <span
      class="herkunft foerderer"
      title={f.herkunft.foerderer_name
        ? "Vom Förderer eingetragen und verifiziert: " + f.herkunft.foerderer_name
        : "Vom Förderer eingetragen und verifiziert"}
    >✓ vom Förderer</span>
  {/if}
  <p class="geber">
    {#if f.logo_thumb}
      <img class="geber-logo" src={f.logo_thumb} alt="" />
    {/if}
    {f.foerdergeber}{#if neu("foerdergeber")}<span class="neu-feld">NEU</span>{/if}
  </p>
  <p class="hoehe">
    {f.foerderhoehe_text}
    {#if neu("foerderhoehe_text")}<span class="neu-feld">NEU</span>{/if}
  </p>
  <div class="chips">
    {#each f.weiche_kriterien.sparten.slice(0, 3) as sp}
      <span class="chip">{SPARTEN[sp] ?? sp}</span>
    {/each}
    {#if f.weiche_kriterien.sparten.length > 3}
      <span class="chip">+{f.weiche_kriterien.sparten.length - 3}</span>
    {/if}
    {#if neu("weiche_kriterien.sparten", "weiche_kriterien.projektarten")}<span class="neu-feld">NEU</span>{/if}
  </div>
  {#if statusBadge}
    <p class="status-badge farbe-{statusBadge.farbe}">
      <span class="punkt"></span>{statusBadge.label}
    </p>
  {/if}
  {#if treffer.length}
    <p class="treffer">passt bei: {treffer.join(", ")}</p>
  {/if}
  {#if gruende.length}
    <p class="gruende">{gruende.join(" · ")}</p>
  {/if}
  <p class="frist">
    {fristText(f)}
    {#if neu("fristen", "weiche_kriterien.zeitpunkt")}<span class="neu-feld">NEU</span>{/if}
  </p>
  {#if stand}<p class="stand">aktualisiert: {stand}</p>{/if}
</div>

<style>
  .foerderkarte {
    text-align: left;
    background: var(--weiss);
    border-radius: 12px;
    padding: 20px;
    box-shadow: 0 1px 3px var(--schatten-sm);
    cursor: pointer;
    transition: box-shadow 0.15s, transform 0.15s;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .foerderkarte:hover {
    box-shadow: 0 4px 14px var(--schatten-md);
    transform: translateY(-1px);
  }
  .foerderkarte:focus-visible {
    outline: 2px solid var(--akzent);
    outline-offset: 2px;
  }
  .foerderkarte.gedimmt {
    opacity: 0.75;
  }
  h3 {
    margin: 0;
    font-size: 1.02rem;
    font-weight: 600;
    line-height: 1.35;
  }

  .oben {
    display: flex;
    justify-content: space-between;
    align-items: center;
  }
  .rechts {
    display: flex;
    align-items: center;
    gap: 6px;
  }
  .punkte {
    font-size: 0.78rem;
    font-weight: 700;
    color: var(--erfolg-text);
    background: var(--erfolg-bg);
    padding: 3px 9px;
    border-radius: 99px;
  }

  .stern {
    background: none;
    border: none;
    font-size: 1.15rem;
    line-height: 1;
    color: var(--grau-4);
    cursor: pointer;
    padding: 2px 4px;
    border-radius: 6px;
  }
  .stern:hover {
    color: var(--warnung-text);
  }
  .stern.aktiv {
    color: var(--warnung-text);
  }

  .herkunft {
    align-self: flex-start;
    font-size: 0.72rem;
    font-weight: 600;
    padding: 2px 8px;
    border-radius: 99px;
  }
  .herkunft.selbst { background: var(--akzent-bg); color: var(--akzent-d4); }
  .herkunft.geteilt { background: var(--akzent-bg5); color: var(--link-d3); }
  .herkunft.foerderer { background: var(--erfolg-bg); color: var(--erfolg-text); }
  .herkunft.weg { background: var(--gefahr-bg); color: var(--gefahr-text); }
  .neu-feld {
    display: inline-block;
    margin-left: 6px;
    font-size: 0.62rem;
    font-weight: 700;
    letter-spacing: 0.04em;
    vertical-align: middle;
    padding: 1px 6px;
    border-radius: 99px;
    background: var(--akzent-bg2);
    color: var(--akzent-d3);
    border: 1px solid var(--akzent-rand);
  }
  .geber {
    margin: 0;
    color: var(--text-muted);
    font-size: 0.85rem;
    display: flex;
    align-items: center;
    gap: 7px;
  }
  .geber-logo {
    height: 22px;
    width: auto;
    max-width: 52px;
    object-fit: contain;
    border-radius: 3px;
    flex: none;
  }
  .hoehe {
    margin: 4px 0 0;
    font-size: 0.9rem;
    font-weight: 600;
    color: var(--erfolg-text);
  }
  .frist {
    margin: 6px 0 0;
    font-size: 0.8rem;
    color: var(--text-leise);
  }
  .stand {
    margin: 2px 0 0;
    font-size: 0.72rem;
    color: var(--grau-5);
    text-align: right;
  }
  .treffer {
    margin: 4px 0 0;
    font-size: 0.8rem;
    color: var(--erfolg-text);
  }

  .status-badge {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    align-self: flex-start;
    margin: 6px 0 0;
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
  .gruende {
    margin: 4px 0 0;
    font-size: 0.8rem;
    color: var(--gefahr-text);
  }

  .land {
    align-self: flex-start;
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

  .chips {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    margin-top: 4px;
  }
  .chip {
    font-size: 0.75rem;
    background: var(--flaeche-2b);
    color: var(--text-2);
    padding: 3px 9px;
    border-radius: 99px;
  }
</style>
