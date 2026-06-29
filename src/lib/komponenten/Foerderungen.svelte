<script>
  // Förder-Katalog: unkritische Sync-Ebene (KEIN Tresor-Inhalt). Kommt
  // aus dem zentralen, zur Laufzeit aktualisierbaren Katalog-Store, damit
  // ein Update (Phase 3) hier sofort wirkt.
  import { katalog } from "$lib/katalog.svelte.js";
  import { LAENDER, SPARTEN } from "$lib/begriffe";
  import FoerderKarte from "./FoerderKarte.svelte";
  import FoerderDetail from "./FoerderDetail.svelte";

  // Deaktivierte Förderungen (aktiv === false) blendet die Browse-Liste aus.
  let foerderungen = $derived(katalog.daten.foerderungen.filter((f) => f.aktiv !== false));

  // merkliste = null bedeutet: kein aktives Projekt, Sterne ausblenden.
  let { merkliste = null, umschalten = null, oeffneKatalog = null, standFuer = null, neuFelderFuer = null } = $props();

  let suche = $state("");
  let ausgewaehlt = $state(null);

  let gefiltert = $derived(
    foerderungen.filter((f) => {
      const s = suche.trim().toLowerCase();
      if (!s) return true;
      const heuhaufen = [
        f.name,
        f.foerdergeber,
        LAENDER[f.land] ?? f.land,
        ...f.weiche_kriterien.sparten.map((sp) => SPARTEN[sp] ?? sp),
      ]
        .join(" ")
        .toLowerCase();
      return heuhaufen.includes(s);
    })
  );
</script>

<div class="bereich">
  <div class="kopfzeile">
    <h2>Förderungen <span class="anzahl">{gefiltert.length} von {foerderungen.length}</span></h2>
    <input
      type="search"
      placeholder="Suchen (Name, Geber, Land, Sparte) …"
      bind:value={suche}
    />
  </div>

  <p class="datenstand">
    Beispieldaten, Stand {new Date(katalog.daten.stand).toLocaleDateString("de-DE")} –
    vor Antragstellung immer beim Fördergeber prüfen.
    {#if oeffneKatalog}
      <button class="db-knopf" onclick={oeffneKatalog}>🗂 Förder-Datenbank</button>
    {/if}
  </p>

  <div class="raster">
    {#each gefiltert as f (f.id)}
      <FoerderKarte
        foerderung={f}
        gemerkt={merkliste?.includes(f.id) ?? null}
        merken={merkliste ? umschalten : null}
        auswaehlen={(x) => (ausgewaehlt = x)}
        stand={standFuer ? standFuer(f.id) : null}
        geaenderteFelder={neuFelderFuer ? neuFelderFuer(f.id) : []}
      />
    {:else}
      <p class="leer">Keine Förderung passt zu deiner Suche.</p>
    {/each}
  </div>
</div>

{#if ausgewaehlt}
  <FoerderDetail
    foerderung={ausgewaehlt}
    alle={foerderungen}
    hinweis={katalog.daten.hinweis}
    gemerkt={merkliste?.includes(ausgewaehlt.id) ?? null}
    umschalten={merkliste ? umschalten : null}
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
  }
  h2 {
    margin: 0;
    font-size: 1.35rem;
    font-weight: 600;
  }
  .anzahl {
    color: var(--text-muted);
    font-size: 0.9rem;
    font-weight: 400;
    margin-left: 8px;
  }

  input[type="search"] {
    flex: 1;
    max-width: 360px;
    padding: 9px 14px;
    font-size: 0.95rem;
    border: 2px solid var(--rand);
    border-radius: 8px;
    background: var(--weiss);
  }
  input[type="search"]:focus {
    outline: none;
    border-color: var(--akzent);
  }

  .datenstand {
    color: var(--text-leise);
    font-size: 0.8rem;
    margin: 10px 0 24px;
  }
  .db-knopf {
    margin-left: 8px;
    background: var(--flaeche-2b);
    border: none;
    border-radius: 6px;
    padding: 3px 9px;
    font-size: 0.78rem;
    font-weight: 600;
    font-family: inherit;
    color: var(--text-2);
    cursor: pointer;
  }
  .db-knopf:hover {
    background: var(--rand-2);
    color: var(--text);
  }

  .raster {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(280px, 1fr));
    gap: 16px;
  }

  .leer {
    color: var(--text-muted);
    grid-column: 1 / -1;
  }
</style>
