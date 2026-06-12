<script>
  // Merkliste des aktiven Projekts. Zeigt eine Warnung, wenn sich
  // zwei gemerkte Förderungen gegenseitig ausschließen
  // (Feld "unvertraeglich_mit" in der Förder-Datenbank).
  import datenbank from "$lib/daten/foerderungen.json";
  import FoerderKarte from "./FoerderKarte.svelte";
  import FoerderDetail from "./FoerderDetail.svelte";

  let { merkliste, umschalten, ordnerOeffnen = null, antragErzeugen = null } = $props();

  let ausgewaehlt = $state(null);

  let gemerkte = $derived(
    merkliste
      .map((id) => datenbank.foerderungen.find((f) => f.id === id))
      .filter(Boolean)
  );

  // Alle Paare gemerkter Förderungen, die sich gegenseitig ausschließen.
  let konflikte = $derived.by(() => {
    const paare = [];
    for (let i = 0; i < gemerkte.length; i++) {
      for (let j = i + 1; j < gemerkte.length; j++) {
        const a = gemerkte[i];
        const b = gemerkte[j];
        if (a.unvertraeglich_mit.includes(b.id) || b.unvertraeglich_mit.includes(a.id)) {
          paare.push([a, b]);
        }
      }
    }
    return paare;
  });
</script>

<div class="bereich">
  <div class="kopfzeile">
    <h2>Merkliste <span class="anzahl">{gemerkte.length}</span></h2>
    {#if ordnerOeffnen}
      <button class="ordner" onclick={() => ordnerOeffnen(null)}>
        📁 Projektordner öffnen
      </button>
    {/if}
  </div>

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
      in der Übersicht oder im Matching-Ergebnis.
    </p>
  {:else}
    <div class="raster">
      {#each gemerkte as f (f.id)}
        <FoerderKarte
          foerderung={f}
          gemerkt={true}
          merken={umschalten}
          auswaehlen={(x) => (ausgewaehlt = x)}
        />
      {/each}
    </div>
  {/if}
</div>

{#if ausgewaehlt}
  <FoerderDetail
    foerderung={ausgewaehlt}
    alle={datenbank.foerderungen}
    hinweis={datenbank.hinweis}
    gemerkt={merkliste.includes(ausgewaehlt.id)}
    umschalten={umschalten}
    ordnerOeffnen={ordnerOeffnen ? () => ordnerOeffnen(ausgewaehlt.name) : null}
    antragErzeugen={antragErzeugen ? () => antragErzeugen(ausgewaehlt) : null}
    schliessen={() => (ausgewaehlt = null)}
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
  .anzahl {
    color: #5e6c84;
    font-size: 0.9rem;
    font-weight: 400;
    margin-left: 8px;
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

  .raster {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(280px, 1fr));
    gap: 16px;
  }
  .leer {
    color: #5e6c84;
  }
</style>
