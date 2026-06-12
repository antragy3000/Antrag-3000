<script>
  // Eine einzelne Förderkarte. Optional mit Matching-Zusatz:
  // punkte/treffer (Rangliste) oder gruende (weitere Vorschläge).
  import { LAENDER, SPARTEN, fristText } from "$lib/begriffe";

  let {
    foerderung: f,
    auswaehlen,
    punkte = null,
    treffer = [],
    gruende = [],
  } = $props();
</script>

<button class="foerderkarte" class:gedimmt={gruende.length > 0} onclick={() => auswaehlen(f)}>
  <div class="oben">
    <span class="land land-{f.land}">{LAENDER[f.land] ?? f.land}</span>
    {#if punkte !== null}
      <span class="punkte">Passung {punkte} %</span>
    {/if}
  </div>
  <h3>{f.name}</h3>
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
  {#if treffer.length}
    <p class="treffer">passt bei: {treffer.join(", ")}</p>
  {/if}
  {#if gruende.length}
    <p class="gruende">{gruende.join(" · ")}</p>
  {/if}
  <p class="frist">{fristText(f)}</p>
</button>

<style>
  .foerderkarte {
    text-align: left;
    background: #fff;
    border: none;
    border-radius: 12px;
    padding: 20px;
    box-shadow: 0 1px 3px rgba(9, 30, 66, 0.12);
    cursor: pointer;
    transition: box-shadow 0.15s, transform 0.15s;
    font: inherit;
    color: inherit;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .foerderkarte:hover {
    box-shadow: 0 4px 14px rgba(9, 30, 66, 0.18);
    transform: translateY(-1px);
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
  .punkte {
    font-size: 0.78rem;
    font-weight: 700;
    color: #216e4e;
    background: #dcfff1;
    padding: 3px 9px;
    border-radius: 99px;
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
  .treffer {
    margin: 4px 0 0;
    font-size: 0.8rem;
    color: #216e4e;
  }
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
