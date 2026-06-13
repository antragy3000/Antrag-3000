<script>
  // Formular für eine selbst recherchierte Förderung. Wird direkt auf
  // die Merkliste gesetzt und liegt verschlüsselt im Projekt.
  let { anlegen, schliessen } = $props();

  let name = $state("");
  let foerdergeber = $state("");
  let land = $state("ANDERES");
  let webseite = $state("");
  let foerderhoehe = $state("");
  let beschreibung = $state("");
  let laufend = $state(false);
  let frist = $state("");
  let beschaeftigt = $state(false);

  async function speichern(event) {
    event.preventDefault();
    if (!name.trim()) return;
    beschaeftigt = true;
    try {
      await anlegen({
        name,
        foerdergeber,
        land,
        webseite,
        foerderhoehe,
        beschreibung,
        laufend,
        frist: laufend ? "" : frist,
      });
      schliessen();
    } finally {
      beschaeftigt = false;
    }
  }
</script>

<div class="schleier" onclick={schliessen} role="presentation">
  <form class="dialog" onsubmit={speichern} onclick={(e) => e.stopPropagation()}>
    <h2>Eigene Förderung</h2>
    <p class="untertitel">
      Für selbst recherchierte Förderungen, die nicht in der Datenbank stehen.
      Sie wird direkt auf deine Merkliste gesetzt.
    </p>

    <label for="ef-name">Name der Förderung *</label>
    <input id="ef-name" type="text" bind:value={name} />

    <label for="ef-geber">Fördergeber</label>
    <input id="ef-geber" type="text" bind:value={foerdergeber} />

    <div class="zwei">
      <div>
        <label for="ef-land">Land</label>
        <select id="ef-land" bind:value={land}>
          <option value="DE">Deutschland</option>
          <option value="AT">Österreich</option>
          <option value="CH">Schweiz</option>
          <option value="INT">International</option>
          <option value="ANDERES">anderes / unbestimmt</option>
        </select>
      </div>
      <div>
        <label for="ef-hoehe">Förderhöhe (Text)</label>
        <input id="ef-hoehe" type="text" placeholder="z. B. bis 10.000 €" bind:value={foerderhoehe} />
      </div>
    </div>

    <label for="ef-web">Webseite</label>
    <input id="ef-web" type="text" placeholder="https://…" bind:value={webseite} />

    <label class="check">
      <input type="checkbox" bind:checked={laufend} />
      Laufend einreichbar (keine feste Frist)
    </label>
    {#if !laufend}
      <label for="ef-frist">Einreichfrist</label>
      <input id="ef-frist" type="date" bind:value={frist} />
    {/if}

    <label for="ef-besch">Kurzbeschreibung</label>
    <textarea id="ef-besch" rows="3" bind:value={beschreibung}></textarea>

    <div class="knoepfe">
      <button type="button" class="leise" onclick={schliessen}>Abbrechen</button>
      <button type="submit" class="primaer" disabled={!name.trim() || beschaeftigt}>
        {beschaeftigt ? "Wird angelegt …" : "Anlegen & merken"}
      </button>
    </div>
  </form>
</div>

<style>
  .schleier {
    position: fixed;
    inset: 0;
    background: rgba(9, 30, 66, 0.45);
    display: grid;
    place-items: center;
    padding: 24px;
    z-index: 30;
  }
  .dialog {
    background: #fff;
    border-radius: 12px;
    padding: 32px;
    max-width: 480px;
    width: 100%;
    max-height: 85vh;
    overflow-y: auto;
    box-shadow: 0 12px 40px rgba(9, 30, 66, 0.3);
  }
  h2 {
    margin: 0 0 6px;
    font-size: 1.2rem;
  }
  .untertitel {
    margin: 0 0 8px;
    color: #5e6c84;
    font-size: 0.9rem;
    line-height: 1.5;
  }

  label {
    display: block;
    font-size: 0.82rem;
    font-weight: 600;
    color: #5e6c84;
    margin: 14px 0 5px;
  }
  label.check {
    display: flex;
    align-items: center;
    gap: 8px;
    font-weight: 400;
    color: #44546f;
    margin: 16px 0 0;
    cursor: pointer;
  }
  label.check input {
    width: auto;
  }

  input[type="text"],
  input[type="date"],
  select,
  textarea {
    width: 100%;
    box-sizing: border-box;
    padding: 9px 12px;
    font-size: 0.95rem;
    font-family: inherit;
    border: 2px solid #dfe1e6;
    border-radius: 8px;
    background: #fafbfc;
  }
  input:focus,
  select:focus,
  textarea:focus {
    outline: none;
    border-color: #4f6df5;
    background: #fff;
  }
  textarea {
    resize: vertical;
    line-height: 1.5;
  }

  .zwei {
    display: flex;
    gap: 12px;
  }
  .zwei > div {
    flex: 1;
    min-width: 0;
  }

  .knoepfe {
    display: flex;
    justify-content: flex-end;
    gap: 10px;
    margin-top: 22px;
  }
  .primaer {
    padding: 10px 20px;
    font-size: 0.95rem;
    font-weight: 600;
    font-family: inherit;
    color: #fff;
    background: #4f6df5;
    border: none;
    border-radius: 8px;
    cursor: pointer;
  }
  .primaer:hover:not(:disabled) {
    background: #3d5bf0;
  }
  .primaer:disabled {
    background: #c1c7d0;
    cursor: default;
  }
  .leise {
    padding: 10px 16px;
    background: none;
    border: none;
    color: #5e6c84;
    font-size: 0.9rem;
    font-family: inherit;
    cursor: pointer;
  }
  .leise:hover {
    color: #172b4d;
    text-decoration: underline;
  }
</style>
