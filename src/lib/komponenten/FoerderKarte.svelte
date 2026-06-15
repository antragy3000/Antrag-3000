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
    neu = false,
  } = $props();
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
  <h3>{f.name}</h3>
  <div class="marker">
    {#if neu}
      <span class="herkunft neu">NEU</span>
    {/if}
    {#if f.nichtMehrImKatalog}
      <span class="herkunft weg">⚠ nicht mehr im Katalog</span>
    {:else if f.eigen}
      <span class="herkunft selbst">✎ selbst eingetragen</span>
    {/if}
  </div>
  <p class="geber">{f.foerdergeber}</p>
  <p class="hoehe">{f.foerderhoehe_text}</p>
  <div class="chips">
    {#each f.weiche_kriterien.sparten.slice(0, 3) as sp}
      <span class="chip">{SPARTEN[sp] ?? sp}</span>
    {/each}
    {#if f.weiche_kriterien.sparten.length > 3}
      <span class="chip">+{f.weiche_kriterien.sparten.length - 3}</span>
    {/if}
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
  <p class="frist">{fristText(f)}</p>
  {#if stand}<p class="stand">aktualisiert: {stand}</p>{/if}
</div>

<style>
  .foerderkarte {
    text-align: left;
    background: #fff;
    border-radius: 12px;
    padding: 20px;
    box-shadow: 0 1px 3px rgba(9, 30, 66, 0.12);
    cursor: pointer;
    transition: box-shadow 0.15s, transform 0.15s;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .foerderkarte:hover {
    box-shadow: 0 4px 14px rgba(9, 30, 66, 0.18);
    transform: translateY(-1px);
  }
  .foerderkarte:focus-visible {
    outline: 2px solid #4f6df5;
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
    color: #216e4e;
    background: #dcfff1;
    padding: 3px 9px;
    border-radius: 99px;
  }

  .stern {
    background: none;
    border: none;
    font-size: 1.15rem;
    line-height: 1;
    color: #b3bac5;
    cursor: pointer;
    padding: 2px 4px;
    border-radius: 6px;
  }
  .stern:hover {
    color: #e2a400;
  }
  .stern.aktiv {
    color: #e2a400;
  }

  .marker {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }
  .herkunft {
    align-self: flex-start;
    font-size: 0.72rem;
    font-weight: 600;
    padding: 2px 8px;
    border-radius: 99px;
  }
  .herkunft.selbst { background: #eef1ff; color: #3b4fb0; }
  .herkunft.weg { background: #ffeceb; color: #ae2e24; }
  .herkunft.neu {
    font-weight: 700;
    letter-spacing: 0.04em;
    background: #e9f0ff;
    color: #2b46c4;
    border: 1px solid #b9c7f7;
  }
  .geber {
    margin: 0;
    color: #5e6c84;
    font-size: 0.85rem;
  }
  .hoehe {
    margin: 4px 0 0;
    font-size: 0.9rem;
    font-weight: 600;
    color: #216e4e;
  }
  .frist {
    margin: 6px 0 0;
    font-size: 0.8rem;
    color: #8590a2;
  }
  .stand {
    margin: 2px 0 0;
    font-size: 0.72rem;
    color: #a9b0bd;
    text-align: right;
  }
  .treffer {
    margin: 4px 0 0;
    font-size: 0.8rem;
    color: #216e4e;
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
  .status-badge.farbe-blau { background: #e9f0ff; color: #2b46c4; }
  .status-badge.farbe-blau .punkt { background: #4f6df5; }
  .status-badge.farbe-lila { background: #f1edff; color: #5e44b0; }
  .status-badge.farbe-lila .punkt { background: #8270db; }
  .status-badge.farbe-gruen { background: #dcfff1; color: #216e4e; }
  .status-badge.farbe-gruen .punkt { background: #22a06b; }
  .status-badge.farbe-rot { background: #ffeceb; color: #ae2e24; }
  .status-badge.farbe-rot .punkt { background: #ca3521; }
  .status-badge.farbe-gelb { background: #fff7d6; color: #7f5f01; }
  .status-badge.farbe-gelb .punkt { background: #e2a400; }
  .status-badge.farbe-grau { background: #f1f2f4; color: #44546f; }
  .status-badge.farbe-grau .punkt { background: #b3bac5; }
  .gruende {
    margin: 4px 0 0;
    font-size: 0.8rem;
    color: #ae2e24;
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
  .land-DE { background: #e9f2ff; color: #0055cc; }
  .land-AT { background: #ffeceb; color: #ae2e24; }
  .land-CH { background: #fff7d6; color: #7f5f01; }
  .land-INT { background: #f3f0ff; color: #5e4db2; }

  .chips {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    margin-top: 4px;
  }
  .chip {
    font-size: 0.75rem;
    background: #f1f2f4;
    color: #44546f;
    padding: 3px 9px;
    border-radius: 99px;
  }
</style>
