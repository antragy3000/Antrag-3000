<script>
  // Organisation weitergeben (art = "geben") und eine weitergegebene
  // Organisation hinzufügen (art = "nehmen"). Die Arbeit an der Datei macht
  // Rust (weitergabe.rs); WAS mitgeht, bestimmt die Positivliste in
  // organisationen.js (weitergabeInhalt / weitergabeUebernehmen).
  //
  // Warum ein eigenes Weitergabe-Passwort? Die Datei geht an eine andere
  // Person. Sie soll weder das Passwort der Organisation erfahren noch die
  // Team-Verbindung bekommen – die bleibt auf diesem Gerät.
  import { invoke } from "@tauri-apps/api/core";
  import { save, open } from "@tauri-apps/plugin-dialog";
  import { namePruefen, weitergabeName, weitergabeUebernehmen, freierOrgName } from "$lib/organisationen.js";
  import { frischerTresor, normalisieren } from "$lib/tresorFormat.js";

  let {
    art,
    schliessen,
    // geben
    org = null,
    orgOrdner = "",
    name = "",
    inhalt = () => ({}),
    vorSichern = null,
    // nehmen
    aktivName = "",
    eintraege = [],
    vorlage = () => null,
    pruefePasswort = async (/** @type {string} */ _pw) => false,
    einrichten = async (/** @type {any} */ _neu, /** @type {string} */ _art, /** @type {string} */ _pw, /** @type {any} */ _dateien) => {},
  } = $props();

  let fehler = $state("");
  let beschaeftigt = $state(false);
  /** @type {string[]} */
  let probleme = $state([]);
  let meldung = $state("");

  /** @param {number} n */
  const dateienText = (n) => (n === 1 ? "1 Datei" : `${n} Dateien`);
  /** @param {unknown} e */
  const fehlerText = (e) =>
    String(e) === "falsches_passwort" ? "Dieses Weitergabe-Passwort passt nicht zur Datei." : String(e);

  // ---------------- geben ----------------
  let mitDateien = $state(true);
  let wgPasswort = $state("");
  let wgWiederholen = $state("");

  async function geben() {
    fehler = "";
    if ([...wgPasswort].length < 8) return (fehler = "Das Weitergabe-Passwort muss mindestens 8 Zeichen haben.");
    if (wgPasswort !== wgWiederholen) return (fehler = "Die beiden Passwörter stimmen nicht überein.");
    const heute = new Date().toISOString().slice(0, 10);
    // Bewusst ohne Namen im Dateinamen (wie bei der Sicherung).
    const ziel = await save({
      title: "Organisation weitergeben",
      defaultPath: `antrag3000-organisation-${heute}.a3korganisation`,
      filters: [{ name: "Antrag 3000 Organisation", extensions: ["a3korganisation"] }],
    });
    if (!ziel) return;
    beschaeftigt = true;
    try {
      if (vorSichern) await vorSichern();
      const b = await invoke("org_weitergeben", {
        org,
        ziel,
        passwort: wgPasswort,
        daten: JSON.stringify(inhalt()),
        mitDateien: mitDateien && !!orgOrdner,
        orgOrdner,
      });
      wgPasswort = wgWiederholen = "";
      probleme = b.probleme;
      meldung =
        `Gespeichert und geprüft: „${name}“` +
        (mitDateien && orgOrdner ? ` mit ${dateienText(b.dateien)}` : " ohne Dateien") +
        `.\n${ziel}`;
    } catch (e) {
      fehler = fehlerText(e);
    } finally {
      beschaeftigt = false;
    }
  }

  // ---------------- nehmen ----------------
  let datei = $state("");
  let dateiPasswort = $state("");
  // $state.raw: die Rohdaten bleiben ein schlichtes Objekt (kein Proxy), damit
  // structuredClone sie für jeden Übernahme-Versuch frisch kopieren kann.
  let geprueft = $state.raw(/** @type {null | { roh: any, dateien: number }} */ (null));
  let neuName = $state("");
  let pwArt = $state("gleich"); // "gleich" | "eigen"
  let pw = $state("");
  let pwWdh = $state("");
  const namePruefung = $derived(namePruefen(neuName, eintraege));
  const hatLinks = $derived(
    !!geprueft?.roh?.projekte?.some((/** @type {any} */ p) => p?.ablageUrl || p?.arbeitsdokumente?.length),
  );

  async function dateiWaehlen() {
    fehler = "";
    const pfad = await open({
      title: "Weitergegebene Organisation auswählen",
      multiple: false,
      filters: [{ name: "Antrag 3000 Organisation", extensions: ["a3korganisation"] }],
    });
    if (!pfad) return;
    try {
      const art = await invoke("sicherung_art", { quelle: pfad });
      if (art !== "organisation") {
        return (fehler = "Das ist keine weitergegebene Organisation. Eine Sicherung spielst du unter „Sicherung“ ein.");
      }
    } catch (e) {
      return (fehler = String(e));
    }
    datei = String(pfad);
    dateiPasswort = "";
    geprueft = null;
  }

  async function dateiPruefen() {
    fehler = "";
    beschaeftigt = true;
    try {
      const w = await invoke("weitergabe_pruefen", { quelle: datei, passwort: dateiPasswort });
      let roh;
      try {
        roh = JSON.parse(w.daten);
      } catch {
        throw "Die Datei enthält keine lesbare Organisation.";
      }
      // Schon hier einmal komplett übernehmen (ohne etwas anzulegen): Passt
      // der Aufbau nicht, erfährt man es, bevor irgendetwas entsteht.
      const probe = uebernehmen(roh, "Probe");
      if (!probe.ok) throw probe.fehler;
      geprueft = { roh, dateien: w.dateien };
      neuName = freierOrgName(weitergabeName(roh), eintraege);
    } catch (e) {
      fehler = fehlerText(e);
    } finally {
      beschaeftigt = false;
    }
  }

  /**
   * @param {any} roh @param {string} n
   * @returns {{ ok: true, daten: any } | { ok: false, fehler: string }}
   */
  function uebernehmen(roh, n) {
    // Frische Kopie der Rohdaten: normalisieren() verändert, was es bekommt.
    const r = weitergabeUebernehmen(frischerTresor(), vorlage(), structuredClone(roh), n);
    if (!r.ok) return r;
    try {
      normalisieren(r.daten);
    } catch {
      return { ok: false, fehler: "Die Datei enthält keine lesbare Organisation." };
    }
    return r;
  }

  async function hinzufuegen(/** @type {SubmitEvent} */ event) {
    event.preventDefault();
    fehler = "";
    if (!geprueft || !namePruefung.ok) return;
    if (pwArt === "eigen") {
      if ([...pw].length < 8) return (fehler = "Das Passwort muss mindestens 8 Zeichen haben.");
      if (pw !== pwWdh) return (fehler = "Die beiden Passwörter stimmen nicht überein.");
    }
    beschaeftigt = true;
    try {
      if (pwArt === "gleich" && !(await pruefePasswort(pw))) {
        pw = "";
        return (fehler = `Das ist nicht das Passwort von „${aktivName}“.`);
      }
      const r = uebernehmen(geprueft.roh, namePruefung.name);
      if (!r.ok) return (fehler = r.fehler);
      const anzahl = geprueft.dateien;
      /** @type {any} */
      let bericht = null;
      // Reihenfolge (in einrichten): Ordner anlegen → Dateien hinein → erst
      // dann den Tresor. So entsteht nie eine Organisation, der Dateien fehlen.
      await einrichten(r.daten, pwArt, pw, anzahl > 0
        ? async (/** @type {string} */ ordner) => {
            bericht = await invoke("weitergabe_dateien_einspielen", { quelle: datei, passwort: dateiPasswort, orgOrdner: ordner });
          }
        : null);
      pw = pwWdh = dateiPasswort = "";
      probleme = bericht?.probleme ?? [];
      meldung =
        `„${namePruefung.name}“ ist jetzt auf diesem Gerät` +
        (bericht ? ` – mit ${dateienText(bericht.dateien)}.` : ".");
    } catch (e) {
      fehler = fehlerText(e);
    } finally {
      beschaeftigt = false;
    }
  }
</script>

<div class="schleier" onclick={schliessen} role="presentation">
  <div class="dialog" onclick={(e) => e.stopPropagation()} role="presentation">
    {#if meldung}
      <h2>{art === "geben" ? "Weitergegeben" : "Hinzugefügt"}</h2>
      <p class="ok" style="white-space: pre-line">{meldung}</p>
      {#if art === "geben"}
        <p>Gib der anderen Person das Weitergabe-Passwort auf einem <strong>anderen Weg</strong> als die Datei (z. B. am Telefon).</p>
      {/if}
      {#if probleme.length > 0}
        <p class="warn">Nicht übernommen ({probleme.length}): {probleme.slice(0, 10).join(", ")}{probleme.length > 10 ? " …" : ""}</p>
      {/if}
      <div class="knoepfe"><button class="primaer" onclick={schliessen}>Fertig</button></div>
    {:else if art === "geben"}
      <h2>„{name}“ weitergeben</h2>
      <p>
        Speichert eine verschlüsselte Datei, mit der eine andere Person diese Organisation in
        ihr Antrag 3000 übernehmen kann – z. B. wenn jemand die Anträge weiterführt.
      </p>
      <p class="klein">
        <strong>Mit drin:</strong> Stammdaten (auch Bankverbindung und Steuerangaben), alle Projekte
        mit Anträgen, Kostenplänen und Abrechnung, Links zu gemeinsamen Dokumenten.<br />
        <strong>Nicht drin:</strong> dein Passwort, die Team-Verbindung, die Einstellungen dieses Geräts.
      </p>
      <label class="haken">
        <input type="checkbox" bind:checked={mitDateien} disabled={!orgOrdner} />
        <span>
          Dateien mitgeben (Belege, Anhänge, Word/PDF)
          {#if orgOrdner}<br /><span class="klein">aus Dokumente\Antrag 3000\{orgOrdner}</span>
          {:else}<br /><span class="klein">Geht erst, wenn die Organisation ihren eigenen Ordner hat (einmal sperren und neu anmelden).</span>{/if}
        </span>
      </label>
      <label for="wg-pw">Weitergabe-Passwort (mindestens 8 Zeichen)</label>
      <input id="wg-pw" type="password" bind:value={wgPasswort} autocomplete="new-password" oninput={() => (fehler = "")} />
      <label for="wg-pw2">Wiederholen</label>
      <input id="wg-pw2" type="password" bind:value={wgWiederholen} autocomplete="new-password" oninput={() => (fehler = "")} />
      <p class="klein">
        Nimm ein anderes Passwort als das deiner Organisation. Die andere Person braucht es zum Öffnen –
        sag es ihr auf einem anderen Weg als die Datei.
      </p>
      {#if fehler}<p class="fehler" role="alert">{fehler}</p>{/if}
      <div class="knoepfe">
        <button class="leise" onclick={schliessen}>Abbrechen</button>
        <button class="primaer" onclick={geben} disabled={!wgPasswort || !wgWiederholen || beschaeftigt}>
          {beschaeftigt ? "Speichere …" : "Datei speichern …"}
        </button>
      </div>
    {:else if !geprueft}
      <h2>Organisation aus Datei hinzufügen</h2>
      <p>
        Übernimmt eine Organisation, die dir jemand mit „Organisation weitergeben“ geschickt hat
        (Datei <strong>.a3korganisation</strong>). Sie kommt neu dazu – deine Organisationen bleiben,
        wie sie sind.
      </p>
      {#if datei}
        <p class="klein">{datei}</p>
        <label for="wg-datei-pw">Weitergabe-Passwort</label>
        <!-- svelte-ignore a11y_autofocus -->
        <input id="wg-datei-pw" type="password" bind:value={dateiPasswort} autocomplete="off" autofocus oninput={() => (fehler = "")} />
      {/if}
      {#if fehler}<p class="fehler" role="alert">{fehler}</p>{/if}
      <div class="knoepfe">
        <button class="leise" onclick={schliessen}>Abbrechen</button>
        {#if datei}
          <button class="leise" onclick={dateiWaehlen}>Andere Datei …</button>
          <button class="primaer" onclick={dateiPruefen} disabled={!dateiPasswort || beschaeftigt}>
            {beschaeftigt ? "Prüfe …" : "Datei prüfen"}
          </button>
        {:else}
          <button class="primaer" onclick={dateiWaehlen}>Datei auswählen …</button>
        {/if}
      </div>
    {:else}
      <form onsubmit={hinzufuegen}>
        <h2>Organisation hinzufügen</h2>
        <p class="ok">
          Datei geprüft: {geprueft.roh.projekte.length === 1 ? "1 Projekt" : `${geprueft.roh.projekte.length} Projekte`},
          {dateienText(geprueft.dateien)}.
        </p>
        {#if hatLinks}
          <p class="klein">
            Darin sind Links zu gemeinsamen Dokumenten (z. B. CryptPad). Wer sie hat, kann diese
            Dokumente öffnen.
          </p>
        {/if}
        <label for="wg-name">Name</label>
        <input id="wg-name" type="text" bind:value={neuName} />
        {#if namePruefung.fehler}<p class="fehler" role="alert">{namePruefung.fehler}</p>{/if}

        <fieldset>
          <legend>Passwort für diese Organisation</legend>
          <label class="haken">
            <input type="radio" name="wg-art" value="gleich" bind:group={pwArt} onchange={() => (fehler = "")} />
            <span>Gleiches Passwort wie „{aktivName}“</span>
          </label>
          <label class="haken">
            <input type="radio" name="wg-art" value="eigen" bind:group={pwArt} onchange={() => (fehler = "")} />
            <span>Eigenes Passwort</span>
          </label>
        </fieldset>
        {#if pwArt === "gleich"}
          <label for="wg-org-pw">Zur Bestätigung: Passwort von „{aktivName}“</label>
          <input id="wg-org-pw" type="password" bind:value={pw} autocomplete="current-password" oninput={() => (fehler = "")} />
        {:else}
          <label for="wg-org-pw">Neues Passwort (mindestens 8 Zeichen)</label>
          <input id="wg-org-pw" type="password" bind:value={pw} autocomplete="new-password" oninput={() => (fehler = "")} />
          <label for="wg-org-pw2">Passwort wiederholen</label>
          <input id="wg-org-pw2" type="password" bind:value={pwWdh} autocomplete="new-password" oninput={() => (fehler = "")} />
          <p class="klein">Auch dieses Passwort lässt sich nicht wiederherstellen.</p>
        {/if}
        {#if fehler}<p class="fehler" role="alert">{fehler}</p>{/if}
        <div class="knoepfe">
          <button type="button" class="leise" onclick={schliessen}>Abbrechen</button>
          <button type="submit" class="primaer" disabled={!namePruefung.ok || !pw || beschaeftigt}>
            {beschaeftigt ? "Füge hinzu …" : "Hinzufügen"}
          </button>
        </div>
      </form>
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
    z-index: 30;
  }
  .dialog {
    background: var(--weiss);
    border-radius: 12px;
    padding: 28px 32px;
    max-width: 480px;
    width: 100%;
    max-height: calc(100vh - 48px);
    overflow-y: auto;
    box-sizing: border-box;
    box-shadow: 0 12px 40px var(--schatten-lg);
  }
  h2 {
    margin: 0 0 12px;
    font-size: 1.2rem;
  }
  p {
    margin: 0 0 10px;
    font-size: 0.92rem;
    line-height: 1.55;
  }
  .klein {
    font-size: 0.85rem;
    color: var(--text-muted);
    overflow-wrap: anywhere;
  }
  fieldset {
    border: none;
    margin: 10px 0 0;
    padding: 0;
  }
  legend {
    font-size: 0.85rem;
    font-weight: 600;
    color: var(--text-2);
    margin-bottom: 2px;
  }
  .haken {
    display: flex;
    gap: 8px;
    align-items: flex-start;
    margin: 4px 0 10px;
    font-size: 0.88rem;
    font-weight: 400;
    color: inherit;
  }
  .haken input {
    width: auto;
    margin-top: 3px;
  }
  .ok {
    color: var(--erfolg-text);
    font-weight: 600;
  }
  .warn {
    background: var(--warnung-bg);
    color: var(--warnung-text2);
    border-radius: 8px;
    padding: 8px 12px;
  }
  .fehler {
    color: var(--gefahr-text);
    font-size: 0.88rem;
    margin-top: 8px;
  }
  label {
    display: block;
    margin: 10px 0 4px;
    font-size: 0.85rem;
    font-weight: 600;
    color: var(--text-2);
  }
  input {
    width: 100%;
    box-sizing: border-box;
    padding: 9px 11px;
    font-size: 0.95rem;
    font-family: inherit;
    border: 1px solid var(--rand);
    border-radius: 8px;
  }
  .knoepfe {
    display: flex;
    justify-content: flex-end;
    flex-wrap: wrap;
    gap: 10px;
    margin-top: 18px;
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
</style>
