<script>
  // Detailansicht einer Förderung als Überlagerung.
  import { openUrl } from "@tauri-apps/plugin-opener";
  import { LAENDER, SPARTEN, PROJEKTARTEN, TRAEGERSCHAFT, fristText } from "$lib/begriffe";

  let { foerderung: f, alle, hinweis, schliessen } = $props();

  function nameVon(id) {
    const x = alle.find((e) => e.id === id);
    return x ? `${x.name} (${x.foerdergeber})` : id;
  }
</script>

<div class="schleier" onclick={schliessen} role="presentation">
  <article class="detail" onclick={(e) => e.stopPropagation()}>
    <header>
      <div>
        <span class="land land-{f.land}">{LAENDER[f.land] ?? f.land}</span>
        <h3>{f.name}</h3>
        <p class="geber">{f.foerdergeber}</p>
      </div>
      <button class="schliessen" onclick={schliessen} aria-label="Schließen">✕</button>
    </header>

    <p>{f.beschreibung}</p>

    <dl>
      <dt>Förderhöhe</dt>
      <dd>{f.foerderhoehe_text}</dd>

      <dt>Einreichung</dt>
      <dd>{fristText(f)}</dd>

      <dt>Wohnsitz-Anforderung</dt>
      <dd>
        {f.harte_kriterien.wohnsitz.length
          ? f.harte_kriterien.wohnsitz.map((l) => LAENDER[l] ?? l).join(", ")
          : "keine Anforderung"}
      </dd>

      <dt>Durchführungsort</dt>
      <dd>
        {f.harte_kriterien.durchfuehrungsort.length
          ? f.harte_kriterien.durchfuehrungsort.map((l) => LAENDER[l] ?? l).join(", ")
          : "frei"}
      </dd>

      <dt>Wer kann beantragen?</dt>
      <dd>
        {f.harte_kriterien.traegerschaft.map((t) => TRAEGERSCHAFT[t] ?? t).join(", ")}
        · studentisch: {f.harte_kriterien.studentisch_erlaubt ? "ja" : "nein"}
      </dd>

      <dt>Sparten</dt>
      <dd>{f.weiche_kriterien.sparten.map((s) => SPARTEN[s] ?? s).join(", ") || "spartenoffen"}</dd>

      <dt>Projektarten</dt>
      <dd>{f.weiche_kriterien.projektarten.map((p) => PROJEKTARTEN[p] ?? p).join(", ")}</dd>

      {#if f.unvertraeglich_mit.length}
        <dt class="warn">Unverträglich mit</dt>
        <dd class="warn">{f.unvertraeglich_mit.map(nameVon).join("; ")}</dd>
      {/if}

      <dt>Übliche Unterlagen</dt>
      <dd>
        <ul>
          {#each f.checkliste_vorschlag as punkt}
            <li>{punkt}</li>
          {/each}
        </ul>
      </dd>
    </dl>

    <p class="datenstand">{hinweis}</p>

    <button class="primaer" onclick={() => openUrl(f.webseite)}>
      Webseite des Fördergebers öffnen
    </button>
  </article>
</div>

<style>
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
    color: #5e6c84;
    font-size: 0.85rem;
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
  .land-DE { background: #e9f2ff; color: #0055cc; }
  .land-AT { background: #ffeceb; color: #ae2e24; }
  .land-CH { background: #fff7d6; color: #7f5f01; }
  .land-INT { background: #f3f0ff; color: #5e4db2; }

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
  .warn {
    color: #ae2e24;
  }

  .datenstand {
    color: #8590a2;
    font-size: 0.8rem;
  }

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
