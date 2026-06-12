<script>
  // Förder-Datenbank: unkritische Sync-Ebene, daher eine normale
  // JSON-Datei (KEIN Tresor-Inhalt). In Phase 2 kommt genau diese
  // Struktur von der NAS.
  import datenbank from "$lib/daten/foerderungen.json";
  import { LAENDER, SPARTEN } from "$lib/begriffe";
  import FoerderKarte from "./FoerderKarte.svelte";
  import FoerderDetail from "./FoerderDetail.svelte";

  const foerderungen = datenbank.foerderungen;

  // merkliste = null bedeutet: kein aktives Projekt, Sterne ausblenden.
  let { merkliste = null, umschalten = null } = $props();

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
    Beispieldaten, Stand {new Date(datenbank.stand).toLocaleDateString("de-DE")} –
    vor Antragstellung immer beim Fördergeber prüfen.
  </p>

  <div class="raster">
    {#each gefiltert as f (f.id)}
      <FoerderKarte
        foerderung={f}
        gemerkt={merkliste?.includes(f.id) ?? null}
        merken={merkliste ? umschalten : null}
        auswaehlen={(x) => (ausgewaehlt = x)}
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
    hinweis={datenbank.hinweis}
    gemerkt={merkliste?.includes(ausgewaehlt.id) ?? null}
    umschalten={merkliste ? umschalten : null}
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

  input[type="search"] {
    flex: 1;
    max-width: 360px;
    padding: 9px 14px;
    font-size: 0.95rem;
    border: 2px solid #dfe1e6;
    border-radius: 8px;
    background: #fff;
  }
  input[type="search"]:focus {
    outline: none;
    border-color: #4f6df5;
  }

  .datenstand {
    color: #8590a2;
    font-size: 0.8rem;
    margin: 10px 0 24px;
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
</style>
