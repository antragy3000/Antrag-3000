<script>
  // Sicherung (Backup): erstellen und wiederherstellen. Zwei Arten:
  //  - „mit Dateien" (.a3ksicherung): Tresor plus alles aus Dokumente\Antrag 3000,
  //    als Ganzes verschlüsselt. Die Arbeit und alle Prüfungen macht Rust (backup.rs).
  //  - „nur Tresor" (.enc): Kopie der ohnehin verschlüsselten Tresor-Datei.
  // Beide sind überall sicher und nur mit dem Passwort lesbar, das beim
  // Erstellen galt.
  //
  // nurWiederherstellen: für den ersten Start auf einem neuen Computer – es gibt
  // noch keinen Tresor, also nichts zu sichern und nichts zu ersetzen.
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { save, open } from "@tauri-apps/plugin-dialog";

  let {
    schliessen,
    nachWiederherstellung,
    nurWiederherstellen = false,
    vorSichern = null,
  } = $props();

  let meldung = $state("");
  let fehler = $state("");
  let beschaeftigt = $state(false);
  let mitDateien = $state(true);
  /** @type {string[]} */
  let probleme = $state([]);
  // Ohne Passwort angemeldet („Auf diesem Gerät merken")? Dann eigens daran
  // erinnern, dass die Sicherung trotzdem das Passwort braucht.
  let ueberMerken = $state(false);

  // Wiederherstellen: erst Datei wählen, dann bestätigen (destruktiv).
  let gewaehlteDatei = $state("");
  let art = $state("");
  let passwort = $state("");
  let bestaetigen = $state(false);

  onMount(async () => {
    if (nurWiederherstellen) return;
    try {
      ueberMerken = !!(await invoke("tresor_ueber_merken"));
    } catch {
      ueberMerken = false;
    }
  });

  /** @param {number} bytes */
  function groesseText(bytes) {
    if (bytes < 1024 * 1024) return `${Math.max(1, Math.round(bytes / 1024))} KB`;
    return `${(bytes / (1024 * 1024)).toLocaleString("de-DE", { maximumFractionDigits: 1 })} MB`;
  }

  /** @param {number} n */
  function dateienText(n) {
    return n === 1 ? "1 Datei" : `${n} Dateien`;
  }

  async function erstellen() {
    fehler = "";
    meldung = "";
    probleme = [];
    const heute = new Date().toISOString().slice(0, 10);
    const endung = mitDateien ? "a3ksicherung" : "enc";
    const ziel = await save({
      title: "Sicherung speichern",
      defaultPath: `antrag3000-sicherung-${heute}.${endung}`,
      filters: [{ name: "Antrag 3000 Sicherung", extensions: [endung] }],
    });
    if (!ziel) return; // abgebrochen
    beschaeftigt = true;
    try {
      // Den neuesten Stand zuerst in den Tresor schreiben.
      if (vorSichern) await vorSichern();
      if (mitDateien) {
        const b = await invoke("sicherung_erstellen", { ziel });
        meldung = `Sicherung gespeichert und geprüft: dein Tresor und ${dateienText(b.dateien)} (${groesseText(b.bytes)}).\n${ziel}`;
        probleme = b.probleme;
      } else {
        await invoke("tresor_backup_erstellen", { ziel });
        meldung = "Sicherung gespeichert:\n" + ziel;
      }
    } catch (e) {
      fehler = String(e);
    } finally {
      beschaeftigt = false;
    }
  }

  async function dateiWaehlen() {
    fehler = "";
    meldung = "";
    probleme = [];
    const pfad = await open({
      title: "Sicherung auswählen",
      multiple: false,
      filters: [{ name: "Antrag 3000 Sicherung", extensions: ["a3ksicherung", "enc"] }],
    });
    if (!pfad) return;
    try {
      art = await invoke("sicherung_art", { quelle: pfad });
    } catch (e) {
      fehler = String(e);
      return;
    }
    gewaehlteDatei = pfad;
    passwort = "";
    bestaetigen = true;
  }

  // Was nach dem Einspielen auf dem Entsperren-Bildschirm stehen soll.
  /** @param {{ dateien: number, vorhanden: number, probleme: string[] }} b */
  function berichtText(b) {
    const teile = [`Sicherung eingespielt: ${dateienText(b.dateien)} wiederhergestellt.`];
    if (b.vorhanden > 0) {
      teile.push(`${dateienText(b.vorhanden)} gab es schon und wurden nicht überschrieben.`);
    }
    if (b.probleme.length > 0) {
      const liste = b.probleme.slice(0, 10).join(", ");
      const rest = b.probleme.length > 10 ? ` und ${b.probleme.length - 10} weitere` : "";
      teile.push(`Nicht wiederhergestellt: ${liste}${rest}.`);
    }
    return teile.join(" ");
  }

  async function wiederherstellen() {
    fehler = "";
    beschaeftigt = true;
    try {
      let hinweis = "Sicherung eingespielt.";
      if (art === "komplett") {
        const b = await invoke("sicherung_einspielen", { quelle: gewaehlteDatei, passwort });
        hinweis = berichtText(b);
      } else {
        await invoke("tresor_backup_einspielen", { quelle: gewaehlteDatei });
      }
      passwort = "";
      bestaetigen = false;
      schliessen();
      nachWiederherstellung(hinweis);
    } catch (e) {
      fehler =
        String(e) === "falsches_passwort"
          ? "Dieses Passwort passt nicht zur Sicherung. Gemeint ist das Passwort, das galt, als du sie erstellt hast."
          : String(e);
    } finally {
      beschaeftigt = false;
    }
  }

  // Eingabe nur bei der Sicherung mit Dateien: Sie wird vor dem Einspielen
  // komplett geprüft, und dafür braucht es das Passwort.
  let kannEinspielen = $derived(art !== "komplett" || passwort.length > 0);
</script>

<div class="schleier" onclick={schliessen} role="presentation">
  <div class="dialog" onclick={(e) => e.stopPropagation()} role="presentation">
    <h2>{nurWiederherstellen ? "Sicherung einspielen" : "Sicherung"}</h2>

    {#if !bestaetigen}
      {#if !nurWiederherstellen}
        <section>
          <h3>Sicherung erstellen</h3>
          <p>
            Speichert eine verschlüsselte Sicherung an einem Ort deiner Wahl
            (z. B. USB-Stick oder Cloud-Ordner). Sie ist nur mit deinem Passwort
            lesbar – auch für den Cloud-Anbieter oder wer den USB-Stick findet.
          </p>
          <label class="haken">
            <input type="checkbox" bind:checked={mitDateien} disabled={beschaeftigt} />
            <span>
              Auch meine Dateien sichern (empfohlen)
              <small>
                Belege, hochgeladene Dokumente, Antrags-PDFs und Word-/Excel-Dateien
                aus <code>Dokumente\Antrag 3000</code>.
              </small>
            </span>
          </label>
          {#if !mitDateien}
            <p class="warn">
              <strong>Ohne Dateien</strong> fehlen in der Sicherung deine Belege und Dokumente –
              besonders die Belege musst du für den Verwendungsnachweis aufbewahren.
              Sichere den Ordner <code>Dokumente\Antrag 3000</code> dann anders und bedenke:
              Dort liegen die Dateien <strong>unverschlüsselt</strong>.
            </p>
          {/if}
          {#if ueberMerken}
            <p class="warn">
              Du bist gerade ohne Passwort angemeldet („Auf diesem Gerät merken"). Zum
              Wiederherstellen brauchst du aber dein <strong>Passwort</strong>. Weißt du es nicht
              mehr, lege zuerst ein neues fest (Stammdaten &amp; Team → Passwort ändern).
            </p>
          {/if}
          <button class="primaer" disabled={beschaeftigt} onclick={erstellen}>
            {beschaeftigt ? "Wird gesichert und geprüft …" : "Sicherung speichern …"}
          </button>
          {#if beschaeftigt && mitDateien}
            <p class="klein">Bei vielen Dateien kann das ein paar Minuten dauern.</p>
          {/if}
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

        <section>
          <details>
            <summary>Auf einen neuen Computer umziehen</summary>
            <ol>
              <li>Hier mit <strong>„Auch meine Dateien sichern"</strong> eine Sicherung speichern,
                am besten auf einen USB-Stick.</li>
              <li>Auf dem neuen Computer Antrag 3000 installieren und starten.</li>
              <li>Beim Willkommens-Bildschirm <strong>kein neues Passwort</strong> festlegen,
                sondern auf <strong>„Ich habe eine Sicherung"</strong> klicken.</li>
              <li>Die Sicherungsdatei auswählen und dein Passwort eingeben.</li>
              <li>Mit demselben Passwort entsperren – fertig. „Auf diesem Gerät merken" musst
                du auf dem neuen Computer neu ankreuzen.</li>
            </ol>
            <p class="klein">
              Dein Tresor enthält alles Übrige: Projekte, Stammdaten, Merklisten und einen
              Team-Zugang. Prüfe auf dem neuen Computer kurz, ob alles da ist, bevor du den
              alten abgibst.
            </p>
          </details>
        </section>
      {:else}
        <section>
          <p>
            Wähle die Sicherungsdatei von deinem alten Computer aus (endet auf
            <code>.a3ksicherung</code> oder <code>.enc</code>). Du brauchst das Passwort, das
            beim Erstellen der Sicherung galt.
          </p>
          <button class="primaer" disabled={beschaeftigt} onclick={dateiWaehlen}>
            Sicherungsdatei auswählen …
          </button>
        </section>
      {/if}

      {#if meldung}<p class="ok">✓ {meldung}</p>{/if}
      {#if probleme.length > 0}
        <p class="warn">
          <strong>Nicht in der Sicherung</strong> (nicht lesbar oder Verknüpfung):
          {probleme.slice(0, 10).join(", ")}{probleme.length > 10 ? ` und ${probleme.length - 10} weitere` : ""}
        </p>
      {/if}
      {#if fehler}<p class="fehler">⚠ {fehler}</p>{/if}

      <div class="fuss">
        <button class="leise" disabled={beschaeftigt} onclick={schliessen}>Schließen</button>
      </div>
    {:else}
      <section>
        <h3>Wiederherstellen bestätigen</h3>
        <p class="pfad"><code>{gewaehlteDatei}</code></p>
        {#if !nurWiederherstellen}
          <p class="warn">
            Dein aktueller Tresor wird durch diese Sicherung ersetzt. Du wirst
            danach abgemeldet und entsperrst mit dem zur Sicherung gehörenden
            Passwort. Fortfahren?
          </p>
        {/if}
        {#if art === "komplett"}
          <p>
            Die Sicherung wird zuerst komplett geprüft. Dateien, die es hier schon gibt,
            werden <strong>nicht</strong> überschrieben.
          </p>
          <label class="feld" for="sicherung-pw">Passwort dieser Sicherung</label>
          <input
            id="sicherung-pw"
            type="password"
            autocomplete="current-password"
            bind:value={passwort}
            disabled={beschaeftigt}
          />
        {:else}
          <p class="warn">
            Diese Sicherung enthält nur den Tresor, <strong>keine Dateien</strong>
            (Belege, Dokumente).
          </p>
        {/if}
        {#if fehler}<p class="fehler">⚠ {fehler}</p>{/if}
        <div class="fuss">
          <button class="leise" disabled={beschaeftigt} onclick={() => { fehler = ""; passwort = ""; bestaetigen = false; }}>Abbrechen</button>
          <button class="gefahr" disabled={beschaeftigt || !kannEinspielen} onclick={wiederherstellen}>
            {beschaeftigt ? "Wird geprüft und eingespielt …" : "Ja, wiederherstellen"}
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
    text-align: left;
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
  .klein {
    margin: 10px 0 0;
    font-size: 0.85rem;
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
    word-break: break-word;
  }
  .fehler {
    color: var(--gefahr-text);
    font-weight: 600;
  }

  .haken {
    display: flex;
    align-items: flex-start;
    gap: 8px;
    margin: 0 0 12px;
    font-size: 0.92rem;
    font-weight: 600;
    color: var(--text);
    cursor: pointer;
  }
  .haken input {
    margin-top: 3px;
    flex: none;
  }
  .haken small {
    display: block;
    margin-top: 2px;
    font-size: 0.85rem;
    font-weight: 400;
    color: var(--text-2);
    line-height: 1.45;
  }
  .feld {
    display: block;
    margin: 10px 0 4px;
    font-size: 0.85rem;
    font-weight: 600;
    color: var(--text-2);
  }
  input[type="password"] {
    width: 100%;
    box-sizing: border-box;
    margin-bottom: 8px;
    padding: 9px 11px;
    font-size: 0.95rem;
    font-family: inherit;
    border: 1px solid var(--rand);
    border-radius: 8px;
  }

  summary {
    cursor: pointer;
    font-size: 0.95rem;
    font-weight: 600;
    color: var(--text);
  }
  ol {
    margin: 10px 0;
    padding-left: 20px;
    font-size: 0.9rem;
    line-height: 1.55;
    color: var(--text-2);
  }
  li {
    margin-bottom: 6px;
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
    color: var(--auf-farbe);
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
    color: var(--auf-farbe);
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
