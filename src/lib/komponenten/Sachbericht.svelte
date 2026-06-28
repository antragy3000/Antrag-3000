<script>
  // Abrechnungs-Modus: EIN projektweiter Sachbericht für den
  // Verwendungsnachweis. Wird in den Nachweis JEDER Geldquelle übernommen.
  // Tresor-Inhalt (bleibt lokal).
  let { sachbericht = "", speichern, projektName = "" } = $props();

  let text = $state(sachbericht);
  let beschaeftigt = $state(false);
  let gespeichert = $state(false);

  let veraendert = $derived(text !== sachbericht);

  async function sichern() {
    if (!veraendert) return;
    beschaeftigt = true;
    try {
      await speichern(text);
      gespeichert = true;
      setTimeout(() => (gespeichert = false), 1600);
    } finally {
      beschaeftigt = false;
    }
  }
</script>

<div class="bereich">
  <div class="kopfzeile">
    <div class="titel-block">
      <h2>Sachbericht</h2>
      <p class="untertitel">
        Ein durchgehender Bericht{#if projektName} für <strong>{projektName}</strong>{/if}: was wurde
        umgesetzt, welche Ziele erreicht, Besonderheiten. Er erscheint im Verwendungsnachweis
        <strong>jeder</strong> Geldquelle.
      </p>
    </div>
    <div class="status">
      {#if gespeichert}<span class="ok">✓ gespeichert</span>{/if}
      <button class="primaer" onclick={sichern} disabled={!veraendert || beschaeftigt}>
        {beschaeftigt ? "Speichert …" : "Speichern"}
      </button>
    </div>
  </div>

  <textarea
    bind:value={text}
    onblur={sichern}
    placeholder={"Sachbericht zum Projekt …\n\n• Ausgangslage und Ziele\n• Ablauf / Durchführung\n• Ergebnisse und Wirkung\n• Besonderheiten, Abweichungen vom Plan"}
  ></textarea>

  <div class="fuss">
    <span class="zeichen">{text.length} Zeichen</span>
    <span class="hinweis">Wird automatisch beim Verlassen des Feldes gespeichert.</span>
  </div>
</div>

<style>
  .bereich {
    max-width: 920px;
    margin: 0 auto;
    padding: 32px 24px 64px;
    display: flex;
    flex-direction: column;
    min-height: 70vh;
  }
  .kopfzeile {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 16px;
    flex-wrap: wrap;
    margin-bottom: 16px;
  }
  .titel-block { flex: 1 1 300px; min-width: 260px; }
  h2 { margin: 0 0 4px; font-size: 1.35rem; font-weight: 600; color: #172b4d; }
  .untertitel { margin: 0; color: #5e6c84; font-size: 0.9rem; max-width: 560px; line-height: 1.5; }
  .status { display: flex; align-items: center; gap: 12px; }
  .ok { color: #14794e; font-size: 0.85rem; font-weight: 600; white-space: nowrap; }

  textarea {
    flex: 1;
    width: 100%;
    min-height: 340px;
    padding: 16px 18px;
    border: 2px solid #dfe1e6;
    border-radius: 12px;
    font-family: inherit;
    font-size: 0.95rem;
    line-height: 1.6;
    color: #172b4d;
    background: #fff;
    resize: vertical;
    box-sizing: border-box;
  }
  textarea:focus { outline: none; border-color: #4f6df5; }

  .fuss {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-top: 10px;
  }
  .zeichen { color: #8590a2; font-size: 0.8rem; }
  .hinweis { color: #8590a2; font-size: 0.8rem; }

  button.primaer {
    padding: 10px 20px;
    font-size: 0.92rem;
    font-weight: 600;
    font-family: inherit;
    color: #fff;
    background: #4f6df5;
    border: none;
    border-radius: 8px;
    cursor: pointer;
  }
  button.primaer:hover:not(:disabled) { background: #3d5bf0; }
  button.primaer:disabled { background: #c1c7d0; cursor: default; }
</style>
