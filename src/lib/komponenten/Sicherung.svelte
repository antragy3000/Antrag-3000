<script>
  // Sicherung (Backup): erstellen und wiederherstellen. Zwei Arten:
  //  - „mit Dateien" (.a3ksicherung): Tresor(e) plus Dateien, als Ganzes
  //    verschlüsselt. Die Arbeit und alle Prüfungen macht Rust (backup.rs).
  //  - „nur Tresor" (.enc): Kopie der ohnehin verschlüsselten Tresor-Datei.
  //
  // Organisationen (O6): gesichert wird „diese“ (ihr Tresor, ihr eigener Ordner)
  // oder „alle“ auf dem Gerät. Beim Einspielen wählt man je Organisation:
  // ersetzen, als Kopie dazu oder weglassen. Eine vorhandene wird nie ohne
  // ausdrückliche Wahl ersetzt (die erste Organisation hat auf jedem Gerät
  // dieselbe ID – siehe einspielVorschlag).
  //
  // nurWiederherstellen: für den ersten Start auf einem neuen Computer – es gibt
  // noch keinen Tresor, also nichts zu sichern und nichts zu ersetzen.
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { save, open } from "@tauri-apps/plugin-dialog";
  import { einspielVorschlag } from "$lib/organisationen.js";

  // org: ID der aktiven Organisation; orgOrdner: ihr Ordner (leer, solange ihr
  // Umzug aussteht); name: ihr Name; eintraege: alle Organisationen auf dem
  // Gerät ({id, name}) – für „alle“ und die Namen beim Einspielen.
  let {
    schliessen,
    nachWiederherstellung,
    nurWiederherstellen = false,
    vorSichern = null,
    org = null,
    orgOrdner = "",
    name = "",
    eintraege = [],
  } = $props();

  let meldung = $state("");
  let fehler = $state("");
  let beschaeftigt = $state(false);
  let mitDateien = $state(true);
  let alle = $state(false);
  /** @type {string[]} */
  let probleme = $state([]);
  // Ohne Passwort angemeldet („Auf diesem Gerät merken")? Dann eigens daran
  // erinnern, dass die Sicherung trotzdem das Passwort braucht.
  let ueberMerken = $state(false);
  const mehrere = $derived(eintraege.length > 1);

  // Wiederherstellen: erst Datei wählen, dann (bei „mit Dateien“) mit dem
  // Passwort prüfen, je Organisation wählen und bestätigen.
  let gewaehlteDatei = $state("");
  let art = $state("");
  let passwort = $state("");
  let bestaetigen = $state(false);
  let vorschau = $state(/** @type {null | { version: number, dateien: number }} */ (null));
  /** @type {{ id: string, name: string | null, vorhanden: boolean, lokalName: string | null, wahl: string, andere: boolean }[]} */
  let auswahl = $state([]);

  onMount(async () => {
    if (nurWiederherstellen) return;
    try {
      ueberMerken = !!(await invoke("tresor_ueber_merken", { org }));
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

  /** @param {number} n */
  function orgsText(n) {
    return n === 1 ? "1 Organisation" : `${n} Organisationen`;
  }

  async function erstellen() {
    fehler = "";
    meldung = "";
    probleme = [];
    const heute = new Date().toISOString().slice(0, 10);
    const alleWahl = mehrere && alle;
    // Ohne Dateien und nur eine Organisation: schlicht ihre Tresor-Datei.
    const nurTresor = !mitDateien && !alleWahl;
    const endung = nurTresor ? "enc" : "a3ksicherung";
    // Bewusst ohne Organisationsnamen im Dateinamen: Der läge sonst lesbar
    // auf dem USB-Stick oder im Cloud-Ordner.
    const ziel = await save({
      title: "Sicherung speichern",
      defaultPath: `antrag3000-sicherung${alleWahl ? "-alle" : ""}-${heute}.${endung}`,
      filters: [{ name: "Antrag 3000 Sicherung", extensions: [endung] }],
    });
    if (!ziel) return; // abgebrochen
    beschaeftigt = true;
    try {
      // Den neuesten Stand zuerst in den Tresor schreiben.
      if (vorSichern) await vorSichern();
      if (nurTresor) {
        await invoke("tresor_backup_erstellen", { org, ziel });
        meldung = "Sicherung gespeichert:\n" + ziel;
      } else {
        const b = await invoke("sicherung_erstellen", { org, ziel, alle: alleWahl, mitDateien, orgOrdner });
        const was = alleWahl ? orgsText(b.organisationen) : "dein Tresor";
        meldung = mitDateien
          ? `Sicherung gespeichert und geprüft: ${was} und ${dateienText(b.dateien)} (${groesseText(b.bytes)}).\n${ziel}`
          : `Sicherung gespeichert und geprüft: ${was}.\n${ziel}`;
        probleme = b.probleme;
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
    if (art === "organisation") {
      art = "";
      fehler = "Das ist eine weitergegebene Organisation, keine Sicherung. Du fügst sie über das Organisations-Menü hinzu: „Organisation aus Datei hinzufügen“.";
      return;
    }
    gewaehlteDatei = pfad;
    passwort = "";
    vorschau = null;
    auswahl = [];
    bestaetigen = true;
  }

  function abbrechen() {
    fehler = "";
    passwort = "";
    vorschau = null;
    auswahl = [];
    bestaetigen = false;
  }

  /** @param {unknown} e */
  function fehlerText(e) {
    return String(e) === "falsches_passwort"
      ? "Dieses Passwort passt nicht zur Sicherung. Gemeint ist das Passwort, das galt, als du sie erstellt hast."
      : String(e);
  }

  // Sicherung mit Dateien: erst komplett prüfen (schreibt nichts) und zeigen,
  // welche Organisationen darin sind.
  async function pruefen() {
    fehler = "";
    beschaeftigt = true;
    try {
      const v = await invoke("sicherung_pruefen", { quelle: gewaehlteDatei, passwort });
      vorschau = { version: v.version, dateien: v.dateien };
      auswahl = einspielVorschlag(v.orgs, eintraege);
    } catch (e) {
      fehler = fehlerText(e);
    } finally {
      beschaeftigt = false;
    }
  }

  // Was nach dem Einspielen auf dem Entsperren-Bildschirm stehen soll.
  /** @param {{ dateien: number, vorhanden: number, probleme: string[], organisationen?: number }} b */
  function berichtText(b) {
    const teile = [];
    if (b.organisationen && b.organisationen > 1) teile.push(`${orgsText(b.organisationen)} eingespielt.`);
    teile.push(`Sicherung eingespielt: ${dateienText(b.dateien)} wiederhergestellt.`);
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
        /** @type {Record<string, string> | null} */
        const wahl = vorschau?.version === 1 ? null : Object.fromEntries(auswahl.map((o) => [o.id, o.wahl]));
        const b = await invoke("sicherung_einspielen", { org, quelle: gewaehlteDatei, passwort, wahl });
        hinweis = berichtText(b);
      } else {
        await invoke("tresor_backup_einspielen", { org, quelle: gewaehlteDatei });
      }
      passwort = "";
      bestaetigen = false;
      schliessen();
      nachWiederherstellung(hinweis);
    } catch (e) {
      fehler = fehlerText(e);
    } finally {
      beschaeftigt = false;
    }
  }

  // Version 2: mindestens eine Organisation muss eingespielt werden.
  const etwasGewaehlt = $derived(auswahl.some((o) => o.wahl !== "weglassen"));
  const ersetztEtwas = $derived(auswahl.some((o) => o.wahl === "ersetzen"));
  const ersetztNamen = $derived(
    auswahl
      .filter((o) => o.wahl === "ersetzen")
      .map((o) => `„${o.lokalName ?? o.name ?? "Organisation mit eigenem Passwort"}“`)
      .join(", "),
  );
  let kannEinspielen = $derived(
    art !== "komplett" || (vorschau !== null && (vorschau.version === 1 || etwasGewaehlt)),
  );
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
          {#if mehrere}
            <fieldset class="umfang" disabled={beschaeftigt}>
              <legend>Was sichern?</legend>
              <label class="haken">
                <input type="radio" name="umfang" value={false} bind:group={alle} />
                <span>Nur „{name}“</span>
              </label>
              <label class="haken">
                <input type="radio" name="umfang" value={true} bind:group={alle} />
                <span>
                  Alle {eintraege.length} Organisationen auf diesem Gerät
                  <small>
                    Auch gesperrte und solche mit eigenem Passwort – jede bleibt mit ihrem
                    eigenen Passwort verschlüsselt. Die Sicherung selbst öffnest du mit dem
                    Passwort von „{name}“.
                  </small>
                </span>
              </label>
            </fieldset>
          {/if}
          <label class="haken">
            <input type="checkbox" bind:checked={mitDateien} disabled={beschaeftigt} />
            <span>
              Auch die Dateien sichern (empfohlen)
              <small>
                Belege, hochgeladene Dokumente, Antrags-PDFs und Word-/Excel-Dateien
                {#if mehrere && alle}
                  aus <code>Dokumente\Antrag 3000</code>.
                {:else if orgOrdner}
                  aus <code>Dokumente\Antrag 3000\{orgOrdner}</code>.
                {:else}
                  aus <code>Dokumente\Antrag 3000</code>.
                {/if}
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
            Spielt eine Sicherung ein. Enthält sie Organisationen, die es hier schon gibt,
            wählst du für jede: ersetzen, als Kopie dazunehmen oder weglassen. Was ersetzt
            wird, legt die App vorher beiseite (nicht gelöscht). Danach meldest du dich mit
            dem Passwort der Sicherung neu an.
          </p>
          <button class="zweit" disabled={beschaeftigt} onclick={dateiWaehlen}>
            Sicherungsdatei auswählen …
          </button>
        </section>

        <section>
          <details>
            <summary>Auf einen neuen Computer umziehen</summary>
            <ol>
              <li>Hier mit <strong>„Auch die Dateien sichern"</strong> eine Sicherung speichern,
                am besten auf einen USB-Stick. Hast du mehrere Organisationen, wähle
                <strong>„Alle Organisationen“</strong>.</li>
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
        <h3>Wiederherstellen</h3>
        <p class="pfad"><code>{gewaehlteDatei}</code></p>
        {#if art === "komplett" && vorschau === null}
          <p>
            Die Sicherung wird zuerst komplett geprüft – dabei wird noch nichts verändert.
          </p>
          <label class="feld" for="sicherung-pw">Passwort dieser Sicherung</label>
          <input
            id="sicherung-pw"
            type="password"
            autocomplete="current-password"
            bind:value={passwort}
            disabled={beschaeftigt}
            onkeydown={(e) => { if (e.key === "Enter" && passwort) pruefen(); }}
          />
          {#if fehler}<p class="fehler">⚠ {fehler}</p>{/if}
          <div class="fuss">
            <button class="leise" disabled={beschaeftigt} onclick={abbrechen}>Abbrechen</button>
            <button class="primaer" disabled={beschaeftigt || !passwort} onclick={pruefen}>
              {beschaeftigt ? "Wird geprüft …" : "Sicherung prüfen"}
            </button>
          </div>
        {:else}
          {#if art === "komplett" && vorschau && vorschau.version !== 1}
            <p>
              Geprüft: {orgsText(auswahl.length)} und {dateienText(vorschau.dateien)}.
              Was soll mit jeder geschehen?
            </p>
            <ul class="orgs">
              {#each auswahl as o (o.id)}
                <li>
                  <div class="org-name">
                    {o.name ?? "Organisation mit eigenem Passwort"}
                    {#if o.vorhanden}
                      <small>
                        Gibt es hier schon{o.lokalName ? ` als „${o.lokalName}“` : ""}.
                        {#if o.andere}
                          Achtung: vermutlich eine andere Organisation – die erste
                          Organisation hat auf jedem Gerät dieselbe Kennung.
                        {:else if o.name && o.lokalName && o.name !== o.lokalName}
                          Seit der Sicherung umbenannt.
                        {/if}
                      </small>
                    {/if}
                  </div>
                  <select bind:value={o.wahl} disabled={beschaeftigt} aria-label="Was mit {o.name ?? 'dieser Organisation'} geschehen soll">
                    {#if o.vorhanden}
                      <option value="ersetzen">ersetzen</option>
                      <option value="kopie">als Kopie dazu</option>
                    {:else}
                      <option value="dazu">einspielen</option>
                    {/if}
                    <option value="weglassen">weglassen</option>
                  </select>
                </li>
              {/each}
            </ul>
            {#if ersetztEtwas}
              <p class="warn">
                Ersetzt wird der aktuelle Stand von {ersetztNamen} auf diesem Gerät (er wird
                beiseitegelegt, nicht gelöscht). Dateien, die es hier schon gibt, werden
                <strong>nicht</strong> überschrieben.
              </p>
            {/if}
            {#if auswahl.some((o) => !o.name && o.wahl !== "weglassen")}
              <p class="klein">
                Organisationen mit eigenem Passwort öffnest du danach wie gewohnt mit ihrem
                eigenen Passwort.
              </p>
            {/if}
            {#if !etwasGewaehlt}<p class="fehler">Wähle mindestens eine Organisation.</p>{/if}
          {:else}
            {#if !nurWiederherstellen}
              <p class="warn">
                {art === "komplett" ? "Diese Sicherung stammt aus einer älteren Version und" : "Diese Sicherung"}
                ersetzt den aktuellen Stand von „{name}“. Du wirst danach abgemeldet und
                entsperrst mit dem zur Sicherung gehörenden Passwort. Fortfahren?
              </p>
            {/if}
            {#if art === "komplett"}
              <p>Dateien, die es hier schon gibt, werden <strong>nicht</strong> überschrieben.</p>
            {:else}
              <p class="warn">
                Diese Sicherung enthält nur den Tresor, <strong>keine Dateien</strong>
                (Belege, Dokumente).
              </p>
            {/if}
          {/if}
          {#if fehler}<p class="fehler">⚠ {fehler}</p>{/if}
          <div class="fuss">
            <button class="leise" disabled={beschaeftigt} onclick={abbrechen}>Abbrechen</button>
            <button class="gefahr" disabled={beschaeftigt || !kannEinspielen} onclick={wiederherstellen}>
              {beschaeftigt ? "Wird eingespielt …" : "Ja, einspielen"}
            </button>
          </div>
        {/if}
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
  .umfang {
    border: none;
    margin: 0 0 4px;
    padding: 0;
  }
  .umfang legend {
    padding: 0;
    margin: 0 0 6px;
    font-size: 0.85rem;
    font-weight: 600;
    color: var(--text-2);
  }
  .orgs {
    list-style: none;
    margin: 0 0 12px;
    padding: 0;
  }
  .orgs li {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    padding: 8px 0;
    border-top: 1px solid var(--flaeche-2b);
  }
  .org-name {
    font-size: 0.92rem;
    font-weight: 600;
    color: var(--text);
  }
  .org-name small {
    display: block;
    font-weight: 400;
    font-size: 0.82rem;
    color: var(--text-2);
  }
  .orgs select {
    flex: none;
    padding: 6px 8px;
    font-size: 0.88rem;
    font-family: inherit;
    border: 1px solid var(--rand);
    border-radius: 8px;
    background: var(--weiss);
    color: var(--text);
  }
</style>
