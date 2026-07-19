<script>
  // Sammel-Formular: die typischen Antragsfragen, EINMAL pro Projekt
  // beantwortet. Tresor-Inhalt (Projektbeschriebe sind sensibel).
  // Aus diesen Antworten entstehen pro Foerderung antworten.json
  // und die Word-Datei.
  import { FORMULAR_FELDER } from "$lib/antrag";

  let { formular, speichern, wordErzeugen = null } = $props();

  let kopie = $state(structuredClone($state.snapshot(formular)));
  let einmalGespeichert = $state(false);
  let beschaeftigt = $state(false);
  let wordBeschaeftigt = $state(false);
  let wordErstellt = $state(false);

  let veraendert = $derived(
    JSON.stringify($state.snapshot(kopie)) !==
      JSON.stringify($state.snapshot(formular))
  );

  async function speichernKlick() {
    beschaeftigt = true;
    try {
      await speichern(structuredClone($state.snapshot(kopie)));
      einmalGespeichert = true;
    } finally {
      beschaeftigt = false;
    }
  }

  // Erzeugt das Word im Projektordner aus dem, was gerade auf dem
  // Bildschirm steht (auch ungespeicherte Änderungen).
  async function wordKlick() {
    if (!wordErzeugen) return;
    wordBeschaeftigt = true;
    try {
      await wordErzeugen(structuredClone($state.snapshot(kopie)));
      wordErstellt = true;
    } catch (e) {
      alert("Das Word konnte nicht erstellt werden.\n" + e);
    } finally {
      wordBeschaeftigt = false;
    }
  }
</script>

<div class="bereich">
  <div class="kopfzeile">
    <div>
      <h2>Sammel-Formular</h2>
      <p class="untertitel">
        Beantworte die typischen Antragsfragen einmal für dieses Projekt.
        Mit <strong>Speichern</strong> bleibt alles verschlüsselt im Tresor.
        Mit <strong>Word erstellen</strong> legst du diese Projektangaben
        als Word-Datei in den Projektordner – ohne Stammdaten und ohne
        Kostenfinanzplan.
      </p>
    </div>
    <div class="speichern-bereich">
      {#if !veraendert && einmalGespeichert}
        <span class="ok">✓ verschlüsselt gespeichert</span>
      {/if}
      <button disabled={!veraendert || beschaeftigt} onclick={speichernKlick}>
        {beschaeftigt ? "Speichert …" : "Speichern"}
      </button>
      {#if wordErzeugen}
        <button class="word" disabled={wordBeschaeftigt} onclick={wordKlick}>
          {wordBeschaeftigt
            ? "Erstellt …"
            : wordErstellt
              ? "✓ Word erstellt – neu erstellen"
              : "📄 Word erstellen"}
        </button>
      {/if}
    </div>
  </div>

  <div class="karte">
    {#each FORMULAR_FELDER as [key, beschriftung, typ] (key)}
      <label for={"feld-" + key}>{beschriftung}</label>
      {#if typ === "textarea"}
        <textarea
          id={"feld-" + key}
          rows={key === "beschreibung" ? 8 : 4}
          bind:value={kopie[key]}
        ></textarea>
      {:else}
        <input id={"feld-" + key} type="text" bind:value={kopie[key]} />
      {/if}
    {/each}
  </div>
</div>

<style>
  .bereich {
    max-width: 760px;
    margin: 0 auto;
    padding: 32px 24px 64px;
  }

  .kopfzeile {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 16px;
    flex-wrap: wrap;
    margin-bottom: 24px;
  }
  h2 {
    margin: 0 0 4px;
    font-size: 1.35rem;
    font-weight: 600;
  }
  .untertitel {
    margin: 0;
    color: var(--text-muted);
    font-size: 0.9rem;
    max-width: 460px;
    line-height: 1.5;
  }

  .speichern-bereich {
    display: flex;
    align-items: center;
    gap: 14px;
  }
  .ok {
    color: var(--erfolg-text);
    font-size: 0.88rem;
    font-weight: 600;
  }

  button {
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
  button:hover:not(:disabled) {
    background: var(--akzent-d);
  }
  button:disabled {
    background: var(--grau-3);
    cursor: default;
  }

  /* Word-Knopf: zweitrangig (weiß), gleiche Höhe wie Speichern */
  button.word {
    padding: 8px 20px;
    color: var(--text);
    background: var(--weiss);
    border: 2px solid var(--rand);
  }
  button.word:hover:not(:disabled) {
    background: var(--warnung-bg2);
    border-color: var(--warnung);
  }
  button.word:disabled {
    color: var(--text-leise);
    background: var(--flaeche-2);
    border-color: var(--rand);
  }

  .karte {
    background: var(--weiss);
    border-radius: 12px;
    box-shadow: 0 1px 3px var(--schatten-sm);
    padding: 32px;
  }

  label {
    display: block;
    font-size: 0.82rem;
    font-weight: 600;
    color: var(--text-muted);
    margin: 18px 0 5px;
  }
  label:first-of-type {
    margin-top: 0;
  }

  input,
  textarea {
    width: 100%;
    box-sizing: border-box;
    padding: 9px 12px;
    font-size: 0.95rem;
    font-family: inherit;
    line-height: 1.5;
    border: 2px solid var(--rand);
    border-radius: 8px;
    background: var(--flaeche);
    transition: border-color 0.15s, background 0.15s;
    resize: vertical;
  }
  input:focus,
  textarea:focus {
    outline: none;
    border-color: var(--akzent);
    background: var(--weiss);
  }
</style>
