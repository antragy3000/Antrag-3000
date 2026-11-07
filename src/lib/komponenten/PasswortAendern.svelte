<script>
  // Tresor-Passwort ändern. Die eigentliche Arbeit macht Rust
  // (tresor_passwort_aendern): altes Passwort prüfen, mit neuem Salt neu
  // verschlüsseln, Vorversion mit altem Passwort löschen, „Merken" erneuern.
  // Diese Komponente sammelt nur die Eingaben und zeigt das Ergebnis.
  //
  // Passwort vergessen: Wurde die App in dieser Sitzung über „Auf diesem
  // Gerät merken" geöffnet, darf ein neues Passwort OHNE das alte gesetzt
  // werden (tresor_passwort_neu_setzen). Ob das geht, entscheidet Rust –
  // die Anzeige hier fragt nur nach (tresor_ueber_merken).
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";

  // aktuelleDaten(): JSON-Text des aktuellen Tresor-Inhalts (wie beim Speichern).
  // org: ID der offenen Organisation (eigene Tresor-Datei, Variante B+).
  let { org, aktuelleDaten, schliessen } = $props();

  let alt = $state("");
  let neu = $state("");
  let wiederholen = $state("");
  let fehler = $state("");
  let beschaeftigt = $state(false);
  /** @type {null | { merkenOk: boolean }} */
  let fertig = $state(null);
  // Darf der „vergessen"-Weg angeboten werden? Und ist er gerade gewählt?
  let vergessenMoeglich = $state(false);
  let vergessen = $state(false);

  onMount(async () => {
    try {
      vergessenMoeglich = !!(await invoke("tresor_ueber_merken", { org }));
    } catch {
      vergessenMoeglich = false;
    }
  });

  async function aendern() {
    fehler = "";
    if (!vergessen && !alt) return (fehler = "Bitte dein aktuelles Passwort eingeben.");
    if ([...neu].length < 8) return (fehler = "Das neue Passwort muss mindestens 8 Zeichen haben.");
    if (neu !== wiederholen) return (fehler = "Die beiden neuen Passwörter stimmen nicht überein.");
    if (!vergessen && neu === alt) return (fehler = "Das neue Passwort ist dasselbe wie das alte.");
    beschaeftigt = true;
    try {
      const merkenOk = vergessen
        ? await invoke("tresor_passwort_neu_setzen", { org, neuesPasswort: neu, daten: aktuelleDaten() })
        : await invoke("tresor_passwort_aendern", {
            org,
            altesPasswort: alt,
            neuesPasswort: neu,
            daten: aktuelleDaten(),
          });
      fertig = { merkenOk: !!merkenOk };
    } catch (e) {
      const code = String(e);
      fehler =
        code === "falsches_passwort"
          ? vergessen
            ? "Der Tresor ließ sich nicht neu verschlüsseln. Bitte die App neu starten und es nochmal versuchen."
            : "Das aktuelle Passwort stimmt nicht."
          : code === "nur_ueber_merken"
            ? "Das geht nur, wenn die App über „Auf diesem Gerät merken“ geöffnet wurde."
            : "Das Passwort konnte nicht geändert werden: " + e;
    } finally {
      beschaeftigt = false;
      alt = "";
      neu = "";
      wiederholen = "";
    }
  }

  /** @param {boolean} wert */
  function wegWechseln(wert) {
    vergessen = wert;
    fehler = "";
    alt = "";
  }
</script>

<div class="schleier" onclick={() => !beschaeftigt && schliessen()} role="presentation">
  <div class="dialog" onclick={(e) => e.stopPropagation()} role="presentation">
    <h2>{vergessen ? "Neues Passwort festlegen" : "Tresor-Passwort ändern"}</h2>
    {#if fertig}
      <p class="ok">✓ Dein Tresor ist jetzt mit dem neuen Passwort verschlüsselt.</p>
      {#if !fertig.merkenOk}
        <p class="warn">„Auf diesem Gerät merken" musste dabei ausgeschaltet werden. Du kannst
          es beim nächsten Entsperren wieder einschalten.</p>
      {/if}
      <p class="klein">Ältere <strong>Sicherungen</strong> bleiben mit dem <strong>alten</strong>
        Passwort verschlüsselt. Mach jetzt am besten eine neue Sicherung (🛡 Sicherung oben rechts).</p>
      <div class="knoepfe"><button class="primaer" onclick={schliessen}>Fertig</button></div>
    {:else}
      {#if vergessen}
        <p class="klein">Weil du die App über <strong>„Auf diesem Gerät merken“</strong> geöffnet
          hast, kannst du hier ein neues Passwort festlegen, ohne das alte zu kennen. Ab dann gilt
          nur noch das neue.</p>
        <p class="warn">Ältere <strong>Sicherungen</strong> lassen sich danach nur noch mit dem
          <strong>alten</strong> (vergessenen) Passwort öffnen. Mach deshalb gleich danach eine
          neue Sicherung.</p>
      {:else}
        <p class="klein">Merk dir das neue Passwort gut: Ohne Passwort lassen sich deine Daten
          nicht wiederherstellen – so ist es gewollt.</p>
        <label for="pw-alt">Aktuelles Passwort</label>
        <input id="pw-alt" type="password" autocomplete="current-password" bind:value={alt} disabled={beschaeftigt} />
        {#if vergessenMoeglich}
          <button class="link" disabled={beschaeftigt} onclick={() => wegWechseln(true)}>
            Ich kenne mein aktuelles Passwort nicht mehr
          </button>
        {/if}
      {/if}
      <label for="pw-neu">Neues Passwort (mindestens 8 Zeichen)</label>
      <input id="pw-neu" type="password" autocomplete="new-password" bind:value={neu} disabled={beschaeftigt} />
      <label for="pw-neu2">Neues Passwort wiederholen</label>
      <input id="pw-neu2" type="password" autocomplete="new-password" bind:value={wiederholen} disabled={beschaeftigt}
        onkeydown={(e) => e.key === "Enter" && aendern()} />
      {#if fehler}<p class="fehler">⚠ {fehler}</p>{/if}
      <div class="knoepfe">
        {#if vergessen}
          <button class="leise" disabled={beschaeftigt} onclick={() => wegWechseln(false)}>Zurück</button>
        {:else}
          <button class="leise" disabled={beschaeftigt} onclick={schliessen}>Abbrechen</button>
        {/if}
        <button class="primaer" disabled={beschaeftigt} onclick={aendern}>
          {beschaeftigt ? "Wird neu verschlüsselt …" : vergessen ? "Neues Passwort festlegen" : "Passwort ändern"}
        </button>
      </div>
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
    max-width: 460px;
    width: 100%;
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
  .link {
    display: inline-block;
    margin-top: 6px;
    padding: 0;
    background: none;
    border: none;
    color: var(--link);
    font-size: 0.82rem;
    font-family: inherit;
    text-decoration: underline;
    cursor: pointer;
  }
  .knoepfe {
    display: flex;
    justify-content: flex-end;
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
