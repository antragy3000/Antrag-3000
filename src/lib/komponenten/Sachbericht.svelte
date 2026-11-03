<script>
  // Abrechnungs-Modus: Sachbericht für den Verwendungsnachweis. Das Projekt hat
  // eine gemeinsame Vorlage; jede Geldquelle kann daraus ihren eigenen Text
  // bekommen, denn Förderer fragen oft nach Unterschiedlichem. Ohne eigenen
  // Text gilt für sie die Vorlage. Tresor-Inhalt (bleibt lokal).
  import { hatEigenenSachbericht } from "$lib/sachbericht";

  let { sachbericht = "", quellen = [], speichern, quelleSpeichern, projektName = "" } = $props();

  const VORLAGE = "vorlage";
  let auswahl = $state(VORLAGE);
  let text = $state(sachbericht);
  let beschaeftigt = $state(false);
  let gespeichert = $state(false);

  let quelle = $derived(quellen.find((q) => q.id === auswahl) ?? null);
  let gespeicherterText = $derived(quelle ? (quelle.sachbericht ?? "") : sachbericht);
  let veraendert = $derived(text !== gespeicherterText);
  let vorlageVorhanden = $derived(!!sachbericht.trim());
  // Gilt für die gewählte Geldquelle gerade nur die Vorlage? Dann zeigen wir
  // sie zum Lesen, statt ein leeres Feld anzubieten.
  let nurVorlage = $derived(!!quelle && !hatEigenenSachbericht(quelle) && vorlageVorhanden && !text.trim());
  let mitEigenemText = $derived(quellen.filter(hatEigenenSachbericht));
  let mitVorlage = $derived(quellen.filter((q) => !hatEigenenSachbericht(q)));

  async function sichern() {
    if (!veraendert) return;
    // Nur Leerzeichen sind kein eigener Text: Dann gilt wieder die Vorlage.
    if (!text.trim()) text = "";
    if (!veraendert) return;
    beschaeftigt = true;
    try {
      if (quelle) await quelleSpeichern(quelle.id, text);
      else await speichern(text);
      gespeichert = true;
      setTimeout(() => (gespeichert = false), 1600);
    } finally {
      beschaeftigt = false;
    }
  }

  /** @param {string} id */
  async function waehlen(id) {
    if (id === auswahl) return;
    await sichern();
    auswahl = id;
    text = gespeicherterText;
  }

  // Die Vorlage als Anfang des eigenen Textes übernehmen.
  async function anpassen() {
    text = sachbericht;
    await sichern();
  }

  async function zurueckZurVorlage() {
    if (!confirm(`Den eigenen Text für „${quelle.name}" löschen? Danach gilt wieder die gemeinsame Vorlage.`)) return;
    text = "";
    await sichern();
  }
</script>

<div class="bereich">
  <div class="kopfzeile">
    <div class="titel-block">
      <h2>Sachbericht</h2>
      <p class="untertitel">
        Was wurde{#if projektName}{" "}bei <strong>{projektName}</strong>{/if} umgesetzt, welche Ziele
        wurden erreicht, gab es Besonderheiten? Schreibe zuerst die gemeinsame Vorlage und passe sie
        dann für einzelne Förderer an.
      </p>
    </div>
    {#if !nurVorlage}
      <div class="status">
        {#if gespeichert}<span class="ok">✓ gespeichert</span>{/if}
        <button class="primaer" onclick={sichern} disabled={!veraendert || beschaeftigt}>
          {beschaeftigt ? "Speichert …" : "Speichern"}
        </button>
      </div>
    {/if}
  </div>

  {#if quellen.length > 0}
    <div class="reiter" role="tablist" aria-label="Sachbericht auswählen">
      <button role="tab" aria-selected={auswahl === VORLAGE} class:aktiv={auswahl === VORLAGE} onclick={() => waehlen(VORLAGE)}>
        <span class="r-name">Gemeinsame Vorlage</span>
      </button>
      {#each quellen as q (q.id)}
        <button role="tab" aria-selected={auswahl === q.id} class:aktiv={auswahl === q.id} onclick={() => waehlen(q.id)} title={q.name}>
          <span class="r-name">{q.name}</span>
          <span class="r-art" class:eigen={hatEigenenSachbericht(q)}>
            {hatEigenenSachbericht(q) ? "eigener Text" : "nutzt die Vorlage"}
          </span>
        </button>
      {/each}
    </div>
  {/if}

  {#if nurVorlage}
    <div class="vorlage-info">
      <p>
        Für <strong>{quelle.name}</strong> gilt gerade die gemeinsame Vorlage. So steht es im
        Verwendungsnachweis:
      </p>
      <div class="vorlage-text">{sachbericht}</div>
      <div class="vorlage-knoepfe">
        <button class="primaer" onclick={anpassen} disabled={beschaeftigt}>
          {quelle.typ === "foerderung" ? "Für diesen Förderer anpassen" : "Für diese Geldquelle anpassen"}
        </button>
        <button class="leise" onclick={() => waehlen(VORLAGE)}>Vorlage bearbeiten</button>
      </div>
    </div>
  {:else}
    <textarea
      bind:value={text}
      onblur={sichern}
      aria-label={quelle ? `Sachbericht für ${quelle.name}` : "Gemeinsame Sachbericht-Vorlage"}
      placeholder={quelle
        ? `Sachbericht für ${quelle.name} …`
        : "Sachbericht zum Projekt …\n\n• Ausgangslage und Ziele\n• Ablauf / Durchführung\n• Ergebnisse und Wirkung\n• Besonderheiten, Abweichungen vom Plan"}
    ></textarea>

    <div class="fuss">
      <span class="zeichen">{text.length} Zeichen</span>
      <span class="hinweis">Wird automatisch beim Verlassen des Feldes gespeichert.</span>
    </div>

    {#if quelle}
      <p class="erklaerung">
        Dieser Text erscheint nur im Verwendungsnachweis von <strong>{quelle.name}</strong>.
        {#if vorlageVorhanden}Spätere Änderungen an der Vorlage ändern ihn nicht.{/if}
      </p>
      {#if hatEigenenSachbericht(quelle)}
        {#if vorlageVorhanden}
          <details class="vergleich">
            <summary>Gemeinsame Vorlage zum Vergleich ansehen</summary>
            <div class="vorlage-text">{sachbericht}</div>
          </details>
        {/if}
        <button class="leise rechts" onclick={zurueckZurVorlage} disabled={beschaeftigt}>
          Eigenen Text löschen und wieder die Vorlage nutzen
        </button>
      {/if}
    {:else if quellen.length === 0}
      <p class="erklaerung">
        Dieser Text erscheint in jedem Verwendungsnachweis. Sobald du unter
        <strong>Geldquellen</strong> Förderer angelegt hast, kannst du ihn hier für jeden einzeln
        anpassen.
      </p>
    {:else}
      <p class="erklaerung">
        {#if mitVorlage.length > 0}
          Dieser Text erscheint im Verwendungsnachweis von:
          <strong>{mitVorlage.map((q) => q.name).join(", ")}</strong>.
        {:else}
          Alle Geldquellen haben gerade einen eigenen Text, diese Vorlage erscheint also nirgends.
        {/if}
        {#if mitVorlage.length > 0 && mitEigenemText.length > 0}
          Nicht betroffen, weil sie einen eigenen Text haben:
          {mitEigenemText.map((q) => q.name).join(", ")}.
        {/if}
      </p>
    {/if}
  {/if}
</div>

<style>
  .bereich {
    max-width: 920px;
    margin: 0 auto;
    padding: 32px 24px 64px;
    display: flex;
    flex-direction: column;
    min-height: 70vh;
  }
  .kopfzeile {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 16px;
    flex-wrap: wrap;
    margin-bottom: 16px;
  }
  .titel-block { flex: 1 1 300px; min-width: 260px; }
  h2 { margin: 0 0 4px; font-size: 1.35rem; font-weight: 600; color: var(--text); }
  .untertitel { margin: 0; color: var(--text-muted); font-size: 0.9rem; max-width: 560px; line-height: 1.5; }
  .status { display: flex; align-items: center; gap: 12px; }
  .ok { color: var(--erfolg-text2); font-size: 0.85rem; font-weight: 600; white-space: nowrap; }

  .reiter {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
    margin-bottom: 14px;
  }
  .reiter button {
    display: flex;
    flex-direction: column;
    justify-content: center;
    gap: 1px;
    max-width: 240px;
    min-height: 46px;
    padding: 6px 12px;
    text-align: left;
    font-family: inherit;
    color: var(--text);
    background: var(--weiss);
    border: 2px solid var(--rand);
    border-radius: 8px;
    cursor: pointer;
  }
  .reiter button:hover { border-color: var(--akzent); }
  .reiter button.aktiv {
    border-color: var(--akzent);
    background: var(--akzent-bg);
  }
  .r-name {
    max-width: 100%;
    font-size: 0.88rem;
    font-weight: 600;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .r-art { font-size: 0.74rem; color: var(--text-muted); }
  .r-art.eigen { color: var(--akzent-d4); font-weight: 600; }

  textarea {
    flex: 1;
    width: 100%;
    min-height: 340px;
    padding: 16px 18px;
    border: 2px solid var(--rand);
    border-radius: 12px;
    font-family: inherit;
    font-size: 0.95rem;
    line-height: 1.6;
    color: var(--text);
    background: var(--weiss);
    resize: vertical;
    box-sizing: border-box;
  }
  textarea:focus { outline: none; border-color: var(--akzent); }

  .fuss {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-top: 10px;
  }
  .zeichen { color: var(--text-leise); font-size: 0.8rem; }
  .hinweis { color: var(--text-leise); font-size: 0.8rem; }

  .erklaerung {
    margin: 12px 0 0;
    color: var(--text-muted);
    font-size: 0.86rem;
    line-height: 1.5;
  }
  .vorlage-info p {
    margin: 0 0 10px;
    color: var(--text-2);
    font-size: 0.9rem;
    line-height: 1.5;
  }
  .vorlage-text {
    max-height: 340px;
    overflow-y: auto;
    padding: 14px 16px;
    border: 1px solid var(--rand);
    border-radius: 10px;
    background: var(--flaeche-b);
    color: var(--text-2);
    font-size: 0.92rem;
    line-height: 1.6;
    white-space: pre-wrap;
    overflow-wrap: anywhere;
  }
  .vorlage-knoepfe {
    display: flex;
    align-items: center;
    gap: 12px;
    flex-wrap: wrap;
    margin-top: 14px;
  }
  .vergleich { margin-top: 12px; }
  .vergleich summary {
    cursor: pointer;
    color: var(--text-muted);
    font-size: 0.86rem;
    margin-bottom: 8px;
  }

  button.primaer {
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
  button.primaer:hover:not(:disabled) { background: var(--akzent-d); }
  button.primaer:disabled { background: var(--grau-3); cursor: default; }
  button.leise {
    background: none;
    border: none;
    color: var(--text-muted);
    font-size: 0.86rem;
    font-family: inherit;
    padding: 6px;
    cursor: pointer;
  }
  button.leise:hover:not(:disabled) { color: var(--text); text-decoration: underline; }
  button.leise.rechts { align-self: flex-end; margin-top: 8px; }
</style>
