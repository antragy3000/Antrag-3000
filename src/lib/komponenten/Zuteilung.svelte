<script>
  // Abrechnungs-Modus, Phase A4b: Zuteilung PRO FÖRDERER.
  // Übersicht = Liste der Geldquellen (wie die Merkliste) mit Fortschritt.
  // Klick auf eine Quelle öffnet ihre Abrechnung: die zugeordneten Belege
  // als Liste – Betrag bearbeiten, entfernen, weitere Belege hinzufügen.
  // Tresor-Inhalt (bleibt lokal).
  import {
    belegBrutto,
    belegZugeordnet,
    belegFrei,
    quelleSoll,
    zugeordnetJeQuelle,
    belegNummern,
    betragFormat,
    betragParsen,
    datumText,
    QUELLE_TYP,
  } from "$lib/abrechnung";

  let {
    belege = [],
    quellen = [],
    speichern, // (neueBelege) – speichert die geänderten Zuordnungen
    kfp = { kosten: [], finanzierung: [] },
    projektName = "",
    // Verwendungsnachweis-Export (Phase A5).
    nachweisPdf,
    nachweisWord,
  } = $props();

  let liste = $state(structuredClone($state.snapshot(belege)));
  let beschaeftigt = $state(false);

  let ausgewaehltId = $state(null);
  let zuordnenOffen = $state(false);

  let nummern = $derived(belegNummern(liste, kfp));
  const anzeigeNr = (b) => nummern.get(b.id) ?? `#${b.nr}`;
  let zugeordnetQ = $derived(zugeordnetJeQuelle(liste));

  let ausgewaehlt = $derived(quellen.find((q) => q.id === ausgewaehltId) ?? null);

  // Kennzahlen je Quelle.
  function info(q) {
    const soll = quelleSoll(q);
    const zu = zugeordnetQ.get(q.id) ?? 0;
    const anzahl = liste.filter((b) => (b.zuordnungen ?? []).some((z) => z.quelleId === q.id)).length;
    return { soll, zu, rest: soll - zu, anzahl, anteil: soll > 0 ? Math.min(100, (zu / soll) * 100) : zu > 0 ? 100 : 0 };
  }

  // Warnungen (für die Statusleiste oben).
  let belegeUeberzogen = $derived(liste.filter((b) => belegFrei(b) < -0.005).length);
  let belegeOhne = $derived(liste.filter((b) => belegBrutto(b) > 0 && belegZugeordnet(b) < 0.005).length);
  let quellenUeberzogen = $derived(quellen.filter((q) => quelleSoll(q) - (zugeordnetQ.get(q.id) ?? 0) < -0.005).length);
  let allesOk = $derived(belegeUeberzogen === 0 && belegeOhne === 0 && quellenUeberzogen === 0);

  // Belege, die der ausgewählten Quelle zugeordnet sind (mit Datum sortiert).
  let zugeordneteBelege = $derived(
    !ausgewaehlt
      ? []
      : [...liste]
          .filter((b) => (b.zuordnungen ?? []).some((z) => z.quelleId === ausgewaehltId))
          .sort((a, b) => String(a.datum ?? "").localeCompare(String(b.datum ?? "")))
  );
  // Belege, die noch freien Betrag haben und dieser Quelle NICHT zugeordnet sind.
  let verfuegbareBelege = $derived(
    !ausgewaehlt
      ? []
      : [...liste]
          .filter(
            (b) =>
              belegFrei(b) > 0.005 && !(b.zuordnungen ?? []).some((z) => z.quelleId === ausgewaehltId)
          )
          .sort((a, b) => String(a.datum ?? "").localeCompare(String(b.datum ?? "")))
  );

  async function sichern() {
    beschaeftigt = true;
    try {
      await speichern($state.snapshot(liste));
    } finally {
      beschaeftigt = false;
    }
  }

  function zuordnungBetrag(b, qId) {
    const z = (b.zuordnungen ?? []).find((x) => x.quelleId === qId);
    return z ? z.betrag : "";
  }

  async function setzeZuordnung(b, qId, roh) {
    const wert = String(roh ?? "").trim();
    const zahl = betragParsen(wert);
    if (!Array.isArray(b.zuordnungen)) b.zuordnungen = [];
    const i = b.zuordnungen.findIndex((z) => z.quelleId === qId);
    if (!wert || zahl === 0) {
      if (i >= 0) b.zuordnungen.splice(i, 1);
    } else if (i >= 0) {
      b.zuordnungen[i].betrag = wert;
    } else {
      b.zuordnungen.push({ quelleId: qId, betrag: wert });
    }
    await sichern();
  }

  async function entfernen(b, qId) {
    await setzeZuordnung(b, qId, "");
  }

  // Beleg dieser Quelle zuordnen: schlägt den sinnvollen Betrag vor
  // (freier Betrag des Belegs, höchstens der noch offene Rest der Quelle).
  async function zuordnen(b) {
    const frei = belegFrei(b);
    const rest = ausgewaehlt ? quelleSoll(ausgewaehlt) - (zugeordnetQ.get(ausgewaehltId) ?? 0) : frei;
    const betrag = rest > 0.005 ? Math.min(frei, rest) : frei;
    await setzeZuordnung(b, ausgewaehltId, String(Math.round(betrag * 100) / 100));
  }

  // Den freien Rest eines bereits zugeordneten Belegs ganz dieser Quelle geben.
  async function restHierhin(b) {
    const frei = belegFrei(b);
    if (frei <= 0.005) return;
    const aktuell = betragParsen(zuordnungBetrag(b, ausgewaehltId));
    await setzeZuordnung(b, ausgewaehltId, String(Math.round((aktuell + frei) * 100) / 100));
  }

  function oeffnen(q) {
    ausgewaehltId = q.id;
    zuordnenOffen = false;
  }
  function zurueck() {
    ausgewaehltId = null;
    zuordnenOffen = false;
  }
</script>

<div class="bereich">
  {#if !ausgewaehlt}
    <!-- ===== Übersicht: Liste der Geldquellen ===== -->
    <div class="kopf">
      <h2>Zuteilung – Abrechnung je Förderer</h2>
      <p class="untertitel">
        Wähle eine Geldquelle, um ihr Belege zuzuordnen{#if projektName} ({projektName}){/if}.
        Ziel: jede Quelle ausschöpfen, jeden Beleg decken.
      </p>
    </div>

    {#if quellen.length === 0}
      <div class="leer">
        <p>Noch keine Geldquellen.</p>
        <p class="dezent">Lege zuerst im Reiter <strong>Geldquellen</strong> Förderer/Eigenmittel an.</p>
      </div>
    {:else}
      {#if allesOk}
        <div class="status ok">✓ Geht auf: kein Beleg überzogen, keine Quelle überschritten, alle Belege zugeordnet.</div>
      {:else}
        <div class="status warn">
          {#if belegeUeberzogen}<span>⚠ {belegeUeberzogen} Beleg(e) überzogen</span>{/if}
          {#if quellenUeberzogen}<span>⚠ {quellenUeberzogen} Quelle(n) überschritten</span>{/if}
          {#if belegeOhne}<span>• {belegeOhne} Beleg(e) ohne Zuordnung</span>{/if}
        </div>
      {/if}

      <div class="quellen-liste">
        {#each quellen as q (q.id)}
          {@const i = info(q)}
          <div
            class="qkarte"
            role="button"
            tabindex="0"
            onclick={() => oeffnen(q)}
            onkeydown={(e) => { if (e.key === "Enter" || e.key === " ") { e.preventDefault(); oeffnen(q); } }}
          >
            <div class="qk-kopf">
              <span class="typ t-{q.typ}">{QUELLE_TYP[q.typ] || q.typ}</span>
              <h3>{q.name}</h3>
              <span class="anzahl">{i.anzahl} Beleg{i.anzahl === 1 ? "" : "e"}</span>
            </div>
            <div class="balken"><div class="fuellung" class:voll={i.rest < -0.005} style="width:{i.anteil}%"></div></div>
            <div class="qk-zahlen">
              <span>Soll <strong>{betragFormat(i.soll)}</strong></span>
              <span>Zugeordnet <strong>{betragFormat(i.zu)}</strong></span>
              <span class="rest" class:ueber={i.rest < -0.005} class:offen={i.rest > 0.005}>
                {#if i.rest < -0.005}überzogen {betragFormat(-i.rest)}
                {:else if i.rest > 0.005}offen {betragFormat(i.rest)}
                {:else}✓ vollständig{/if}
              </span>
            </div>
          </div>
        {/each}
      </div>
    {/if}
  {:else}
    <!-- ===== Detail: Abrechnung einer Quelle ===== -->
    {@const i = info(ausgewaehlt)}
    <button class="zurueck" onclick={zurueck}>← Übersicht</button>
    <div class="d-kopf">
      <span class="typ t-{ausgewaehlt.typ}">{QUELLE_TYP[ausgewaehlt.typ] || ausgewaehlt.typ}</span>
      <h2>{ausgewaehlt.name}</h2>
    </div>
    <div class="d-zahlen">
      <span>Soll <strong>{betragFormat(i.soll)}</strong></span>
      <span>Zugeordnet <strong>{betragFormat(i.zu)}</strong></span>
      <span class="rest" class:ueber={i.rest < -0.005} class:offen={i.rest > 0.005}>
        {#if i.rest < -0.005}überzogen {betragFormat(-i.rest)}
        {:else if i.rest > 0.005}offen {betragFormat(i.rest)}
        {:else}✓ vollständig{/if}
      </span>
    </div>

    <div class="export">
      <span class="export-titel">Verwendungsnachweis:</span>
      <button class="zweit schmal" onclick={() => nachweisPdf(ausgewaehltId)}>PDF</button>
      <button class="zweit schmal" onclick={() => nachweisWord(ausgewaehltId)}>Word</button>
    </div>

    {#if zugeordneteBelege.length === 0}
      <p class="leer-klein">Noch keine Belege zugeordnet. Füge unten Belege hinzu.</p>
    {:else}
      <table class="belege">
        <thead>
          <tr>
            <th class="num">Nr.</th>
            <th>Datum</th>
            <th>Beleg</th>
            <th class="betrag">Betrag</th>
            <th class="betrag">zugeordnet</th>
            <th class="akt"></th>
          </tr>
        </thead>
        <tbody>
          {#each zugeordneteBelege as b (b.id)}
            {@const frei = belegFrei(b)}
            <tr>
              <td class="num">{anzeigeNr(b)}</td>
              <td>{datumText(b.datum)}</td>
              <td class="bel">
                <div class="b-emp">{b.empfaenger || "—"}</div>
                {#if b.zweck}<div class="b-zw">{b.zweck}</div>{/if}
              </td>
              <td class="betrag">{betragFormat(belegBrutto(b))}</td>
              <td class="betrag zelle">
                <input
                  type="text"
                  inputmode="decimal"
                  value={zuordnungBetrag(b, ausgewaehltId)}
                  onchange={(e) => setzeZuordnung(b, ausgewaehltId, e.currentTarget.value)}
                />
                {#if frei > 0.005}
                  <button class="rest-btn" title="freien Rest ({betragFormat(frei)}) hierhin" onclick={() => restHierhin(b)}>+{betragFormat(frei)}</button>
                {/if}
              </td>
              <td class="akt">
                <button class="leise gefahr" onclick={() => entfernen(b, ausgewaehltId)} disabled={beschaeftigt}>entfernen</button>
              </td>
            </tr>
          {/each}
        </tbody>
        <tfoot>
          <tr>
            <td colspan="4" class="summe-label">Summe zugeordnet</td>
            <td class="betrag summe">{betragFormat(i.zu)}</td>
            <td></td>
          </tr>
        </tfoot>
      </table>
    {/if}

    <!-- Belege hinzufügen -->
    <div class="zuordnen">
      <button class="zweit" onclick={() => (zuordnenOffen = !zuordnenOffen)} disabled={!verfuegbareBelege.length}>
        + Beleg zuordnen{#if verfuegbareBelege.length} ({verfuegbareBelege.length} verfügbar){/if}
      </button>
      {#if !verfuegbareBelege.length}
        <span class="dezent"> – alle Belege mit freiem Betrag sind dieser Quelle bereits zugeordnet.</span>
      {/if}
      {#if zuordnenOffen && verfuegbareBelege.length}
        <ul class="verfuegbar">
          {#each verfuegbareBelege as b (b.id)}
            <li>
              <span class="vb-nr">{anzeigeNr(b)}</span>
              <span class="vb-emp">{b.empfaenger || "—"}{#if b.zweck} · {b.zweck}{/if}</span>
              <span class="vb-frei">frei {betragFormat(belegFrei(b))}</span>
              <button class="zweit schmal" onclick={() => zuordnen(b)} disabled={beschaeftigt}>zuordnen</button>
            </li>
          {/each}
        </ul>
      {/if}
    </div>
  {/if}
</div>

<style>
  .bereich {
    max-width: 1000px;
    margin: 0 auto;
    padding: 32px 24px 64px;
  }
  .kopf {
    margin-bottom: 16px;
  }
  h2 {
    margin: 0 0 4px;
    font-size: 1.35rem;
    font-weight: 600;
    color: var(--text);
  }
  .untertitel {
    margin: 0;
    color: var(--text-muted);
    font-size: 0.9rem;
    max-width: 620px;
    line-height: 1.5;
  }
  .leer {
    text-align: center;
    color: var(--text-muted);
    padding: 44px 16px;
    border: 1px dashed var(--rand);
    border-radius: 12px;
  }
  .leer p { margin: 0 0 8px; }
  .leer-klein { color: var(--text-muted); font-size: 0.9rem; margin: 14px 0; }
  .dezent { color: var(--text-leise); font-size: 0.88rem; }

  .status {
    border-radius: 8px;
    padding: 9px 14px;
    font-size: 0.88rem;
    margin-bottom: 14px;
  }
  .status.ok { background: var(--erfolg-bg); color: var(--erfolg-text2); }
  .status.warn { background: var(--warnung-bg); color: var(--warnung-text3); display: flex; gap: 18px; flex-wrap: wrap; }

  /* Quellen-Liste (Merkliste-Stil) */
  .quellen-liste { display: flex; flex-direction: column; gap: 12px; }
  .qkarte {
    background: var(--weiss);
    border-radius: 12px;
    box-shadow: 0 1px 3px var(--schatten-sm);
    padding: 14px 18px;
    cursor: pointer;
    transition: box-shadow 0.15s;
  }
  .qkarte:hover { box-shadow: 0 4px 14px var(--schatten-md); }
  .qkarte:focus-visible { outline: 2px solid var(--akzent); outline-offset: 2px; }
  .qk-kopf { display: flex; align-items: center; gap: 10px; }
  .qk-kopf h3 { margin: 0; font-size: 1.02rem; font-weight: 600; flex: 1; min-width: 0; color: var(--text); }
  .qk-kopf .anzahl { color: var(--text-leise); font-size: 0.82rem; white-space: nowrap; }
  .balken { height: 7px; background: var(--flaeche-2c); border-radius: 99px; margin: 10px 0; overflow: hidden; }
  .fuellung { height: 100%; background: var(--akzent); border-radius: 99px; }
  .fuellung.voll { background: var(--gefahr); }
  .qk-zahlen { display: flex; gap: 20px; font-size: 0.86rem; color: var(--text-muted); flex-wrap: wrap; }
  .qk-zahlen strong { color: var(--text); }
  .rest.offen { color: var(--warnung-text3); font-weight: 600; }
  .rest.ueber { color: var(--gefahr); font-weight: 700; }

  /* Detail */
  .zurueck {
    background: none; border: none; color: var(--akzent); font-size: 0.9rem;
    font-weight: 600; cursor: pointer; padding: 0; margin-bottom: 14px; font-family: inherit;
  }
  .zurueck:hover { text-decoration: underline; }
  .d-kopf { display: flex; align-items: center; gap: 10px; }
  .d-kopf h2 { margin: 0; }
  .d-zahlen { display: flex; gap: 22px; font-size: 0.92rem; color: var(--text-muted); margin: 8px 0 14px; flex-wrap: wrap; }
  .d-zahlen strong { color: var(--text); }
  .export { display: flex; align-items: center; gap: 8px; margin-bottom: 18px; }
  .export-titel { font-size: 0.85rem; color: var(--text-muted); font-weight: 600; }

  .typ { display: inline-block; padding: 2px 9px; border-radius: 99px; font-size: 0.74rem; font-weight: 600; }
  .typ.t-foerderung { background: var(--akzent-bg); color: var(--akzent-d4); }
  .typ.t-eigenmittel { background: var(--erfolg-bg); color: var(--erfolg-text2); }

  table.belege { width: 100%; border-collapse: collapse; font-size: 0.9rem; }
  table.belege th, table.belege td { text-align: left; padding: 8px 10px; border-bottom: 1px solid var(--flaeche-3); vertical-align: middle; }
  table.belege th { color: var(--text-muted); font-weight: 600; font-size: 0.78rem; text-transform: uppercase; letter-spacing: 0.03em; }
  .num { width: 56px; color: var(--text-2); font-variant-numeric: tabular-nums; white-space: nowrap; }
  .bel { min-width: 150px; }
  .b-emp { color: var(--text); }
  .b-zw { color: var(--text-leise); font-size: 0.78rem; }
  .betrag { text-align: right; white-space: nowrap; font-variant-numeric: tabular-nums; }
  td.zelle input {
    width: 90px; padding: 5px 7px; border: 2px solid var(--rand); border-radius: 7px;
    font-family: inherit; font-size: 0.85rem; text-align: right; color: var(--text); font-variant-numeric: tabular-nums;
  }
  td.zelle input:focus { outline: none; border-color: var(--akzent); }
  .rest-btn {
    display: block; margin: 4px 0 0 auto; border: none; border-radius: 6px;
    background: var(--akzent-bg); color: var(--akzent-d); font-size: 0.72rem; font-weight: 600;
    padding: 2px 6px; cursor: pointer; font-family: inherit;
  }
  .rest-btn:hover { background: var(--akzent); color: var(--weiss); }
  .akt { text-align: right; white-space: nowrap; }
  .summe-label { text-align: right; font-weight: 600; color: var(--text-2); }
  .summe { font-weight: 700; color: var(--text); }

  .zuordnen { margin-top: 18px; }
  .verfuegbar { list-style: none; margin: 12px 0 0; padding: 0; display: flex; flex-direction: column; gap: 6px; }
  .verfuegbar li {
    display: flex; align-items: center; gap: 12px; padding: 8px 12px;
    background: var(--flaeche-b); border: 1px solid var(--flaeche-3); border-radius: 8px; font-size: 0.88rem;
  }
  .vb-nr { color: var(--text-2); font-variant-numeric: tabular-nums; white-space: nowrap; }
  .vb-emp { flex: 1; min-width: 0; color: var(--text); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .vb-frei { color: var(--text-muted); white-space: nowrap; }

  button.zweit {
    padding: 9px 14px; font-size: 0.9rem; font-weight: 600; font-family: inherit;
    color: var(--text); background: var(--weiss); border: 2px solid var(--rand); border-radius: 8px; cursor: pointer;
  }
  button.zweit:hover:not(:disabled) { border-color: var(--akzent); }
  button.zweit:disabled { color: var(--grau-4); border-color: var(--flaeche-3); cursor: default; }
  button.zweit.schmal { padding: 6px 11px; font-size: 0.82rem; }
  button.leise { background: none; border: none; color: var(--text-muted); font-size: 0.84rem; cursor: pointer; padding: 4px 6px; font-family: inherit; }
  button.leise:hover { color: var(--text); text-decoration: underline; }
  button.leise.gefahr:hover { color: var(--gefahr-text); }
</style>
