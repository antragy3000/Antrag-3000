<script>
  // Antrag-Status (eine Förderoption) + Checkliste der benötigten
  // Dokumente, je mit eigenem Status. Wird in der Detailansicht einer
  // gemerkten Förderung gezeigt. Mutiert das antrag-Objekt (Tresor)
  // und ruft danach aendern() zum verschlüsselten Speichern.
  import { ANTRAG_STATUS, CHECK_STATUS, statusFarbe } from "$lib/status";

  let { antrag, aendern, hochladen = null } = $props();

  let neuerPunkt = $state("");
  let neueFrist = $state("");
  let neuerFristTitel = $state("");
  // Index des Punkts, dessen Datei gerade hochgeladen wird (-1 = keiner).
  let laedtIdx = $state(-1);

  function punktHinzufuegen(event) {
    event.preventDefault();
    const t = neuerPunkt.trim();
    if (!t) return;
    antrag.checkliste.push({ text: t, status: "noch_nicht", statusFrei: "", datei: "" });
    neuerPunkt = "";
    aendern();
  }

  // Dokument zu einem Checklisten-Punkt hochladen (Datei-Dialog +
  // Kopie in den Förderer-Ordner; merkt sich nur den Dateinamen).
  async function dateiHochladen(i) {
    if (!hochladen) return;
    laedtIdx = i;
    try {
      const name = await hochladen(antrag.checkliste[i].text);
      if (name) {
        antrag.checkliste[i].datei = name;
        aendern();
      }
    } finally {
      laedtIdx = -1;
    }
  }
  // Nur die Verknüpfung lösen – die Datei selbst bleibt im Ordner.
  function dateiEntfernen(i) {
    antrag.checkliste[i].datei = "";
    aendern();
  }
  function punktEntfernen(i) {
    antrag.checkliste.splice(i, 1);
    aendern();
  }

  // Offizielle Einreichfrist(en) – editierbar
  function offFristAendern(i, wert) {
    antrag.offizielleFristen[i] = wert;
    aendern();
  }
  function offFristEntfernen(i) {
    antrag.offizielleFristen.splice(i, 1);
    aendern();
  }
  function offFristNeuSlot() {
    antrag.offizielleFristen.push("");
  }

  // Eigene (benannte) Fristen
  function fristHinzufuegen(event) {
    event.preventDefault();
    if (!neueFrist) return;
    antrag.eigeneFristen.push({ datum: neueFrist, titel: neuerFristTitel.trim() });
    antrag.eigeneFristen.sort((a, b) => a.datum.localeCompare(b.datum));
    neueFrist = "";
    neuerFristTitel = "";
    aendern();
  }
  function fristEntfernen(i) {
    antrag.eigeneFristen.splice(i, 1);
    aendern();
  }
  function fristAnzeige(d) {
    return new Date(d).toLocaleDateString("de-DE", {
      day: "2-digit",
      month: "long",
      year: "numeric",
    });
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

  <h4 class="check-titel">Kontakt zum Förderer</h4>
  <div class="kontakt">
    <div class="zwei">
      <div>
        <label for="kt-name">Ansprechpartner:in</label>
        <input id="kt-name" type="text" bind:value={antrag.kontakt.ansprechpartner} onchange={aendern} />
      </div>
      <div>
        <label for="kt-tel">Telefon</label>
        <input id="kt-tel" type="text" bind:value={antrag.kontakt.telefon} onchange={aendern} />
      </div>
    </div>
    <label for="kt-mail">E-Mail</label>
    <input id="kt-mail" type="email" bind:value={antrag.kontakt.email} onchange={aendern} />
    <label for="kt-notiz">Notiz</label>
    <textarea id="kt-notiz" rows="2" bind:value={antrag.kontakt.notiz} onchange={aendern}></textarea>
  </div>

  <h4 class="check-titel">Offizielle Einreichfrist</h4>
  <div class="off-fristen">
    {#each antrag.offizielleFristen ?? [] as d, i (i)}
      <span class="off-frist">
        <input
          type="date"
          value={d}
          onchange={(e) => offFristAendern(i, e.currentTarget.value)}
        />
        <button class="entfernen" title="Frist entfernen" onclick={() => offFristEntfernen(i)}>✕</button>
      </span>
    {/each}
    <button type="button" class="off-add" onclick={offFristNeuSlot}>+ Frist</button>
  </div>
  <p class="hinweis-klein">
    Aus der Datenbank vorbefüllt – falls falsch übernommen, hier korrigieren.
  </p>

  <h4 class="check-titel">Eigene Fristen</h4>
  {#if (antrag.eigeneFristen ?? []).length === 0}
    <p class="leer">Keine eigenen Fristen. Trage unten z. B. interne Abgabetermine ein.</p>
  {/if}
  <ul class="fristen">
    {#each antrag.eigeneFristen ?? [] as f, i (i)}
      <li>
        <span class="frist-datum">
          📅 {fristAnzeige(f.datum)}{#if f.titel} – {f.titel}{/if}
        </span>
        <button class="entfernen" title="Frist entfernen" onclick={() => fristEntfernen(i)}>✕</button>
      </li>
    {/each}
  </ul>
  <form class="hinzufuegen eigene-frist" onsubmit={fristHinzufuegen}>
    <input type="date" bind:value={neueFrist} />
    <input type="text" placeholder="Bezeichnung (z. B. Teamabgabe)" bind:value={neuerFristTitel} />
    <button type="submit" disabled={!neueFrist}>+ Frist</button>
  </form>

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
          {#if hochladen}
            <div class="datei-zeile">
              {#if punkt.datei}
                <span class="datei-name" title={punkt.datei}>📎 {punkt.datei}</span>
                <button class="datei-knopf" disabled={laedtIdx === i} onclick={() => dateiHochladen(i)}>
                  {laedtIdx === i ? "lädt …" : "ersetzen"}
                </button>
                <button class="datei-entfernen" title="Verknüpfung entfernen" onclick={() => dateiEntfernen(i)}>✕</button>
              {:else}
                <button class="datei-knopf hochladen" disabled={laedtIdx === i} onclick={() => dateiHochladen(i)}>
                  {laedtIdx === i ? "lädt …" : "⬆ Datei hochladen"}
                </button>
              {/if}
            </div>
          {/if}
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

  .kontakt label {
    display: block;
    font-size: 0.82rem;
    font-weight: 600;
    color: #5e6c84;
    margin: 10px 0 5px;
  }
  .kontakt input,
  .kontakt textarea {
    width: 100%;
    box-sizing: border-box;
    padding: 8px 10px;
    font-size: 0.9rem;
    font-family: inherit;
    border: 2px solid #dfe1e6;
    border-radius: 8px;
    background: #fafbfc;
  }
  .kontakt input:focus,
  .kontakt textarea:focus {
    outline: none;
    border-color: #4f6df5;
    background: #fff;
  }
  .kontakt textarea {
    resize: vertical;
    line-height: 1.5;
  }
  .kontakt .zwei {
    display: flex;
    gap: 12px;
  }
  .kontakt .zwei > div {
    flex: 1;
    min-width: 0;
  }

  .fristen {
    list-style: none;
    margin: 0 0 4px;
    padding: 0;
  }
  .fristen li {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
    padding: 6px 0;
    border-bottom: 1px solid #f1f2f4;
  }
  .frist-datum {
    font-size: 0.9rem;
  }
  /* kompaktes, editierbares Datum der offiziellen Frist */
  .off-fristen {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 8px;
  }
  .off-frist {
    display: inline-flex;
    align-items: center;
    gap: 2px;
  }
  .off-frist input[type="date"] {
    padding: 7px 9px;
    font-size: 0.9rem;
    font-family: inherit;
    border: 2px solid #dfe1e6;
    border-radius: 8px;
    background: #fafbfc;
  }
  .off-frist input[type="date"]:focus {
    outline: none;
    border-color: #4f6df5;
    background: #fff;
  }
  .off-add {
    background: none;
    border: none;
    color: #4f6df5;
    font-size: 0.85rem;
    font-family: inherit;
    cursor: pointer;
    padding: 4px 6px;
    border-radius: 6px;
  }
  .off-add:hover {
    background: #eef1ff;
  }
  .hinzufuegen input[type="date"] {
    flex: 1;
  }
  .eigene-frist input[type="date"] {
    flex: 0 0 auto;
  }
  .hinweis-klein {
    font-size: 0.8rem;
    color: #8590a2;
    margin: 4px 0 0;
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

  /* Hochgeladenes Dokument je Checklisten-Punkt */
  .datei-zeile {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-top: 8px;
    flex-wrap: wrap;
  }
  .datei-name {
    font-size: 0.82rem;
    color: #216e4e;
    background: #dcfff1;
    padding: 3px 10px;
    border-radius: 99px;
    max-width: 100%;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .datei-knopf {
    background: none;
    border: 2px solid #dfe1e6;
    color: #44546f;
    font-size: 0.82rem;
    font-family: inherit;
    cursor: pointer;
    padding: 4px 12px;
    border-radius: 8px;
  }
  .datei-knopf:hover:not(:disabled) {
    border-color: #4f6df5;
    color: #172b4d;
  }
  .datei-knopf:disabled {
    opacity: 0.6;
    cursor: default;
  }
  .datei-knopf.hochladen {
    color: #3d5bf0;
    border-color: #c7d0f8;
  }
  .datei-entfernen {
    background: none;
    border: none;
    color: #8590a2;
    font-size: 0.9rem;
    cursor: pointer;
    padding: 2px 6px;
    border-radius: 6px;
  }
  .datei-entfernen:hover {
    background: #ffeceb;
    color: #ae2e24;
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
