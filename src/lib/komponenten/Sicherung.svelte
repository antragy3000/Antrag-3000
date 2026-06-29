<script>
  // Sicherung (Backup) des Tresors: erstellen und wiederherstellen.
  // Die Sicherung ist eine Kopie der bereits verschlüsselten
  // Tresor-Datei – sie ist überall sicher und nur mit dem Passwort
  // lesbar, das zu ihr gehört.
  import { invoke } from "@tauri-apps/api/core";
  import { save, open } from "@tauri-apps/plugin-dialog";

  let { schliessen, nachWiederherstellung } = $props();

  let meldung = $state("");
  let fehler = $state("");
  let beschaeftigt = $state(false);

  // Wiederherstellen: erst Datei wählen, dann bestätigen (destruktiv).
  let gewaehlteDatei = $state("");
  let bestaetigen = $state(false);

  async function erstellen() {
    fehler = "";
    meldung = "";
    const heute = new Date().toISOString().slice(0, 10);
    const ziel = await save({
      title: "Tresor-Sicherung speichern",
      defaultPath: `antrag3000-sicherung-${heute}.enc`,
      filters: [{ name: "Antrag 3000 Sicherung", extensions: ["enc"] }],
    });
    if (!ziel) return; // abgebrochen
    beschaeftigt = true;
    try {
      await invoke("tresor_backup_erstellen", { ziel });
      meldung = "Sicherung gespeichert:\n" + ziel;
    } catch (e) {
      fehler = String(e);
    } finally {
      beschaeftigt = false;
    }
  }

  async function dateiWaehlen() {
    fehler = "";
    meldung = "";
    const pfad = await open({
      title: "Sicherung auswählen",
      multiple: false,
      filters: [{ name: "Antrag 3000 Sicherung", extensions: ["enc"] }],
    });
    if (!pfad) return;
    gewaehlteDatei = pfad;
    bestaetigen = true;
  }

  async function wiederherstellen() {
    beschaeftigt = true;
    try {
      await invoke("tresor_backup_einspielen", { quelle: gewaehlteDatei });
      bestaetigen = false;
      schliessen();
      nachWiederherstellung();
    } catch (e) {
      fehler = String(e);
      bestaetigen = false;
    } finally {
      beschaeftigt = false;
    }
  }
</script>

<div class="schleier" onclick={schliessen} role="presentation">
  <div class="dialog" onclick={(e) => e.stopPropagation()} role="presentation">
    <h2>Sicherung</h2>

    {#if !bestaetigen}
      <section>
        <h3>Sicherung erstellen</h3>
        <p>
          Speichert eine verschlüsselte Kopie deines Tresors an einen Ort
          deiner Wahl (z. B. USB-Stick oder Cloud-Ordner). Sie ist nur mit
          deinem Passwort lesbar.
        </p>
        <button class="primaer" disabled={beschaeftigt} onclick={erstellen}>
          Sicherung speichern …
        </button>
      </section>

      <section>
        <h3>Sicherung wiederherstellen</h3>
        <p class="warn">
          Ersetzt deinen aktuellen Tresor durch eine Sicherung. Danach musst du
          mit dem Passwort entsperren, das zu dieser Sicherung gehört. Der
          bisherige Tresor wird vorher beiseitegelegt (nicht gelöscht).
        </p>
        <button class="zweit" disabled={beschaeftigt} onclick={dateiWaehlen}>
          Sicherungsdatei auswählen …
        </button>
      </section>

      {#if meldung}<p class="ok">✓ {meldung}</p>{/if}
      {#if fehler}<p class="fehler">⚠ {fehler}</p>{/if}

      <div class="fuss">
        <button class="leise" onclick={schliessen}>Schließen</button>
      </div>
    {:else}
      <section>
        <h3>Wiederherstellen bestätigen</h3>
        <p class="pfad"><code>{gewaehlteDatei}</code></p>
        <p class="warn">
          Dein aktueller Tresor wird durch diese Sicherung ersetzt. Du wirst
          danach abgemeldet und entsperrst mit dem zur Sicherung gehörenden
          Passwort. Fortfahren?
        </p>
        {#if fehler}<p class="fehler">⚠ {fehler}</p>{/if}
        <div class="fuss">
          <button class="leise" onclick={() => (bestaetigen = false)}>Abbrechen</button>
          <button class="gefahr" disabled={beschaeftigt} onclick={wiederherstellen}>
            {beschaeftigt ? "Wird eingespielt …" : "Ja, wiederherstellen"}
          </button>
        </div>
      </section>
    {/if}
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
    z-index: 40;
  }
  .dialog {
    background: var(--weiss);
    border-radius: 12px;
    padding: 32px;
    max-width: 480px;
    width: 100%;
    max-height: 85vh;
    overflow-y: auto;
    box-shadow: 0 12px 40px var(--schatten-lg);
  }
  h2 {
    margin: 0 0 16px;
    font-size: 1.2rem;
  }
  h3 {
    margin: 0 0 6px;
    font-size: 1rem;
    font-weight: 600;
  }
  section {
    padding: 16px 0;
    border-top: 1px solid var(--flaeche-2b);
  }
  section:first-of-type {
    border-top: none;
    padding-top: 0;
  }
  p {
    margin: 0 0 12px;
    font-size: 0.92rem;
    line-height: 1.55;
    color: var(--text-2);
  }
  .warn {
    background: var(--warnung-bg);
    color: var(--warnung-text2);
    border-radius: 8px;
    padding: 10px 14px;
  }
  .pfad code {
    font-size: 0.85rem;
    word-break: break-all;
  }
  .ok {
    color: var(--erfolg-text);
    font-weight: 600;
    white-space: pre-wrap;
    word-break: break-all;
  }
  .fehler {
    color: var(--gefahr-text);
    font-weight: 600;
  }

  button {
    padding: 10px 18px;
    font-size: 0.93rem;
    font-weight: 600;
    font-family: inherit;
    border-radius: 8px;
    cursor: pointer;
    border: none;
  }
  .primaer {
    color: var(--weiss);
    background: var(--akzent);
  }
  .primaer:hover:not(:disabled) {
    background: var(--akzent-d);
  }
  .zweit {
    color: var(--text);
    background: var(--weiss);
    border: 2px solid var(--rand);
  }
  .zweit:hover:not(:disabled) {
    border-color: var(--akzent);
  }
  .gefahr {
    color: var(--weiss);
    background: var(--gefahr);
  }
  .gefahr:hover:not(:disabled) {
    background: var(--gefahr-text);
  }
  button:disabled {
    opacity: 0.6;
    cursor: default;
  }
  .leise {
    background: none;
    color: var(--text-muted);
    font-weight: 400;
  }
  .leise:hover {
    color: var(--text);
    text-decoration: underline;
  }
  .fuss {
    display: flex;
    justify-content: flex-end;
    gap: 10px;
    margin-top: 8px;
  }
</style>
