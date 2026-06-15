<script>
  // "Passende für mich": zeigt den Fragebogen (falls noch keine
  // Antworten da sind) oder das Matching-Ergebnis in zwei Abschnitten:
  // Rangliste der passenden Förderungen + "Weitere Vorschläge" mit
  // Begründung, welches harte Kriterium nicht erfüllt ist.
  import { katalog } from "$lib/katalog.svelte.js";
  import { matchen } from "$lib/matching";
  import Fragebogen from "./Fragebogen.svelte";
  import FoerderKarte from "./FoerderKarte.svelte";
  import FoerderDetail from "./FoerderDetail.svelte";

  let { antworten = null, speichern, merkliste = [], umschalten = null, oeffneKatalog = null, standFuer = null, neuFelderFuer = null } = $props();

  let bearbeiten = $state(false);
  let ausgewaehlt = $state(null);

  let ergebnis = $derived(
    antworten && !bearbeiten ? matchen(katalog.daten.foerderungen, antworten) : null
  );

  async function fragebogenFertig(neue) {
    await speichern(neue);
    bearbeiten = false;
  }
</script>

{#if !antworten || bearbeiten}
  <Fragebogen
    start={antworten}
    fertig={fragebogenFertig}
    abbrechen={antworten ? () => (bearbeiten = false) : null}
  />
{:else if ergebnis}
  <div class="bereich">
    <div class="kopfzeile">
      <h2>
        Passende Förderungen
        <span class="anzahl">{ergebnis.passende.length}</span>
      </h2>
      <div class="kopf-knoepfe">
        {#if oeffneKatalog}
          <button class="db-knopf" onclick={oeffneKatalog}>🗂 Förder-Datenbank</button>
        {/if}
        <button class="leise" onclick={() => (bearbeiten = true)}>
          Fragebogen ändern
        </button>
      </div>
    </div>

    <div class="raster">
      {#each ergebnis.passende as e (e.foerderung.id)}
        <FoerderKarte
          foerderung={e.foerderung}
          punkte={e.punkte}
          treffer={e.treffer}
          gemerkt={merkliste.includes(e.foerderung.id)}
          merken={umschalten}
          auswaehlen={(f) => (ausgewaehlt = f)}
          stand={standFuer ? standFuer(e.foerderung.id) : null}
          geaenderteFelder={neuFelderFuer ? neuFelderFuer(e.foerderung.id) : []}
        />
      {:else}
        <p class="leer">
          Keine Förderung erfüllt aktuell alle harten Kriterien.
          Schau unter "Weitere Vorschläge", woran es jeweils liegt.
        </p>
      {/each}
    </div>

    {#if ergebnis.weitere.length}
      <h2 class="weitere-titel">
        Weitere Vorschläge
        <span class="anzahl">{ergebnis.weitere.length}</span>
      </h2>
      <p class="erklaerung">
        Diese Förderungen erfüllen mindestens ein hartes Kriterium nicht –
        vielleicht hilft eine Kooperation oder ein anderer Projektzuschnitt.
      </p>
      <div class="raster">
        {#each ergebnis.weitere as e (e.foerderung.id)}
          <FoerderKarte
            foerderung={e.foerderung}
            gruende={e.gruende}
            gemerkt={merkliste.includes(e.foerderung.id)}
            merken={umschalten}
            auswaehlen={(f) => (ausgewaehlt = f)}
            stand={standFuer ? standFuer(e.foerderung.id) : null}
            geaenderteFelder={neuFelderFuer ? neuFelderFuer(e.foerderung.id) : []}
          />
        {/each}
      </div>
    {/if}
  </div>
{/if}

{#if ausgewaehlt}
  <FoerderDetail
    foerderung={ausgewaehlt}
    alle={katalog.daten.foerderungen}
    hinweis={katalog.daten.hinweis}
    gemerkt={merkliste.includes(ausgewaehlt.id)}
    umschalten={umschalten}
    schliessen={() => (ausgewaehlt = null)}
    stand={standFuer ? standFuer(ausgewaehlt.id) : null}
    geaenderteFelder={neuFelderFuer ? neuFelderFuer(ausgewaehlt.id) : []}
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
  .weitere-titel {
    margin-top: 44px;
  }
  .erklaerung {
    color: #5e6c84;
    font-size: 0.88rem;
    margin: 6px 0 20px;
  }
  .raster {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(280px, 1fr));
    gap: 16px;
  }
  .leer {
    color: #5e6c84;
    grid-column: 1 / -1;
  }
  .leise {
    background: none;
    border: none;
    color: #5e6c84;
    font-size: 0.875rem;
    cursor: pointer;
    padding: 6px;
    font-family: inherit;
  }
  .leise:hover {
    color: #172b4d;
    text-decoration: underline;
  }
  .kopf-knoepfe {
    display: flex;
    align-items: center;
    gap: 6px;
  }
  .db-knopf {
    background: #f1f2f4;
    border: none;
    border-radius: 6px;
    padding: 5px 11px;
    font-size: 0.82rem;
    font-weight: 600;
    font-family: inherit;
    color: #44546f;
    cursor: pointer;
  }
  .db-knopf:hover {
    background: #e4e7ec;
    color: #172b4d;
  }
</style>
