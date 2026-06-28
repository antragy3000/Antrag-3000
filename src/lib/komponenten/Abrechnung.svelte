<script>
  // Abrechnungs-Modus, Phase A1: Belege laufend erfassen.
  // Tresor-Inhalt (Betraege, Lieferanten sind sensibel) – bleibt lokal.
  // Kostenstellen-Verknuepfung, Dateien und Foerderer-Zuordnung folgen in
  // spaeteren Phasen; die Felder existieren im Datenmodell bereits.
  import { fade } from "svelte/transition";
  import {
    ZAHLUNGSARTEN,
    BELEG_STATUS,
    neuerBeleg,
    naechsteBelegNr,
    belegBrutto,
    belegNetto,
    belegMwstBetrag,
    mwstSatz,
    belegeSumme,
    betragFormat,
    datumText,
    groesseText,
    kostenstellenNachKategorie,
    kostenstelleLabel,
    belegNummern,
  } from "$lib/abrechnung";

  let {
    belege = [],
    speichern,
    projektName = "",
    // KFP für die Kostenstellen-Auswahl (Phase A3): nur lesen, plus ein
    // Callback zum Anlegen eines neuen Kosten-Postens.
    kfp = { kosten: [], finanzierung: [] },
    kostenstelleAnlegen,
    // Beleg-Dateien (Phase A2) – kommen als Callbacks aus +page.svelte,
    // die das Rust-Backend (Verschlüsseln/Ablegen/Öffnen/Löschen) aufrufen.
    dateiHinzufuegen,
    dateiOeffnen,
    dateiHerunterladen,
    dateiEntfernen,
    ordnerEntfernen,
  } = $props();

  // Lokale Arbeitskopie; jede Aenderung wird sofort verschluesselt
  // gesichert (wie ein laufendes Kassenbuch). Quelle der Wahrheit bleibt
  // der Tresor – wir schicken die volle Liste per speichern() hoch.
  let liste = $state(structuredClone($state.snapshot(belege)));
  let beschaeftigt = $state(false);

  // Formular-Zustand (Hinzufuegen/Bearbeiten in einem Panel).
  let formOffen = $state(false);
  let bearbeiteId = $state(null); // null = neuer Beleg
  let form = $state(null);
  let formFehler = $state("");

  // Kostenstellen-Auswahl im Formular (nach Kategorie gruppiert) + das
  // Anlegen einer neuen Kostenstelle direkt aus der Beleg-Maske.
  let kategorien = $derived(kostenstellenNachKategorie(kfp));
  let neuKsOffen = $state(false);
  let neuKsKategorie = $state(0);
  let neuKsBezeichnung = $state("");

  // Welcher Beleg hat gerade sein Datei-Panel offen?
  let dateienOffenId = $state(null);
  let dateiBeschaeftigt = $state(false);
  let dateienBeleg = $derived(liste.find((b) => b.id === dateienOffenId) ?? null);

  let summe = $derived(belegeSumme(liste));

  // Belegnummer je Kostenstelle (z. B. "3.1.1"); Belege ohne Kostenstelle
  // haben keine – dort zeigen wir die laufende Roh-Nummer als "#7".
  let nummern = $derived(belegNummern(liste, kfp));
  const anzeigeNr = (b) => nummern.get(b.id) ?? `#${b.nr}`;

  async function sichern() {
    beschaeftigt = true;
    try {
      await speichern($state.snapshot(liste));
    } finally {
      beschaeftigt = false;
    }
  }

  function neuKsZuruecksetzen() {
    neuKsOffen = false;
    neuKsKategorie = 0;
    neuKsBezeichnung = "";
  }

  function neu() {
    bearbeiteId = null;
    form = neuerBeleg(naechsteBelegNr(liste));
    formFehler = "";
    neuKsZuruecksetzen();
    formOffen = true;
  }

  function bearbeiten(b) {
    bearbeiteId = b.id;
    form = structuredClone($state.snapshot(b));
    formFehler = "";
    neuKsZuruecksetzen();
    formOffen = true;
  }

  function abbrechen() {
    formOffen = false;
    form = null;
    bearbeiteId = null;
    formFehler = "";
    neuKsZuruecksetzen();
  }

  // Neue Kostenstelle (KFP-Kosten-Posten) direkt aus der Beleg-Maske
  // anlegen und gleich auswählen.
  async function neueKostenstelle() {
    const bez = neuKsBezeichnung.trim();
    if (!bez) return;
    const id = await kostenstelleAnlegen(neuKsKategorie, bez);
    if (id) form.kostenstelle = id;
    neuKsZuruecksetzen();
  }

  async function formSpeichern() {
    // Minimal-Pruefung: Datum und ein Betrag > 0 muessen sein.
    if (!form.datum) {
      formFehler = "Bitte ein Datum angeben.";
      return;
    }
    if (belegBrutto(form) <= 0) {
      formFehler = "Bitte einen Betrag größer als 0 angeben.";
      return;
    }
    const eintrag = structuredClone($state.snapshot(form));
    if (bearbeiteId) {
      const i = liste.findIndex((x) => x.id === bearbeiteId);
      if (i >= 0) liste[i] = eintrag;
    } else {
      liste.push(eintrag);
    }
    await sichern();
    abbrechen();
  }

  async function entfernen(b) {
    if (!confirm(`Beleg ${anzeigeNr(b)} wirklich löschen?`)) return;
    // Erst die (verschlüsselten) Dateien des Belegs entfernen, dann den Beleg.
    if (b.dateien?.length) await ordnerEntfernen(b.id);
    if (dateienOffenId === b.id) dateienOffenId = null;
    liste = liste.filter((x) => x.id !== b.id);
    await sichern();
  }

  // --- Beleg-Dateien (Phase A2) ---
  function dateienUmschalten(b) {
    dateienOffenId = dateienOffenId === b.id ? null : b.id;
  }

  async function dateiAnhaengen(b) {
    if (dateiBeschaeftigt) return;
    dateiBeschaeftigt = true;
    try {
      const d = await dateiHinzufuegen(b.id);
      if (d) {
        b.dateien = [...(b.dateien ?? []), d];
        await sichern();
      }
    } finally {
      dateiBeschaeftigt = false;
    }
  }

  async function dateiAnsehen(b, d) {
    await dateiOeffnen(b.id, d.ref, d.name);
  }

  async function dateiSpeichernUnter(b, d) {
    await dateiHerunterladen(b.id, d.ref, d.name);
  }

  async function dateiLoeschen(b, d) {
    if (!confirm(`Datei „${d.name}" löschen?`)) return;
    dateiBeschaeftigt = true;
    try {
      await dateiEntfernen(b.id, d.ref);
      b.dateien = (b.dateien ?? []).filter((x) => x.ref !== d.ref);
      await sichern();
    } finally {
      dateiBeschaeftigt = false;
    }
  }

  // Belege nach Datum sortiert (neueste zuletzt), Nummer als Zweitschluessel.
  let sortiert = $derived(
    [...liste].sort((a, b) => {
      const d = String(a.datum).localeCompare(String(b.datum));
      return d !== 0 ? d : Number(a.nr) - Number(b.nr);
    })
  );
</script>

<div class="bereich">
  <div class="kopfzeile">
    <div class="titel-block">
      <h2>Abrechnung – Belege</h2>
      <p class="untertitel">
        Erfasse hier laufend deine Belege{#if projektName}
          für <strong>{projektName}</strong>{/if}. Später ordnest du sie den
        Förderern zu. Alle Angaben bleiben lokal verschlüsselt auf deinem Gerät.
      </p>
    </div>
    <button class="primaer" onclick={neu} disabled={beschaeftigt}>+ Neuer Beleg</button>
  </div>

  {#if liste.length === 0}
    <div class="leer">
      <p>Noch keine Belege erfasst.</p>
      <button class="zweit" onclick={neu}>Ersten Beleg erfassen</button>
    </div>
  {:else}
    <table class="belege">
      <thead>
        <tr>
          <th class="num">Nr.</th>
          <th>Datum</th>
          <th>Empfänger</th>
          <th>Zweck</th>
          <th>Kostenstelle</th>
          <th class="betrag">Betrag</th>
          <th>Zahlung</th>
          <th>Status</th>
          <th class="akt"></th>
        </tr>
      </thead>
      <tbody>
        {#each sortiert as b (b.id)}
          <tr>
            <td class="num">
              {#if nummern.get(b.id)}{nummern.get(b.id)}{:else}<span class="roh-nr">#{b.nr}</span>{/if}
            </td>
            <td>{datumText(b.datum)}</td>
            <td>{b.empfaenger || "—"}</td>
            <td class="zweck">{b.zweck || "—"}</td>
            <td class="ks">{kostenstelleLabel(kfp, b.kostenstelle) || "—"}</td>
            <td class="betrag">
              {betragFormat(belegBrutto(b))}
              {#if mwstSatz(b) > 0}<div class="mwst">inkl. {mwstSatz(b)} % MwSt</div>{/if}
            </td>
            <td>{ZAHLUNGSARTEN[b.zahlungsart] || "—"}</td>
            <td><span class="status s-{b.status}">{BELEG_STATUS[b.status] || b.status}</span></td>
            <td class="akt">
              <button class="leise" class:aktiv={dateienOffenId === b.id} onclick={() => dateienUmschalten(b)} title="Dateien zum Beleg">
                📎 {b.dateien?.length || 0}
              </button>
              <button class="leise" onclick={() => bearbeiten(b)}>bearbeiten</button>
              <button class="leise gefahr" onclick={() => entfernen(b)}>löschen</button>
            </td>
          </tr>
        {/each}
      </tbody>
      <tfoot>
        <tr>
          <td colspan="5" class="summe-label">Summe ({liste.length} Belege)</td>
          <td class="betrag summe">{betragFormat(summe)}</td>
          <td colspan="3"></td>
        </tr>
      </tfoot>
    </table>
  {/if}

  {#if dateienBeleg}
    <div class="datei-panel" transition:fade={{ duration: 120 }}>
      <div class="dp-kopf">
        <h3>
          📎 Dateien zu Beleg {anzeigeNr(dateienBeleg)}{#if dateienBeleg.empfaenger}
            – {dateienBeleg.empfaenger}{/if}
        </h3>
        <button class="leise" onclick={() => (dateienOffenId = null)}>schließen</button>
      </div>

      {#if dateienBeleg.dateien?.length}
        <ul class="datei-liste">
          {#each dateienBeleg.dateien as d (d.ref)}
            <li>
              <span class="dn" title={d.name}>{d.name}</span>
              <span class="dg">{groesseText(d.groesse)}</span>
              <button class="leise" onclick={() => dateiAnsehen(dateienBeleg, d)}>ansehen</button>
              <button class="leise" onclick={() => dateiSpeichernUnter(dateienBeleg, d)}>herunterladen</button>
              <button
                class="leise gefahr"
                onclick={() => dateiLoeschen(dateienBeleg, d)}
                disabled={dateiBeschaeftigt}>löschen</button
              >
            </li>
          {/each}
        </ul>
      {:else}
        <p class="dp-leer">Noch keine Dateien. Lade einen Scan oder ein Foto des Belegs hoch.</p>
      {/if}

      <button class="zweit" onclick={() => dateiAnhaengen(dateienBeleg)} disabled={dateiBeschaeftigt}>
        {dateiBeschaeftigt ? "Bitte warten …" : "+ Datei hinzufügen"}
      </button>
      <p class="dp-hinweis">
        Dateien werden <strong>verschlüsselt</strong> im Projektordner gespeichert
        (PDF, JPG oder PNG, max. 30 MB). Sie verlassen dein Gerät nie.
      </p>
    </div>
  {/if}
</div>

{#if formOffen && form}
  <div class="overlay" transition:fade={{ duration: 120 }}>
    <div class="dialog">
      <h2>{bearbeiteId ? "Beleg bearbeiten" : "Neuer Beleg"} <span class="nr">{anzeigeNr(form)}</span></h2>

      <div class="gitter">
        <label class="feld">
          <span>Datum</span>
          <input type="date" bind:value={form.datum} />
        </label>
        <label class="feld">
          <span>Betrag (brutto)</span>
          <input type="text" bind:value={form.brutto} placeholder="z. B. 1.234,56" inputmode="decimal" />
        </label>
        <label class="feld">
          <span>MwSt-Satz % <em>(optional)</em></span>
          <input type="text" bind:value={form.mwst_satz} placeholder="z. B. 19" inputmode="decimal" />
        </label>
        <label class="feld">
          <span>Zahlungsart</span>
          <select bind:value={form.zahlungsart}>
            <option value="">—</option>
            {#each Object.entries(ZAHLUNGSARTEN) as [wert, text]}
              <option value={wert}>{text}</option>
            {/each}
          </select>
        </label>
        <label class="feld breit">
          <span>Empfänger / Lieferant</span>
          <input type="text" bind:value={form.empfaenger} placeholder="An wen wurde gezahlt?" />
        </label>
        <label class="feld breit">
          <span>Zweck / Beschreibung</span>
          <input type="text" bind:value={form.zweck} placeholder="Wofür? (kurz)" />
        </label>
        <div class="feld breit">
          <span>Kostenstelle <em>(optional)</em></span>
          <select bind:value={form.kostenstelle}>
            <option value={null}>— keine —</option>
            {#each kategorien as kat}
              {#if kat.posten.length}
                <optgroup label={kat.name}>
                  {#each kat.posten as p}
                    <option value={p.id}>{p.nummer} {p.bezeichnung}</option>
                  {/each}
                </optgroup>
              {/if}
            {/each}
          </select>
          {#if !neuKsOffen}
            <div class="ks-zeile">
              <button
                type="button"
                class="leise"
                onclick={() => (neuKsOffen = true)}
                disabled={!kfp.kosten?.length}>＋ neue Kostenstelle</button
              >
              {#if !kfp.kosten?.length}
                <span class="ks-hinweis">Lege zuerst im Kostenplan eine Kategorie an.</span>
              {/if}
            </div>
          {:else}
            <div class="ks-neu">
              <select bind:value={neuKsKategorie} aria-label="Kategorie">
                {#each kfp.kosten as k, i}
                  <option value={i}>{k.name || "(ohne Name)"}</option>
                {/each}
              </select>
              <input
                type="text"
                bind:value={neuKsBezeichnung}
                placeholder="Bezeichnung, z. B. Honorar Regie"
              />
              <button type="button" class="zweit schmal" onclick={neueKostenstelle}>anlegen</button>
              <button type="button" class="leise" onclick={neuKsZuruecksetzen}>abbrechen</button>
            </div>
          {/if}
        </div>
        <label class="feld">
          <span>Status</span>
          <select bind:value={form.status}>
            {#each Object.entries(BELEG_STATUS) as [wert, text]}
              <option value={wert}>{text}</option>
            {/each}
          </select>
        </label>
        <label class="feld breit">
          <span>Notiz <em>(optional)</em></span>
          <textarea rows="2" bind:value={form.notiz} placeholder="Interne Notiz"></textarea>
        </label>
      </div>

      {#if mwstSatz(form) > 0 && belegBrutto(form) > 0}
        <p class="rechen">
          Netto {betragFormat(belegNetto(form))} · MwSt {betragFormat(belegMwstBetrag(form))} ·
          Brutto {betragFormat(belegBrutto(form))}
        </p>
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

  .leer {
    text-align: center;
    color: #5e6c84;
    padding: 48px 16px;
    border: 1px dashed #dfe1e6;
    border-radius: 12px;
  }
  .leer p {
    margin: 0 0 14px;
  }

  table.belege {
    width: 100%;
    border-collapse: collapse;
    font-size: 0.9rem;
  }
  table.belege th,
  table.belege td {
    text-align: left;
    padding: 9px 10px;
    border-bottom: 1px solid #ebedf0;
    vertical-align: top;
  }
  table.belege th {
    color: #5e6c84;
    font-weight: 600;
    font-size: 0.8rem;
    text-transform: uppercase;
    letter-spacing: 0.03em;
  }
  .num {
    width: 60px;
    color: #44546f;
    font-variant-numeric: tabular-nums;
    white-space: nowrap;
  }
  .roh-nr {
    color: #b3bac5;
  }
  .betrag {
    text-align: right;
    white-space: nowrap;
    font-variant-numeric: tabular-nums;
  }
  .zweck {
    color: #44546f;
  }
  .mwst {
    font-size: 0.72rem;
    color: #8590a2;
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

  .status {
    display: inline-block;
    padding: 2px 9px;
    border-radius: 99px;
    font-size: 0.74rem;
    font-weight: 600;
    background: #eef1ff;
    color: #3b4fb0;
  }
  .status.s-zugeordnet {
    background: #fff7d6;
    color: #7a5b00;
  }
  .status.s-abgerechnet {
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
    padding: 10px 18px;
    font-size: 0.92rem;
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

  /* Dialog */
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
    max-width: 560px;
    box-shadow: 0 12px 40px rgba(9, 30, 66, 0.3);
  }
  .dialog h2 {
    font-size: 1.2rem;
    margin: 0 0 16px;
  }
  .dialog h2 .nr {
    color: #8590a2;
    font-weight: 500;
    font-size: 0.9rem;
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
    font-variant-numeric: tabular-nums;
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

  button.leise.aktiv {
    color: #3d5bf0;
    font-weight: 700;
  }

  td.ks {
    color: #44546f;
    font-size: 0.84rem;
  }

  /* Kostenstelle im Formular */
  .ks-zeile {
    display: flex;
    align-items: center;
    gap: 10px;
    margin-top: 2px;
  }
  .ks-zeile button.leise {
    padding: 2px 0;
  }
  .ks-hinweis {
    color: #8590a2;
    font-size: 0.78rem;
  }
  .ks-neu {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 8px;
    margin-top: 6px;
  }
  .ks-neu select {
    flex: 0 0 auto;
    max-width: 45%;
  }
  .ks-neu input {
    flex: 1 1 160px;
  }
  button.zweit.schmal {
    padding: 7px 12px;
    font-size: 0.85rem;
  }

  /* Datei-Panel */
  .datei-panel {
    margin-top: 18px;
    border: 1px solid #dfe1e6;
    border-radius: 12px;
    padding: 16px 18px;
    background: #f7f8fa;
  }
  .dp-kopf {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    margin-bottom: 10px;
  }
  .dp-kopf h3 {
    margin: 0;
    font-size: 1rem;
    color: #172b4d;
  }
  .datei-liste {
    list-style: none;
    margin: 0 0 12px;
    padding: 0;
  }
  .datei-liste li {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 7px 0;
    border-bottom: 1px solid #ebedf0;
  }
  .datei-liste .dn {
    flex: 1;
    color: #172b4d;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .datei-liste .dg {
    color: #8590a2;
    font-size: 0.8rem;
    font-variant-numeric: tabular-nums;
    white-space: nowrap;
  }
  .dp-leer {
    color: #5e6c84;
    font-size: 0.88rem;
    margin: 0 0 12px;
  }
  .dp-hinweis {
    margin: 12px 0 0;
    color: #5e6c84;
    font-size: 0.8rem;
    line-height: 1.5;
  }
</style>
