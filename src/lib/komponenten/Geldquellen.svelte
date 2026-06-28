<script>
  // Abrechnungs-Modus, Phase A4a: Geldquellen verwalten (Förderer +
  // Eigenmittel/Einnahmen) mit ihrem Soll-Betrag. Tresor-Inhalt.
  import { fade } from "svelte/transition";
  import {
    QUELLE_TYP,
    neueQuelle,
    quelleSoll,
    quellenAusFinanzierung,
    zugeordnetJeQuelle,
    betragFormat,
    betragParsen,
  } from "$lib/abrechnung";

  let {
    quellen = [],
    speichern,
    entfernen, // (quelleId) – räumt auch die Zuordnungen in den Belegen auf
    kfp = { kosten: [], finanzierung: [] },
    belege = [],
    projektName = "",
  } = $props();

  let liste = $state(structuredClone($state.snapshot(quellen)));
  let beschaeftigt = $state(false);

  let formOffen = $state(false);
  let bearbeiteId = $state(null);
  let form = $state(null);
  let formFehler = $state("");

  let zugeordnet = $derived(zugeordnetJeQuelle(belege));
  let sollGesamt = $derived(liste.reduce((s, q) => s + quelleSoll(q), 0));
  let zugeordnetGesamt = $derived(
    liste.reduce((s, q) => s + (zugeordnet.get(q.id) ?? 0), 0)
  );

  async function sichern() {
    beschaeftigt = true;
    try {
      await speichern($state.snapshot(liste));
    } finally {
      beschaeftigt = false;
    }
  }

  function neu(typ) {
    bearbeiteId = null;
    form = neueQuelle(typ);
    formFehler = "";
    formOffen = true;
  }
  function bearbeiten(q) {
    bearbeiteId = q.id;
    form = structuredClone($state.snapshot(q));
    formFehler = "";
    formOffen = true;
  }
  function abbrechen() {
    formOffen = false;
    form = null;
    bearbeiteId = null;
    formFehler = "";
  }
  async function formSpeichern() {
    if (!form.name.trim()) {
      formFehler = "Bitte einen Namen angeben.";
      return;
    }
    const eintrag = structuredClone($state.snapshot(form));
    eintrag.name = eintrag.name.trim();
    if (bearbeiteId) {
      const i = liste.findIndex((x) => x.id === bearbeiteId);
      if (i >= 0) liste[i] = eintrag;
    } else {
      liste.push(eintrag);
    }
    await sichern();
    abbrechen();
  }

  async function quelleLoeschen(q) {
    const benutzt = (zugeordnet.get(q.id) ?? 0) > 0;
    const text = benutzt
      ? `„${q.name}" löschen? Die bereits zugeordneten Beträge (${betragFormat(zugeordnet.get(q.id))}) werden aus den Belegen entfernt.`
      : `„${q.name}" löschen?`;
    if (!confirm(text)) return;
    beschaeftigt = true;
    try {
      await entfernen(q.id);
      liste = liste.filter((x) => x.id !== q.id);
    } finally {
      beschaeftigt = false;
    }
  }

  async function ausKostenplan() {
    const neuQ = quellenAusFinanzierung(kfp, liste);
    if (!neuQ.length) {
      alert("Keine neuen Geldquellen im Kostenplan gefunden (alle schon vorhanden).");
      return;
    }
    liste.push(...neuQ);
    await sichern();
  }
</script>

<div class="bereich">
  <div class="kopfzeile">
    <div class="titel-block">
      <h2>Geldquellen</h2>
      <p class="untertitel">
        Förderer und Eigenmittel{#if projektName} für <strong>{projektName}</strong>{/if}, denen du
        später Belege zuordnest. Das <strong>Soll</strong> ist der bewilligte bzw. erwartete Betrag.
      </p>
    </div>
    <div class="knoepfe">
      <button class="zweit" onclick={() => neu("foerderung")}>+ Förderer</button>
      <button class="zweit" onclick={() => neu("eigenmittel")}>+ Eigenmittel</button>
      <button class="leise" onclick={ausKostenplan} disabled={beschaeftigt}>aus Kostenplan übernehmen</button>
    </div>
  </div>

  {#if liste.length === 0}
    <div class="leer">
      <p>Noch keine Geldquellen.</p>
      <p class="dezent">
        Lege Förderer/Eigenmittel an – oder übernimm sie aus dem Finanzierungsplan des Kostenplans.
      </p>
    </div>
  {:else}
    <table class="quellen">
      <thead>
        <tr>
          <th>Name</th>
          <th>Art</th>
          <th class="betrag">Soll</th>
          <th class="betrag">Zugeordnet</th>
          <th class="betrag">Rest</th>
          <th class="akt"></th>
        </tr>
      </thead>
      <tbody>
        {#each liste as q (q.id)}
          {@const zu = zugeordnet.get(q.id) ?? 0}
          {@const rest = quelleSoll(q) - zu}
          <tr>
            <td class="name">{q.name}</td>
            <td><span class="typ t-{q.typ}">{QUELLE_TYP[q.typ] || q.typ}</span></td>
            <td class="betrag">{betragFormat(quelleSoll(q))}</td>
            <td class="betrag">{betragFormat(zu)}</td>
            <td class="betrag" class:ueber={rest < -0.005}>{betragFormat(rest)}</td>
            <td class="akt">
              <button class="leise" onclick={() => bearbeiten(q)}>bearbeiten</button>
              <button class="leise gefahr" onclick={() => quelleLoeschen(q)} disabled={beschaeftigt}>löschen</button>
            </td>
          </tr>
        {/each}
      </tbody>
      <tfoot>
        <tr>
          <td colspan="2" class="summe-label">Gesamt</td>
          <td class="betrag summe">{betragFormat(sollGesamt)}</td>
          <td class="betrag summe">{betragFormat(zugeordnetGesamt)}</td>
          <td class="betrag summe" class:ueber={sollGesamt - zugeordnetGesamt < -0.005}>
            {betragFormat(sollGesamt - zugeordnetGesamt)}
          </td>
          <td></td>
        </tr>
      </tfoot>
    </table>
  {/if}
</div>

{#if formOffen && form}
  <div class="overlay" transition:fade={{ duration: 120 }}>
    <div class="dialog">
      <h2>{bearbeiteId ? "Geldquelle bearbeiten" : "Neue Geldquelle"}</h2>
      <div class="gitter">
        <label class="feld breit">
          <span>Name</span>
          <input type="text" bind:value={form.name} placeholder="z. B. Stadt Musterstadt / Ticketeinnahmen" />
        </label>
        <label class="feld">
          <span>Art</span>
          <select bind:value={form.typ}>
            {#each Object.entries(QUELLE_TYP) as [wert, text]}
              <option value={wert}>{text}</option>
            {/each}
          </select>
        </label>
        <label class="feld">
          <span>Soll (bewilligt)</span>
          <input type="text" bind:value={form.soll} placeholder="z. B. 5.000" inputmode="decimal" />
        </label>
        <label class="feld breit">
          <span>Sachbericht <em>(optional, für den Verwendungsnachweis)</em></span>
          <textarea rows="3" bind:value={form.sachbericht} placeholder="Kurzer Bericht zur Mittelverwendung"></textarea>
        </label>
      </div>
      {#if betragParsen(form.soll) > 0}
        <p class="rechen">Soll: {betragFormat(betragParsen(form.soll))}</p>
      {/if}
      {#if formFehler}<p class="fehler">{formFehler}</p>{/if}
      <div class="dialog-knoepfe">
        <button class="zweit" onclick={abbrechen} disabled={beschaeftigt}>Abbrechen</button>
        <button class="primaer" onclick={formSpeichern} disabled={beschaeftigt}>
          {beschaeftigt ? "Speichert …" : "Speichern"}
        </button>
      </div>
    </div>
  </div>
{/if}

<style>
  .bereich {
    max-width: 920px;
    margin: 0 auto;
    padding: 32px 24px 64px;
  }
  .kopfzeile {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 16px;
    flex-wrap: wrap;
    margin-bottom: 20px;
  }
  .titel-block {
    flex: 1 1 300px;
    min-width: 260px;
  }
  h2 {
    margin: 0 0 4px;
    font-size: 1.35rem;
    font-weight: 600;
    color: #172b4d;
  }
  .untertitel {
    margin: 0;
    color: #5e6c84;
    font-size: 0.9rem;
    max-width: 520px;
    line-height: 1.5;
  }
  .knoepfe {
    display: flex;
    align-items: center;
    gap: 8px;
    flex-wrap: wrap;
  }

  .leer {
    text-align: center;
    color: #5e6c84;
    padding: 44px 16px;
    border: 1px dashed #dfe1e6;
    border-radius: 12px;
  }
  .leer p {
    margin: 0 0 8px;
  }
  .dezent {
    font-size: 0.88rem;
  }

  table.quellen {
    width: 100%;
    border-collapse: collapse;
    font-size: 0.9rem;
  }
  table.quellen th,
  table.quellen td {
    text-align: left;
    padding: 9px 10px;
    border-bottom: 1px solid #ebedf0;
  }
  table.quellen th {
    color: #5e6c84;
    font-weight: 600;
    font-size: 0.78rem;
    text-transform: uppercase;
    letter-spacing: 0.03em;
  }
  .name {
    color: #172b4d;
    font-weight: 500;
  }
  .betrag {
    text-align: right;
    white-space: nowrap;
    font-variant-numeric: tabular-nums;
  }
  .betrag.ueber {
    color: #ca3521;
    font-weight: 700;
  }
  .summe-label {
    text-align: right;
    font-weight: 600;
    color: #44546f;
  }
  .summe {
    font-weight: 700;
    color: #172b4d;
  }
  .akt {
    text-align: right;
    white-space: nowrap;
  }
  .typ {
    display: inline-block;
    padding: 2px 9px;
    border-radius: 99px;
    font-size: 0.74rem;
    font-weight: 600;
  }
  .typ.t-foerderung {
    background: #eef1ff;
    color: #3b4fb0;
  }
  .typ.t-eigenmittel {
    background: #dcfff1;
    color: #14794e;
  }

  button.primaer {
    padding: 10px 18px;
    font-size: 0.92rem;
    font-weight: 600;
    font-family: inherit;
    color: #fff;
    background: #4f6df5;
    border: none;
    border-radius: 8px;
    cursor: pointer;
  }
  button.primaer:hover:not(:disabled) {
    background: #3d5bf0;
  }
  button.primaer:disabled {
    background: #c1c7d0;
    cursor: default;
  }
  button.zweit {
    padding: 9px 14px;
    font-size: 0.9rem;
    font-weight: 600;
    font-family: inherit;
    color: #172b4d;
    background: #fff;
    border: 2px solid #dfe1e6;
    border-radius: 8px;
    cursor: pointer;
  }
  button.zweit:hover:not(:disabled) {
    border-color: #4f6df5;
  }
  button.leise {
    background: none;
    border: none;
    color: #5e6c84;
    font-size: 0.84rem;
    cursor: pointer;
    padding: 4px 6px;
    font-family: inherit;
  }
  button.leise:hover {
    color: #172b4d;
    text-decoration: underline;
  }
  button.leise.gefahr:hover {
    color: #ae2e24;
  }

  .overlay {
    position: fixed;
    inset: 0;
    background: rgba(9, 30, 66, 0.45);
    display: flex;
    align-items: flex-start;
    justify-content: center;
    padding: 6vh 16px;
    overflow-y: auto;
    z-index: 50;
  }
  .dialog {
    background: #fff;
    border-radius: 14px;
    padding: 24px;
    width: 100%;
    max-width: 540px;
    box-shadow: 0 12px 40px rgba(9, 30, 66, 0.3);
  }
  .dialog h2 {
    font-size: 1.2rem;
    margin: 0 0 16px;
  }
  .gitter {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 12px 14px;
  }
  .feld {
    display: flex;
    flex-direction: column;
    gap: 4px;
    font-size: 0.85rem;
  }
  .feld.breit {
    grid-column: 1 / -1;
  }
  .feld > span {
    color: #44546f;
    font-weight: 600;
  }
  .feld em {
    color: #8590a2;
    font-weight: 400;
    font-style: normal;
  }
  .feld input,
  .feld select,
  .feld textarea {
    padding: 8px 10px;
    border: 2px solid #dfe1e6;
    border-radius: 8px;
    font-family: inherit;
    font-size: 0.9rem;
    color: #172b4d;
    background: #fff;
  }
  .feld input:focus,
  .feld select:focus,
  .feld textarea:focus {
    outline: none;
    border-color: #4f6df5;
  }
  .rechen {
    margin: 14px 0 0;
    color: #5e6c84;
    font-size: 0.84rem;
  }
  .fehler {
    margin: 12px 0 0;
    color: #ae2e24;
    font-size: 0.86rem;
  }
  .dialog-knoepfe {
    display: flex;
    justify-content: flex-end;
    gap: 10px;
    margin-top: 20px;
  }
</style>
