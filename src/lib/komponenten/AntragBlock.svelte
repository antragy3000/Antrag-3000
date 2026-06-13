<script>
  // Antrag-Status (eine Förderoption) + Checkliste der benötigten
  // Dokumente, je mit eigenem Status. Wird in der Detailansicht einer
  // gemerkten Förderung gezeigt. Mutiert das antrag-Objekt (Tresor)
  // und ruft danach aendern() zum verschlüsselten Speichern.
  import { ANTRAG_STATUS, CHECK_STATUS, statusFarbe } from "$lib/status";

  let { antrag, aendern } = $props();

  let neuerPunkt = $state("");

  function punktHinzufuegen(event) {
    event.preventDefault();
    const t = neuerPunkt.trim();
    if (!t) return;
    antrag.checkliste.push({ text: t, status: "noch_nicht", statusFrei: "" });
    neuerPunkt = "";
    aendern();
  }
  function punktEntfernen(i) {
    antrag.checkliste.splice(i, 1);
    aendern();
  }
</script>

<section class="antrag">
  <div class="status-zeile">
    <h4>Antrag-Status</h4>
    <span class="punkt farbe-{statusFarbe(ANTRAG_STATUS, antrag.status)}"></span>
    <select bind:value={antrag.status} onchange={aendern}>
      {#each ANTRAG_STATUS as s (s.key)}
        <option value={s.key}>{s.label}</option>
      {/each}
    </select>
  </div>
  {#if antrag.status === "anderer"}
    <input
      class="frei"
      type="text"
      placeholder="Status frei beschriften …"
      bind:value={antrag.statusFrei}
      onchange={aendern}
    />
  {/if}

  <h4 class="check-titel">Benötigte Dokumente</h4>
  {#if antrag.checkliste.length === 0}
    <p class="leer">Noch keine Punkte. Füge unten die nötigen Dokumente hinzu.</p>
  {/if}
  <ul class="checkliste">
    {#each antrag.checkliste as punkt, i (punkt)}
      <li>
        <span class="punkt farbe-{statusFarbe(CHECK_STATUS, punkt.status)}"></span>
        <div class="punkt-inhalt">
          <span class="punkt-text" class:fertig={punkt.status === "abgeschlossen"}>
            {punkt.text}
          </span>
          <div class="punkt-status">
            <select bind:value={punkt.status} onchange={aendern}>
              {#each CHECK_STATUS as s (s.key)}
                <option value={s.key}>{s.label}</option>
              {/each}
            </select>
            {#if punkt.status === "anderer"}
              <input
                class="frei"
                type="text"
                placeholder="frei beschriften …"
                bind:value={punkt.statusFrei}
                onchange={aendern}
              />
            {/if}
          </div>
        </div>
        <button class="entfernen" title="Punkt entfernen" onclick={() => punktEntfernen(i)}>
          ✕
        </button>
      </li>
    {/each}
  </ul>

  <form class="hinzufuegen" onsubmit={punktHinzufuegen}>
    <input type="text" placeholder="Weiteres Dokument …" bind:value={neuerPunkt} />
    <button type="submit" disabled={!neuerPunkt.trim()}>+ Hinzufügen</button>
  </form>
</section>

<style>
  .antrag {
    border-top: 1px solid #dfe1e6;
    margin-top: 20px;
    padding-top: 20px;
  }
  h4 {
    margin: 0;
    font-size: 0.95rem;
    font-weight: 600;
  }
  .status-zeile {
    display: flex;
    align-items: center;
    gap: 10px;
    flex-wrap: wrap;
  }
  .check-titel {
    margin: 24px 0 10px;
  }

  select {
    padding: 8px 10px;
    font-size: 0.9rem;
    font-family: inherit;
    color: #172b4d;
    border: 2px solid #dfe1e6;
    border-radius: 8px;
    background: #fafbfc;
  }
  select:focus {
    outline: none;
    border-color: #4f6df5;
    background: #fff;
  }
  .frei {
    width: 100%;
    box-sizing: border-box;
    margin-top: 8px;
    padding: 8px 10px;
    font-size: 0.9rem;
    font-family: inherit;
    border: 2px solid #dfe1e6;
    border-radius: 8px;
    background: #fafbfc;
  }
  .frei:focus {
    outline: none;
    border-color: #4f6df5;
    background: #fff;
  }

  .leer {
    color: #5e6c84;
    font-size: 0.9rem;
    margin: 0 0 10px;
  }
  .checkliste {
    list-style: none;
    margin: 0;
    padding: 0;
  }
  .checkliste li {
    display: flex;
    align-items: flex-start;
    gap: 10px;
    padding: 10px 0;
    border-bottom: 1px solid #f1f2f4;
  }
  .punkt-inhalt {
    flex: 1;
    min-width: 0;
  }
  .punkt-text {
    display: block;
    font-size: 0.92rem;
    margin-bottom: 6px;
  }
  .punkt-text.fertig {
    color: #5e6c84;
    text-decoration: line-through;
  }
  .punkt-status {
    display: flex;
    align-items: center;
    gap: 8px;
    flex-wrap: wrap;
  }
  .punkt-status .frei {
    width: auto;
    flex: 1;
    margin-top: 0;
  }

  /* runder Farbpunkt je Status */
  .punkt {
    width: 11px;
    height: 11px;
    border-radius: 50%;
    flex-shrink: 0;
    margin-top: 4px;
  }
  .farbe-blau { background: #4f6df5; }
  .farbe-lila { background: #8270db; }
  .farbe-gruen { background: #22a06b; }
  .farbe-rot { background: #ca3521; }
  .farbe-gelb { background: #e2a400; }
  .farbe-grau { background: #b3bac5; }

  .entfernen {
    background: none;
    border: none;
    color: #8590a2;
    font-size: 0.95rem;
    cursor: pointer;
    padding: 4px 8px;
    border-radius: 6px;
    flex-shrink: 0;
  }
  .entfernen:hover {
    background: #ffeceb;
    color: #ae2e24;
  }

  .hinzufuegen {
    display: flex;
    gap: 8px;
    margin-top: 14px;
  }
  .hinzufuegen input {
    flex: 1;
    box-sizing: border-box;
    padding: 9px 12px;
    font-size: 0.9rem;
    font-family: inherit;
    border: 2px solid #dfe1e6;
    border-radius: 8px;
    background: #fafbfc;
  }
  .hinzufuegen input:focus {
    outline: none;
    border-color: #4f6df5;
    background: #fff;
  }
  .hinzufuegen button {
    padding: 9px 16px;
    font-size: 0.9rem;
    font-weight: 600;
    font-family: inherit;
    color: #fff;
    background: #4f6df5;
    border: none;
    border-radius: 8px;
    cursor: pointer;
  }
  .hinzufuegen button:hover:not(:disabled) {
    background: #3d5bf0;
  }
  .hinzufuegen button:disabled {
    background: #c1c7d0;
    cursor: default;
  }
</style>
