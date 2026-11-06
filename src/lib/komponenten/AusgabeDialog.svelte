<script>
  // Dialog „Nächste Ausgabe anlegen": Name, was von der vorigen Ausgabe
  // mitkommt, und die Projektgruppe. Die Regeln dazu stehen in $lib/ausgaben.
  import { projektNamePruefen } from "$lib/projektname";
  import { ausgabeNameVorschlag, gruppenNameVorschlag } from "$lib/ausgaben";

  let { quelle, projekte, gruppen = [], abbrechen, anlegen } = $props();

  // Der Dialog entsteht beim Öffnen neu: Die Vorschläge sind Startwerte.
  // svelte-ignore state_referenced_locally
  let name = $state(ausgabeNameVorschlag(quelle.name, projekte));
  // svelte-ignore state_referenced_locally
  let gruppenName = $state(gruppenNameVorschlag(quelle.name));
  let texte = $state(true);
  let kostenplan = $state(true);
  let betraege = $state(false);
  let merkliste = $state(true);
  // Standardmäßig aus: Der Verwendungsnachweis der vorigen Ausgabe kann noch
  // offen sein, und das Archiv ist schreibgeschützt.
  let archivieren = $state(false);

  let pruefung = $derived(projektNamePruefen(name, projekte));
  let eigeneGruppe = $derived(gruppen.find((g) => g.id === quelle.gruppeId) ?? null);

  function absenden(event) {
    event.preventDefault();
    if (!pruefung.ok) return;
    anlegen({
      name: pruefung.name,
      gruppenName,
      texte,
      kostenplan,
      betraege: kostenplan && betraege,
      merkliste,
      archivieren: archivieren && !quelle.archiviert,
    });
  }
</script>

<div class="schleier" onclick={abbrechen} role="presentation">
  <form class="dialog" onsubmit={absenden} onclick={(e) => e.stopPropagation()}>
    <h2>Nächste Ausgabe anlegen</h2>

    <label class="feld" for="ausgabe-name">Name der neuen Ausgabe</label>
    <input id="ausgabe-name" type="text" bind:value={name} />
    {#if pruefung.fehler}<p class="fehler" role="alert">{pruefung.fehler}</p>{/if}

    <fieldset>
      <legend>Von „{quelle.name}" übernehmen:</legend>
      <label class="wahl">
        <input type="checkbox" bind:checked={texte} />
        <span>Texte aus dem Sammel-Formular <small>(ohne Zeitraum)</small></span>
      </label>
      <label class="wahl">
        <input type="checkbox" bind:checked={kostenplan} />
        <span>Kostenplan-Struktur</span>
      </label>
      <label class="wahl eingerueckt" class:aus={!kostenplan}>
        <input type="checkbox" bind:checked={betraege} disabled={!kostenplan} />
        <span>auch die Beträge</span>
      </label>
      <label class="wahl">
        <input type="checkbox" bind:checked={merkliste} />
        <span>Merkliste mit Kontakten und Checklisten <small>(Status beginnen neu)</small></span>
      </label>
    </fieldset>

    <p class="klein">
      <strong>Nie übernommen:</strong> Belege, Geldquellen, Sachbericht, hochgeladene
      Dateien, eigene und interne Fristen, Links zu Arbeitsdokumenten. Die vorige
      Ausgabe bleibt unverändert. Prüfe in den Texten danach Jahreszahlen und
      andere Angaben, die sich geändert haben.
    </p>

    {#if eigeneGruppe}
      <p class="klein">Die neue Ausgabe kommt in die Projektgruppe <strong>{eigeneGruppe.name}</strong>.</p>
    {:else}
      <label class="feld" for="ausgabe-gruppe">Projektgruppe für beide Ausgaben <small>(leer lassen für keine)</small></label>
      <input id="ausgabe-gruppe" type="text" bind:value={gruppenName} />
    {/if}

    {#if !quelle.archiviert}
      <label class="wahl archiv">
        <input type="checkbox" bind:checked={archivieren} />
        <span>„{quelle.name}" danach archivieren <small>(schreibgeschützt, lässt sich zurückholen)</small></span>
      </label>
    {/if}

    <div class="dialog-knoepfe">
      <button type="button" class="leise" onclick={abbrechen}>Abbrechen</button>
      <button type="submit" class="primaer" disabled={!pruefung.ok}>Ausgabe anlegen</button>
    </div>
  </form>
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
    padding: 32px;
    max-width: 500px;
    width: 100%;
    box-sizing: border-box;
    max-height: 85vh;
    overflow-y: auto;
    box-shadow: 0 12px 40px var(--schatten-lg);
  }
  h2 {
    margin: 0 0 16px;
    font-size: 1.2rem;
  }
  .feld {
    display: block;
    margin: 14px 0 6px;
    font-size: 0.85rem;
    font-weight: 600;
    color: var(--text-2);
  }
  input[type="text"] {
    width: 100%;
    box-sizing: border-box;
    padding: 10px 12px;
    font-size: 1rem;
    font-family: inherit;
    color: var(--text);
    border: 2px solid var(--rand);
    border-radius: 8px;
    background: var(--flaeche);
  }
  input[type="text"]:focus {
    outline: none;
    border-color: var(--akzent);
    background: var(--weiss);
  }
  .fehler {
    margin: 8px 0 0;
    font-size: 0.88rem;
    color: var(--gefahr-text);
  }
  fieldset {
    margin: 18px 0 0;
    padding: 0;
    border: none;
  }
  legend {
    padding: 0;
    margin-bottom: 8px;
    font-size: 0.85rem;
    font-weight: 600;
    color: var(--text-2);
    overflow-wrap: anywhere;
  }
  .wahl {
    display: flex;
    align-items: flex-start;
    gap: 9px;
    padding: 4px 0;
    font-size: 0.92rem;
    line-height: 1.35;
    color: var(--text);
    cursor: pointer;
    overflow-wrap: anywhere;
  }
  .wahl input {
    flex-shrink: 0;
    margin: 2px 0 0;
    cursor: inherit;
  }
  .wahl.eingerueckt {
    margin-left: 24px;
  }
  .wahl.aus {
    color: var(--text-leise);
    cursor: not-allowed;
  }
  .wahl.archiv {
    margin-top: 14px;
  }
  small {
    font-size: 0.82rem;
    font-weight: 400;
    color: var(--text-muted);
  }
  .klein {
    margin: 12px 0 0;
    font-size: 0.84rem;
    line-height: 1.5;
    color: var(--text-muted);
    overflow-wrap: anywhere;
  }
  .dialog-knoepfe {
    display: flex;
    justify-content: flex-end;
    gap: 10px;
    margin-top: 22px;
  }
  button {
    padding: 10px 18px;
    font-size: 0.95rem;
    font-weight: 600;
    font-family: inherit;
    border: none;
    border-radius: 8px;
    cursor: pointer;
  }
  button.leise {
    color: var(--text-muted);
    background: none;
  }
  button.primaer {
    color: var(--auf-farbe);
    background: var(--akzent);
  }
  button.primaer:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }
</style>
