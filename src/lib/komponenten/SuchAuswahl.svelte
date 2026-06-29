<script>
  // Freitext-Suchfeld mit Auswahl-Liste (Combobox). Generisch: die
  // Trefferliste liefert eine Suchfunktion des Aufrufers; ausgewählt wird
  // ein Eintrag { wert, label }. Mit X lässt sich die Auswahl leeren.
  let {
    platzhalter = "Suchen …",
    label = "",            // Klartext der aktuellen Auswahl (von außen)
    suche,                 // (text) => [{ wert, label }]
    onwaehlen,             // ({wert,label} | null) => void
    deaktiviert = false,
  } = $props();

  let eingabe = $state(label ?? "");
  let offen = $state(false);

  // Wenn sich die Auswahl von außen ändert (z. B. Land gewechselt →
  // zurückgesetzt), Eingabefeld nachziehen.
  $effect(() => {
    eingabe = label ?? "";
  });

  let treffer = $derived(offen && !deaktiviert ? suche(eingabe) : []);

  function waehle(o) {
    onwaehlen(o);
    eingabe = o.label;
    offen = false;
  }
  function leeren() {
    onwaehlen(null);
    eingabe = "";
    offen = false;
  }
</script>

<div class="combo" class:aus={deaktiviert}>
  <div class="feldzeile">
    <input
      type="text"
      placeholder={platzhalter}
      bind:value={eingabe}
      disabled={deaktiviert}
      onfocus={() => (offen = true)}
      oninput={() => (offen = true)}
      onblur={() => setTimeout(() => (offen = false), 150)}
    />
    {#if eingabe && !deaktiviert}
      <button class="x" type="button" onmousedown={leeren} aria-label="Auswahl löschen">✕</button>
    {/if}
  </div>
  {#if offen && treffer.length}
    <ul class="liste">
      {#each treffer as o (o.wert)}
        <li>
          <button type="button" onmousedown={() => waehle(o)}>{o.label}</button>
        </li>
      {/each}
    </ul>
  {/if}
</div>

<style>
  .combo { position: relative; }
  .feldzeile { position: relative; display: flex; align-items: center; }
  input {
    width: 100%; box-sizing: border-box; padding: 10px 34px 10px 12px;
    font-size: 0.95rem; font-family: inherit;
    border: 2px solid var(--rand); border-radius: 8px; background: var(--flaeche);
  }
  input:focus { outline: none; border-color: var(--akzent); background: var(--weiss); }
  input:disabled { background: var(--flaeche-2); color: var(--grau-4); }
  .x {
    position: absolute; right: 8px; border: none; background: none;
    color: var(--text-leise); cursor: pointer; font-size: 0.85rem; padding: 4px;
  }
  .x:hover { color: var(--gefahr-text); }
  .liste {
    position: absolute; z-index: 20; left: 0; right: 0; top: calc(100% + 4px);
    margin: 0; padding: 4px; list-style: none; max-height: 240px; overflow-y: auto;
    background: var(--weiss); border: 1px solid var(--rand); border-radius: 8px;
    box-shadow: 0 8px 24px var(--schatten-md);
  }
  .liste li { margin: 0; }
  .liste button {
    width: 100%; text-align: left; padding: 8px 10px; border: none; background: none;
    border-radius: 6px; cursor: pointer; font-size: 0.92rem; font-family: inherit; color: var(--text);
  }
  .liste button:hover { background: var(--akzent-bg); }
</style>
