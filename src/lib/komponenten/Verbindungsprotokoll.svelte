<script>
  // Verbindungsprotokoll (Paket 6): zeigt jede Verbindung der App ins Internet
  // seit dem Start – wann, wozu, zu welchem Server, was mitgeschickt wurde, ob
  // von selbst oder per Klick, und ob sie geklappt hat. Die Daten führt
  // netzProtokoll.svelte.js; hier wird nur angezeigt.
  import { verbindungen, sitzungsStart } from "$lib/netzProtokoll.svelte.js";

  let { schliessen } = $props();

  /** @param {string} iso */
  function uhrzeit(iso) {
    return new Date(iso).toLocaleTimeString("de-DE", { hour: "2-digit", minute: "2-digit", second: "2-digit" });
  }

  let gesamt = $derived(verbindungen.reduce((s, v) => s + v.anzahl, 0));
</script>

<div class="schleier" onclick={schliessen} role="presentation">
  <div class="dialog" onclick={(e) => e.stopPropagation()} role="presentation">
    <h2>Verbindungsprotokoll</h2>
    <p class="klein">
      Alle Verbindungen der App ins Internet seit dem Start um <strong>{uhrzeit(sitzungsStart)}</strong>.
      Inhalte deiner Daten stehen hier nie – nur, wozu die Verbindung war. Die Liste gilt nur für
      diese Sitzung und wird nicht gespeichert.
    </p>

    {#if verbindungen.length === 0}
      <p class="leer">Seit dem Start hat sich die App mit nichts verbunden.</p>
    {:else}
      <p class="zahl">{gesamt} {gesamt === 1 ? "Verbindung" : "Verbindungen"}, neueste oben</p>
      <ul>
        {#each verbindungen as v (v.schluessel)}
          <li class:fehl={!v.ok}>
            <div class="kopf">
              <span class="zeit">
                {#if v.anzahl > 1}
                  {uhrzeit(v.von)}–{uhrzeit(v.bis)} · {v.anzahl}×
                {:else}
                  {uhrzeit(v.von)}
                {/if}
              </span>
              <span class="zweck">{v.zweck}</span>
              <span class="ergebnis">{v.ok ? "✓" : "✗ fehlgeschlagen"}</span>
            </div>
            <div class="details">
              <span>Server: <code>{v.ziel}</code></span>
              <span>Mitgeschickt: {v.mit}</span>
              <span class="ausloeser" class:auto={v.automatisch}>{v.automatisch ? "automatisch" : "von dir ausgelöst"}</span>
            </div>
          </li>
        {/each}
      </ul>
    {/if}

    <p class="klein fuss">
      Nicht hier aufgeführt: Links, die du im Browser öffnest (z. B. Förderer-Webseiten oder
      CryptPad). Die lädt dein Browser, nicht Antrag 3000.
    </p>
    <div class="knoepfe"><button class="primaer" onclick={schliessen}>Schließen</button></div>
  </div>
</div>

<style>
  .schleier {
    position: fixed;
    inset: 0;
    background: var(--schatten-xl);
    display: grid;
    place-items: center;
    padding: 24px;
    z-index: 30;
  }
  .dialog {
    background: var(--weiss);
    border-radius: 12px;
    padding: 28px 32px;
    max-width: 680px;
    width: 100%;
    max-height: 88vh;
    overflow-y: auto;
    box-shadow: 0 12px 40px var(--schatten-lg);
  }
  h2 {
    margin: 0 0 10px;
    font-size: 1.2rem;
  }
  p {
    margin: 0 0 10px;
    line-height: 1.5;
  }
  .klein {
    color: var(--text-muted);
    font-size: 0.84rem;
  }
  .leer {
    margin: 18px 0;
    padding: 14px 16px;
    border-radius: 8px;
    background: var(--erfolg-bg);
    color: var(--erfolg-text);
    font-weight: 600;
    font-size: 0.92rem;
  }
  .zahl {
    font-size: 0.85rem;
    font-weight: 600;
    color: var(--text-2);
    margin-top: 14px;
  }
  ul {
    list-style: none;
    margin: 0 0 12px;
    padding: 0;
  }
  li {
    border: 1px solid var(--rand);
    border-radius: 8px;
    padding: 8px 12px;
    margin-bottom: 6px;
    font-size: 0.86rem;
  }
  li.fehl {
    border-color: var(--gefahr-text);
  }
  .kopf {
    display: flex;
    gap: 10px;
    align-items: baseline;
  }
  .zeit {
    font-variant-numeric: tabular-nums;
    color: var(--text-muted);
    white-space: nowrap;
  }
  .zweck {
    flex: 1;
    font-weight: 600;
  }
  .ergebnis {
    white-space: nowrap;
    color: var(--erfolg-text);
  }
  li.fehl .ergebnis {
    color: var(--gefahr-text);
  }
  .details {
    display: flex;
    flex-wrap: wrap;
    gap: 4px 14px;
    margin-top: 4px;
    color: var(--text-muted);
    font-size: 0.8rem;
  }
  code {
    font-size: 0.78rem;
  }
  .ausloeser {
    padding: 0 6px;
    border-radius: 4px;
    background: var(--flaeche);
  }
  .ausloeser.auto {
    background: var(--akzent-bg);
    color: var(--text-2);
  }
  .fuss {
    margin-top: 12px;
  }
  .knoepfe {
    display: flex;
    justify-content: flex-end;
    margin-top: 14px;
  }
  .primaer {
    padding: 10px 22px;
    font-size: 0.95rem;
    font-weight: 600;
    font-family: inherit;
    color: var(--auf-farbe);
    background: var(--akzent);
    border: none;
    border-radius: 8px;
    cursor: pointer;
  }
  .primaer:hover {
    background: var(--akzent-d);
  }
</style>
