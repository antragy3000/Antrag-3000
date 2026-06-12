<script>
  // Förder-Datenbank: unkritische Sync-Ebene, daher eine normale
  // JSON-Datei (KEIN Tresor-Inhalt). In Phase 2 kommt genau diese
  // Struktur von der NAS.
  import datenbank from "$lib/daten/foerderungen.json";
  import { openUrl } from "@tauri-apps/plugin-opener";

  const foerderungen = datenbank.foerderungen;

  let suche = $state("");
  let ausgewaehlt = $state(null);

  // Klartext-Namen für die maschinenlesbaren Codes
  const LAENDER = { DE: "Deutschland", AT: "Österreich", CH: "Schweiz", INT: "International" };
  const SPARTEN = {
    musik: "Musik",
    theater: "Theater",
    tanz: "Tanz",
    performance: "Performance",
    bildende_kunst: "Bildende Kunst",
    medienkunst: "Medienkunst",
    literatur: "Literatur",
    film: "Film",
    interdisziplinaer: "Interdisziplinär",
  };
  const PROJEKTARTEN = {
    produktion: "Produktion",
    recherche_entwicklung: "Recherche & Entwicklung",
    residenz: "Residenz",
    gastspiel_tournee: "Gastspiel / Tournee",
    veroeffentlichung: "Veröffentlichung",
    vermittlung: "Vermittlung",
  };
  const TRAEGERSCHAFT = {
    einzelperson: "Einzelperson",
    gruppe: "Gruppe / GbR",
    organisation: "Verein / Organisation",
  };

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

  function nameVon(id) {
    const f = foerderungen.find((x) => x.id === id);
    return f ? `${f.name} (${f.foerdergeber})` : id;
  }

  function fristText(f) {
    if (f.weiche_kriterien.zeitpunkt === "laufend") return "laufend einreichbar";
    return f.fristen.length
      ? "Frist: " + f.fristen.map((d) => new Date(d).toLocaleDateString("de-DE")).join(", ")
      : "Fristen siehe Webseite";
  }
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
      <button class="foerderkarte" onclick={() => (ausgewaehlt = f)}>
        <span class="land land-{f.land}">{LAENDER[f.land] ?? f.land}</span>
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
        <p class="frist">{fristText(f)}</p>
      </button>
    {:else}
      <p class="leer">Keine Förderung passt zu deiner Suche.</p>
    {/each}
  </div>
</div>

{#if ausgewaehlt}
  <!-- Detailansicht als Überlagerung -->
  <div class="schleier" onclick={() => (ausgewaehlt = null)} role="presentation">
    <article class="detail" onclick={(e) => e.stopPropagation()}>
      <header>
        <div>
          <span class="land land-{ausgewaehlt.land}">{LAENDER[ausgewaehlt.land] ?? ausgewaehlt.land}</span>
          <h3>{ausgewaehlt.name}</h3>
          <p class="geber">{ausgewaehlt.foerdergeber}</p>
        </div>
        <button class="schliessen" onclick={() => (ausgewaehlt = null)} aria-label="Schließen">✕</button>
      </header>

      <p>{ausgewaehlt.beschreibung}</p>

      <dl>
        <dt>Förderhöhe</dt>
        <dd>{ausgewaehlt.foerderhoehe_text}</dd>

        <dt>Einreichung</dt>
        <dd>{fristText(ausgewaehlt)}</dd>

        <dt>Wohnsitz-Anforderung</dt>
        <dd>
          {ausgewaehlt.harte_kriterien.wohnsitz.length
            ? ausgewaehlt.harte_kriterien.wohnsitz.map((l) => LAENDER[l] ?? l).join(", ")
            : "keine Anforderung"}
        </dd>

        <dt>Durchführungsort</dt>
        <dd>
          {ausgewaehlt.harte_kriterien.durchfuehrungsort.length
            ? ausgewaehlt.harte_kriterien.durchfuehrungsort.map((l) => LAENDER[l] ?? l).join(", ")
            : "frei"}
        </dd>

        <dt>Wer kann beantragen?</dt>
        <dd>
          {ausgewaehlt.harte_kriterien.traegerschaft.map((t) => TRAEGERSCHAFT[t] ?? t).join(", ")}
          · studentisch: {ausgewaehlt.harte_kriterien.studentisch_erlaubt ? "ja" : "nein"}
        </dd>

        <dt>Sparten</dt>
        <dd>{ausgewaehlt.weiche_kriterien.sparten.map((s) => SPARTEN[s] ?? s).join(", ")}</dd>

        <dt>Projektarten</dt>
        <dd>{ausgewaehlt.weiche_kriterien.projektarten.map((p) => PROJEKTARTEN[p] ?? p).join(", ")}</dd>

        {#if ausgewaehlt.unvertraeglich_mit.length}
          <dt class="warn-dt">Unverträglich mit</dt>
          <dd class="warn-dd">
            {ausgewaehlt.unvertraeglich_mit.map(nameVon).join("; ")}
          </dd>
        {/if}

        <dt>Übliche Unterlagen</dt>
        <dd>
          <ul>
            {#each ausgewaehlt.checkliste_vorschlag as punkt}
              <li>{punkt}</li>
            {/each}
          </ul>
        </dd>
      </dl>

      <p class="datenstand">{datenbank.hinweis}</p>

      <button class="primaer" onclick={() => openUrl(ausgewaehlt.webseite)}>
        Webseite des Fördergebers öffnen
      </button>
    </article>
  </div>
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
  .foerderkarte h3 {
    margin: 0;
    font-size: 1.02rem;
    font-weight: 600;
    line-height: 1.35;
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

  .leer {
    color: #5e6c84;
    grid-column: 1 / -1;
  }

  /* Detail-Überlagerung */
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
  .detail h3 {
    margin: 8px 0 2px;
    font-size: 1.25rem;
  }
  .detail p {
    line-height: 1.55;
    font-size: 0.95rem;
  }

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
  .warn-dt { color: #ae2e24; }
  .warn-dd { color: #ae2e24; }

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
