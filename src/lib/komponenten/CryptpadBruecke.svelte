<script>
  // Sammel-Formular ↔ CryptPad über die Zwischenablage (Logik und
  // Begründung: $lib/cryptpadBruecke.js). Die App sendet selbst nichts:
  //  ↗ Hinweg: Text mit Abschnitts-Markern in die Zwischenablage kopieren,
  //    ein neues CryptPad-Dokument öffnen, danach den Link als
  //    Arbeitsdokument speichern.
  //  ↙ Rückweg: Text aus CryptPad einfügen, Vorschau der Änderungen,
  //    erst nach Bestätigung ins Formular (dann wie gewohnt „Speichern").
  import { openUrl } from "@tauri-apps/plugin-opener";
  import { invoke } from "@tauri-apps/api/core";
  import { formularAlsText, textEinlesen, CRYPTPAD_NEU } from "$lib/cryptpadBruecke";
  import { dokumentPruefen, TITEL_MAX, LINK_MAX } from "$lib/arbeitsdokumente";

  // aktuell(): Momentaufnahme der Formular-Arbeitskopie (auch ungespeichert).
  // uebernehmen(aenderungen): wendet bestätigte Änderungen auf die Kopie an.
  // cryptpad: { projektName, hinweisAus, hinweisMerken(), dokumentSpeichern(titel, url) }
  let { aktuell, uebernehmen, cryptpad } = $props();

  // ---------- Hinweg ----------
  /** @type {"hinweis" | "link" | null} */
  let hinSchritt = $state(null);
  let nichtMehrZeigen = $state(false);
  let kopiert = $state(false);
  let ersatzText = $state(""); // falls die Zwischenablage nicht geht: Text zum Kopieren
  let dokTitel = $state("");
  let dokLink = $state("");
  let dokFehler = $state("");
  let dokGespeichert = $state(false);

  function hinwegStart() {
    if (cryptpad.hinweisAus) kopierenUndOeffnen();
    else {
      nichtMehrZeigen = false;
      hinSchritt = "hinweis";
    }
  }
  async function kopierenUndOeffnen() {
    const text = formularAlsText(aktuell(), cryptpad.projektName);
    kopiert = false;
    ersatzText = "";
    // Nativ über das offizielle Tauri-Plugin (nur Schreiben freigegeben);
    // der Browser-Weg navigator.clipboard war im App-Fenster unzuverlässig.
    // Den Text zeigen wir zusätzlich immer an – zum Nachkopieren von Hand.
    ersatzText = text;
    try {
      await invoke("plugin:clipboard-manager|write_text", { text });
      kopiert = true;
    } catch {
      kopiert = false;
    }
    if (nichtMehrZeigen) await cryptpad.hinweisMerken();
    dokTitel = "Sammel-Formular" + (cryptpad.projektName ? " – " + cryptpad.projektName : "");
    dokLink = "";
    dokFehler = "";
    dokGespeichert = false;
    hinSchritt = "link";
    openUrl(CRYPTPAD_NEU);
  }
  let nochmalOk = $state(false);
  async function nochmalKopieren() {
    nochmalOk = false;
    try {
      await invoke("plugin:clipboard-manager|write_text", { text: ersatzText });
      nochmalOk = true;
    } catch {
      kopiert = false;
    }
  }
  async function linkSpeichern() {
    const r = dokumentPruefen(dokTitel, dokLink);
    if (!r.ok) {
      dokFehler = r.fehler ?? "";
      return;
    }
    await cryptpad.dokumentSpeichern(r.titel, r.url);
    dokGespeichert = true;
  }

  // ---------- Rückweg ----------
  /** @type {"einfuegen" | "vorschau" | null} */
  let rueckSchritt = $state(null);
  let eingefuegt = $state("");
  let einleseFehler = $state("");
  /** @type {ReturnType<typeof textEinlesen> | null} */
  let ergebnis = $state(null);
  let meldung = $state("");

  function rueckwegStart() {
    eingefuegt = "";
    einleseFehler = "";
    ergebnis = null;
    rueckSchritt = "einfuegen";
  }
  function pruefen() {
    const r = textEinlesen(eingefuegt, aktuell());
    if (r.aenderungen.length === 0 && r.unveraendert === 0) {
      einleseFehler =
        "Im eingefügten Text wurden keine Abschnitts-Marker (=== … ===) gefunden. Bitte in CryptPad den ganzen Text markieren (Strg+A) und kopieren (Strg+C).";
      return;
    }
    ergebnis = r;
    rueckSchritt = "vorschau";
  }
  function bestaetigen() {
    if (!ergebnis) return;
    const n = ergebnis.aenderungen.length;
    uebernehmen(ergebnis.aenderungen);
    rueckSchritt = null;
    meldung = n ? `✓ ${n} Feld${n === 1 ? "" : "er"} übernommen – bitte noch „Speichern"` : "";
  }
  function kurz(s) {
    const t = (s ?? "").toString().trim();
    if (!t) return "(leer)";
    return t.length > 220 ? t.slice(0, 220) + " …" : t;
  }
</script>

<button class="zweit" onclick={hinwegStart} title="Formular-Texte in ein neues CryptPad-Dokument bringen">↗ In CryptPad öffnen</button>
<button class="zweit" onclick={rueckwegStart} title="Text aus CryptPad zurück ins Formular holen">↙ Aus CryptPad übernehmen</button>
{#if meldung}<span class="meldung">{meldung}</span>{/if}

{#if hinSchritt}
  <div class="schleier" onclick={() => (hinSchritt = null)} role="presentation">
    <div class="dialog" onclick={(e) => e.stopPropagation()} role="presentation">
      {#if hinSchritt === "hinweis"}
        <h2>Formular in CryptPad öffnen</h2>
        <p>Antrag 3000 kopiert alle Texte dieses Formulars in die <strong>Zwischenablage</strong>
          und öffnet ein <strong>neues CryptPad-Dokument</strong> im Browser. Dort fügst du sie mit
          <strong>Strg+V</strong> ein.</p>
        <p class="warn">Danach liegen deine Projekttexte bei CryptPad (cryptpad.fr) –
          <strong>Ende-zu-Ende-verschlüsselt</strong>, aber nicht mehr nur auf diesem Gerät.
          Antrag 3000 selbst sendet nichts; das Einfügen machst du.</p>
        <p class="klein">Die Zeilen mit <code>=== … ===</code> bitte in CryptPad stehen lassen – an ihnen
          erkennt die App beim Zurückholen, welcher Text in welches Feld gehört.</p>
        <label class="nichtmehr"><input type="checkbox" bind:checked={nichtMehrZeigen} /> Hinweis bei diesem Projekt nicht mehr anzeigen</label>
        <div class="dialog-knoepfe">
          <button class="leise" onclick={() => (hinSchritt = null)}>Abbrechen</button>
          <button class="primaer" onclick={kopierenUndOeffnen}>Text kopieren und CryptPad öffnen</button>
        </div>
      {:else}
        <h2>Jetzt in CryptPad einfügen</h2>
        {#if kopiert}
          <p class="ok">✓ In die Zwischenablage kopiert. CryptPad öffnet sich im Browser.</p>
        {:else}
          <p class="warn">Die Zwischenablage war nicht erreichbar. Klick in das Feld unten,
            dann Strg+A und Strg+C – und in CryptPad Strg+V.</p>
        {/if}
        <textarea class="ersatz" readonly rows="5" value={ersatzText} onfocus={(e) => e.currentTarget.select()}></textarea>
        <div class="nochmal"><button class="leise" onclick={nochmalKopieren}>📋 Nochmal kopieren</button>
          {#if nochmalOk}<span class="ok">✓ kopiert</span>{/if}</div>
        <ol>
          <li>Im neuen CryptPad-Dokument <strong>Strg+V</strong> drücken.</li>
          <li>Oben links den Titel des Dokuments anpassen, wenn du magst.</li>
          <li><strong>Erst danach</strong> den Link kopieren – aus der Adresszeile des Browsers oder
            über „Teilen". (Vorher kopiert, würde er den Text in der Zwischenablage ersetzen.)
            Mit dem Link gibst du das Dokument auch deinem Team.</li>
          <li>Den Link hier einfügen, damit du das Dokument in der App wiederfindest:</li>
        </ol>
        {#if dokGespeichert}
          <p class="ok">✓ Als Arbeitsdokument gespeichert – du findest es oben im Formular.</p>
        {:else}
          <label for="cp-titel">Titel</label>
          <input id="cp-titel" type="text" maxlength={TITEL_MAX} bind:value={dokTitel} oninput={() => (dokFehler = "")} />
          <label for="cp-link">Link aus CryptPad</label>
          <input id="cp-link" type="text" maxlength={LINK_MAX} placeholder="https://cryptpad.fr/code/#/2/code/edit/…"
            bind:value={dokLink} oninput={() => (dokFehler = "")} />
          {#if dokFehler}<p class="fehler">{dokFehler}</p>{/if}
        {/if}
        <div class="dialog-knoepfe">
          <button class="leise" onclick={() => (hinSchritt = null)}>{dokGespeichert ? "Fertig" : "Ohne Link schließen"}</button>
          {#if !dokGespeichert}<button class="primaer" onclick={linkSpeichern}>Als Arbeitsdokument speichern</button>{/if}
        </div>
      {/if}
    </div>
  </div>
{/if}

{#if rueckSchritt}
  <div class="schleier" onclick={() => (rueckSchritt = null)} role="presentation">
    <div class="dialog" onclick={(e) => e.stopPropagation()} role="presentation">
      <h2>Aus CryptPad übernehmen</h2>
      {#if rueckSchritt === "einfuegen"}
        <p>In CryptPad den ganzen Text markieren (<strong>Strg+A</strong>) und kopieren
          (<strong>Strg+C</strong>), dann hier einfügen (<strong>Strg+V</strong>).</p>
        <!-- svelte-ignore a11y_autofocus -->
        <textarea rows="8" placeholder="=== Projekttitel ===&#10;…" bind:value={eingefuegt} oninput={() => (einleseFehler = "")} autofocus></textarea>
        {#if einleseFehler}<p class="fehler">{einleseFehler}</p>{/if}
        <p class="klein">Übernommen werden nur Abschnitte mit den Markern <code>=== … ===</code>.
          Du siehst vorher genau, was sich ändert.</p>
        <div class="dialog-knoepfe">
          <button class="leise" onclick={() => (rueckSchritt = null)}>Abbrechen</button>
          <button class="primaer" disabled={!eingefuegt.trim()} onclick={pruefen}>Prüfen</button>
        </div>
      {:else if ergebnis}
        {#if ergebnis.aenderungen.length}
          <p>Diese Felder ändern sich:</p>
          {#each ergebnis.aenderungen as a (a.titel)}
            <div class="aenderung">
              <strong>{a.titel}</strong>
              <div class="alt">{kurz(a.alt)}</div>
              <div class="neu">{kurz(a.neu)}</div>
            </div>
          {/each}
        {:else}
          <p class="ok">Keine Änderungen – der Text in CryptPad entspricht dem Formular.</p>
        {/if}
        {#if ergebnis.unveraendert}<p class="klein">{ergebnis.unveraendert} Feld(er) unverändert.</p>{/if}
        {#if ergebnis.fehlend.length}
          <p class="warn">Nicht gefunden (Marker fehlt) – bleibt wie bisher: {ergebnis.fehlend.join(", ")}</p>
        {/if}
        {#if ergebnis.unbekannt.length}
          <p class="warn">Unbekannte Abschnitte – werden ignoriert: {ergebnis.unbekannt.join(", ")}</p>
        {/if}
        {#if ergebnis.doppelt.length}
          <p class="warn">Doppelt vorhanden – nur das erste Vorkommen zählt: {ergebnis.doppelt.join(", ")}</p>
        {/if}
        <div class="dialog-knoepfe">
          <button class="leise" onclick={() => (rueckSchritt = "einfuegen")}>Zurück</button>
          {#if ergebnis.aenderungen.length}
            <button class="primaer" onclick={bestaetigen}>{ergebnis.aenderungen.length} Feld{ergebnis.aenderungen.length === 1 ? "" : "er"} übernehmen</button>
          {:else}
            <button class="primaer" onclick={() => (rueckSchritt = null)}>Schließen</button>
          {/if}
        </div>
      {/if}
    </div>
  </div>
{/if}

<style>
  button.zweit {
    padding: 8px 16px;
    font-size: 0.9rem;
    font-weight: 600;
    font-family: inherit;
    color: var(--text);
    background: var(--weiss);
    border: 2px solid var(--rand);
    border-radius: 8px;
    cursor: pointer;
  }
  button.zweit:hover {
    border-color: var(--akzent);
  }
  .meldung {
    flex-basis: 100%;
    text-align: right;
    font-size: 0.85rem;
    font-weight: 600;
    color: var(--erfolg-text);
  }
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
    max-width: 560px;
    width: 100%;
    max-height: 88vh;
    overflow-y: auto;
    box-shadow: 0 12px 40px var(--schatten-lg);
  }
  .dialog h2 {
    margin: 0 0 14px;
    font-size: 1.2rem;
  }
  .dialog p,
  .dialog li {
    font-size: 0.92rem;
    line-height: 1.55;
  }
  .dialog p {
    margin: 0 0 10px;
  }
  .dialog ol {
    margin: 4px 0 10px;
    padding-left: 22px;
  }
  .warn {
    background: var(--warnung-bg);
    color: var(--warnung-text2);
    border-radius: 8px;
    padding: 8px 12px;
  }
  .ok {
    color: var(--erfolg-text);
    font-weight: 600;
  }
  .klein {
    color: var(--text-muted);
    font-size: 0.84rem !important;
  }
  .fehler {
    color: var(--gefahr-text);
    font-size: 0.85rem !important;
    margin-top: 6px !important;
  }
  label {
    display: block;
    margin: 10px 0 4px;
    font-size: 0.85rem;
    font-weight: 600;
    color: var(--text-2);
  }
  .nichtmehr {
    display: flex;
    align-items: center;
    gap: 8px;
    margin: 14px 0 6px;
    font-weight: 400;
    font-size: 0.9rem;
    cursor: pointer;
  }
  .nichtmehr input {
    width: auto;
  }
  input[type="text"],
  textarea {
    width: 100%;
    box-sizing: border-box;
    padding: 9px 11px;
    font-size: 0.9rem;
    font-family: inherit;
    border: 1px solid var(--rand);
    border-radius: 8px;
  }
  textarea {
    resize: vertical;
    font-family: Consolas, "Cascadia Mono", monospace;
    font-size: 0.85rem;
  }
  .ersatz {
    margin-top: 2px;
  }
  .nochmal {
    display: flex;
    align-items: center;
    gap: 10px;
    margin: 2px 0 6px;
  }
  .nochmal .ok {
    font-size: 0.85rem;
  }
  .aenderung {
    padding: 8px 0;
    border-top: 1px solid var(--flaeche-2b);
    font-size: 0.88rem;
  }
  .aenderung .alt {
    color: var(--gefahr-text);
    text-decoration: line-through;
    white-space: pre-wrap;
    margin-top: 2px;
  }
  .aenderung .neu {
    color: var(--erfolg-text);
    white-space: pre-wrap;
    margin-top: 2px;
  }
  .dialog-knoepfe {
    display: flex;
    justify-content: flex-end;
    gap: 10px;
    margin-top: 16px;
  }
  .primaer {
    padding: 10px 20px;
    font-size: 0.92rem;
    font-weight: 600;
    font-family: inherit;
    color: var(--auf-farbe);
    background: var(--akzent);
    border: none;
    border-radius: 8px;
    cursor: pointer;
  }
  .primaer:hover:not(:disabled) {
    background: var(--akzent-d);
  }
  .primaer:disabled {
    background: var(--grau-3);
    cursor: default;
  }
  .leise {
    background: none;
    border: none;
    color: var(--text-muted);
    font-size: 0.875rem;
    cursor: pointer;
    padding: 6px;
    font-family: inherit;
  }
  .leise:hover {
    color: var(--text);
    text-decoration: underline;
  }
</style>
