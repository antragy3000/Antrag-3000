<script>
  // Kalender-Kachel der Übersicht: Monatsraster mit einem farbigen Punkt je
  // Termin-Art. Ein Klick auf einen Tag zeigt seine Termine darunter und
  // erlaubt, dort eine interne Frist anzulegen. Das Raster ist bewusst
  // selbst gebaut (rund 100 Zeilen) statt eine Bibliothek einzubinden.
  import { TERMIN_ARTEN, termineJeTag, isoTag } from "$lib/termine";
  import { monatsRaster } from "$lib/uebersicht";
  import { datumText } from "$lib/abrechnung";

  let {
    termine = [],
    interneAnlegen, // ({ datum, titel }) – wie im Bereich „Fristen"
    zuZiel, // (ziel) – springt an die Stelle, zu der ein Termin gehört
  } = $props();

  const WOCHENTAGE = ["Mo", "Di", "Mi", "Do", "Fr", "Sa", "So"];
  const HEUTE = new Date();
  const HEUTE_ISO = isoTag(HEUTE);

  let jahr = $state(HEUTE.getFullYear());
  let monat = $state(HEUTE.getMonth()); // 0-basiert
  let gewaehlt = $state(null); // "JJJJ-MM-TT" oder null
  let neuTitel = $state("");
  let speichert = $state(false);

  let zellen = $derived(monatsRaster(jahr, monat));
  let jeTag = $derived(termineJeTag(termine));
  let monatsName = $derived(
    new Date(jahr, monat, 1).toLocaleDateString("de-DE", { month: "long", year: "numeric" })
  );
  // Die Legende nennt nur Arten, die es in diesem Projekt auch gibt.
  let legende = $derived(
    Object.entries(TERMIN_ARTEN).filter(([art]) => termine.some((t) => t.art === art))
  );

  // Je Tag höchstens ein Punkt pro Art – sonst wird die Zelle unleserlich.
  function artenAmTag(iso) {
    return [...new Set((jeTag[iso] ?? []).map((t) => t.art))];
  }

  function tagBeschreibung(z) {
    const liste = jeTag[z.iso] ?? [];
    const datum = datumText(z.iso);
    if (!liste.length) return `${datum}, keine Termine`;
    return `${datum}: ${liste.map((t) => `${TERMIN_ARTEN[t.art]} ${t.titel}`).join(", ")}`;
  }

  function blaettern(schritt) {
    const d = new Date(jahr, monat + schritt, 1);
    jahr = d.getFullYear();
    monat = d.getMonth();
    gewaehlt = null;
  }
  function zuHeute() {
    jahr = HEUTE.getFullYear();
    monat = HEUTE.getMonth();
    gewaehlt = HEUTE_ISO;
  }
  function tagWaehlen(iso) {
    gewaehlt = gewaehlt === iso ? null : iso;
    neuTitel = "";
  }

  async function anlegen(event) {
    event.preventDefault();
    if (speichert) return;
    speichert = true;
    try {
      await interneAnlegen({ datum: gewaehlt, titel: neuTitel });
      neuTitel = "";
    } finally {
      speichert = false;
    }
  }
</script>

<section class="kachel">
  <div class="kopf">
    <h3>{monatsName}</h3>
    <div class="blaettern">
      <button class="leise" onclick={() => blaettern(-1)} aria-label="Voriger Monat">◀</button>
      <button class="leise" onclick={zuHeute}>Heute</button>
      <button class="leise" onclick={() => blaettern(1)} aria-label="Nächster Monat">▶</button>
    </div>
  </div>

  <div class="raster">
    {#each WOCHENTAGE as w}
      <span class="wochentag">{w}</span>
    {/each}
    {#each zellen as z}
      {#if z}
        <button
          class="tag"
          class:heute={z.iso === HEUTE_ISO}
          class:gewaehlt={z.iso === gewaehlt}
          aria-label={tagBeschreibung(z)}
          aria-pressed={z.iso === gewaehlt}
          onclick={() => tagWaehlen(z.iso)}
        >
          {z.tag}
          <span class="punkte" aria-hidden="true">
            {#each artenAmTag(z.iso) as art}
              <span class="punkt art-{art}"></span>
            {/each}
          </span>
        </button>
      {:else}
        <span></span>
      {/if}
    {/each}
  </div>

  {#if legende.length}
    <div class="legende">
      {#each legende as [art, text]}
        <span><span class="punkt art-{art}" aria-hidden="true"></span>{text}</span>
      {/each}
    </div>
  {/if}

  {#if gewaehlt}
    <div class="tag-panel">
      <h4>{datumText(gewaehlt)}</h4>
      {#each jeTag[gewaehlt] ?? [] as t (t.id)}
        <button class="termin" onclick={() => zuZiel(t.ziel)}>
          <span class="punkt art-{t.art}" aria-hidden="true"></span>
          <span class="termin-text">
            {t.titel}
            <span class="termin-art">
              {t.untertitel || TERMIN_ARTEN[t.art]}{#if t.projektName} · {t.projektName}{/if}
            </span>
          </span>
        </button>
      {/each}
      <form class="anlegen" onsubmit={anlegen}>
        <input type="text" bind:value={neuTitel} placeholder="Neue interne Frist, z. B. Probe" aria-label="Titel der internen Frist" />
        <button class="zweit" type="submit" disabled={speichert}>Anlegen</button>
      </form>
    </div>
  {:else}
    <p class="hinweis">Klicke auf einen Tag, um seine Termine zu sehen oder eine interne Frist anzulegen.</p>
  {/if}
</section>

<style>
  .kachel {
    min-width: 0;
    background: var(--weiss);
    border: 1px solid var(--rand);
    border-radius: 12px;
    padding: 16px 18px;
  }
  .kopf {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    margin-bottom: 10px;
  }
  h3 {
    margin: 0;
    font-size: 1rem;
    font-weight: 600;
    color: var(--text);
  }
  h4 {
    margin: 0 0 6px;
    font-size: 0.88rem;
    font-weight: 600;
    color: var(--text);
  }
  .blaettern {
    display: flex;
    align-items: center;
    gap: 2px;
  }
  button.leise {
    width: auto;
    margin: 0;
    background: none;
    border: none;
    color: var(--text-muted);
    font-size: 0.84rem;
    font-family: inherit;
    cursor: pointer;
    padding: 4px 6px;
    border-radius: 6px;
  }
  button.leise:hover {
    color: var(--text);
    background: var(--flaeche-3);
  }
  .raster {
    display: grid;
    grid-template-columns: repeat(7, minmax(0, 1fr));
    gap: 2px;
  }
  .wochentag {
    text-align: center;
    font-size: 0.74rem;
    color: var(--text-muted);
    padding: 2px 0 4px;
  }
  button.tag {
    width: auto;
    margin: 0;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 2px;
    min-height: 40px;
    padding: 5px 0 4px;
    font-family: inherit;
    font-size: 0.84rem;
    font-weight: 400;
    color: var(--text);
    background: none;
    border: 1px solid transparent;
    border-radius: 6px;
    cursor: pointer;
  }
  button.tag:hover {
    background: var(--flaeche-3);
  }
  button.tag.heute {
    background: var(--akzent-bg);
    color: var(--akzent-text);
    font-weight: 700;
  }
  button.tag.gewaehlt {
    border-color: var(--akzent);
  }
  .punkte {
    display: flex;
    gap: 2px;
    min-height: 6px;
  }
  .punkt {
    display: inline-block;
    flex: none;
    width: 6px;
    height: 6px;
    border-radius: 50%;
  }
  .art-foerderung { background: var(--akzent-hell); }
  .art-intern { background: var(--lila); }
  .art-team { background: var(--grau-4); }
  .art-nachweis { background: var(--warnung); }
  .legende {
    display: flex;
    flex-wrap: wrap;
    gap: 4px 12px;
    margin-top: 10px;
    font-size: 0.76rem;
    color: var(--text-muted);
  }
  .legende .punkt {
    margin-right: 5px;
    vertical-align: 1px;
  }
  .hinweis {
    margin: 12px 0 0;
    font-size: 0.8rem;
    line-height: 1.5;
    color: var(--text-muted);
  }
  .tag-panel {
    margin-top: 12px;
    padding-top: 12px;
    border-top: 1px solid var(--flaeche-3);
  }
  button.termin {
    width: 100%;
    margin: 0;
    display: flex;
    align-items: baseline;
    gap: 8px;
    padding: 6px;
    font-family: inherit;
    font-size: 0.86rem;
    font-weight: 400;
    text-align: left;
    color: var(--text);
    background: none;
    border: none;
    border-radius: 6px;
    cursor: pointer;
  }
  button.termin:hover {
    background: var(--flaeche-3);
  }
  .termin-text {
    min-width: 0;
    overflow-wrap: anywhere;
  }
  .termin-art {
    display: block;
    font-size: 0.76rem;
    color: var(--text-muted);
  }
  .anlegen {
    display: flex;
    gap: 8px;
    margin-top: 8px;
  }
  .anlegen input {
    flex: 1;
    min-width: 0;
    padding: 7px 10px;
    font-family: inherit;
    font-size: 0.86rem;
    color: var(--text);
    background: var(--weiss);
    border: 2px solid var(--rand);
    border-radius: 8px;
  }
  .anlegen input:focus {
    outline: none;
    border-color: var(--akzent);
  }
  button.zweit {
    width: auto;
    margin: 0;
    padding: 7px 12px;
    font-family: inherit;
    font-size: 0.86rem;
    font-weight: 600;
    color: var(--text);
    background: var(--weiss);
    border: 2px solid var(--rand);
    border-radius: 8px;
    cursor: pointer;
  }
  button.zweit:hover:not(:disabled) {
    border-color: var(--akzent);
  }
</style>
