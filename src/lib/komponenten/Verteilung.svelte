<script>
  // Abrechnungs-Modus, Phase A4b: Verteil-Ansicht ("rumschieben, bis es
  // aufgeht"). Belege als Zeilen, Geldquellen als Spalten; in den Zellen
  // stehen die anteilig zugeordneten Beträge. Live-Summen + Warnungen.
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
    QUELLE_TYP,
  } from "$lib/abrechnung";

  let {
    belege = [],
    quellen = [],
    speichern, // (neueBelege) – speichert die geänderten Zuordnungen
    kfp = { kosten: [], finanzierung: [] },
    projektName = "",
  } = $props();

  // Arbeitskopie der Belege; Änderungen werden sofort gesichert.
  let liste = $state(structuredClone($state.snapshot(belege)));
  let beschaeftigt = $state(false);

  let nummern = $derived(belegNummern(liste, kfp));
  const anzeigeNr = (b) => nummern.get(b.id) ?? `#${b.nr}`;

  let zugeordnetQ = $derived(zugeordnetJeQuelle(liste));

  // Sortierung wie in der Belegliste (Datum, dann Roh-Nr).
  let sortiert = $derived(
    [...liste].sort((a, b) => {
      const d = String(a.datum ?? "").localeCompare(String(b.datum ?? ""));
      return d !== 0 ? d : Number(a.nr) - Number(b.nr);
    })
  );

  // Warnungen.
  let belegeUeberzogen = $derived(liste.filter((b) => belegFrei(b) < -0.005).length);
  let belegeOhne = $derived(
    liste.filter((b) => belegBrutto(b) > 0 && belegZugeordnet(b) < 0.005).length
  );
  let quellenUeberzogen = $derived(
    quellen.filter((q) => quelleSoll(q) - (zugeordnetQ.get(q.id) ?? 0) < -0.005).length
  );
  let allesOk = $derived(belegeUeberzogen === 0 && belegeOhne === 0 && quellenUeberzogen === 0);

  async function sichern() {
    beschaeftigt = true;
    try {
      await speichern($state.snapshot(liste));
    } finally {
      beschaeftigt = false;
    }
  }

  function zellenWert(b, qId) {
    const z = (b.zuordnungen ?? []).find((x) => x.quelleId === qId);
    return z ? z.betrag : "";
  }

  async function setzeZelle(b, qId, roh) {
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

  // Den noch freien Betrag eines Belegs ganz dieser Quelle zuordnen
  // (bequemes "Rest hierhin"). Addiert zum schon zugeordneten Wert.
  async function restHierhin(b, qId) {
    const frei = belegFrei(b);
    if (frei <= 0.005) return;
    const aktuell = betragParsen(zellenWert(b, qId));
    await setzeZelle(b, qId, String(Math.round((aktuell + frei) * 100) / 100));
  }
</script>

<div class="bereich">
  <div class="kopf">
    <h2>Verteilung</h2>
    <p class="untertitel">
      Ordne jeden Beleg{#if projektName} von <strong>{projektName}</strong>{/if} anteilig den
      Geldquellen zu – so lange, bis jeder Beleg gedeckt und jede Quelle ausgeschöpft ist.
    </p>
  </div>

  {#if quellen.length === 0}
    <div class="leer">
      <p>Noch keine Geldquellen.</p>
      <p class="dezent">Lege zuerst im Reiter <strong>Geldquellen</strong> Förderer/Eigenmittel an.</p>
    </div>
  {:else if liste.length === 0}
    <div class="leer">
      <p>Noch keine Belege.</p>
      <p class="dezent">Erfasse zuerst im Reiter <strong>Belege</strong> ein paar Belege.</p>
    </div>
  {:else}
    <!-- Warn-/Status-Leiste -->
    {#if allesOk}
      <div class="status ok">✓ Geht auf: kein Beleg überzogen, keine Quelle überschritten, alle Belege zugeordnet.</div>
    {:else}
      <div class="status warn">
        {#if belegeUeberzogen}<span>⚠ {belegeUeberzogen} Beleg(e) überzogen</span>{/if}
        {#if quellenUeberzogen}<span>⚠ {quellenUeberzogen} Quelle(n) überschritten</span>{/if}
        {#if belegeOhne}<span>• {belegeOhne} Beleg(e) ohne Zuordnung</span>{/if}
      </div>
    {/if}

    <div class="scroll">
      <table class="matrix">
        <thead>
          <tr>
            <th class="num">Nr.</th>
            <th class="beleg">Beleg</th>
            <th class="betrag">Betrag</th>
            {#each quellen as q (q.id)}
              <th class="q">
                <div class="qname" title={q.name}>{q.name}</div>
                <div class="qsoll">Soll {betragFormat(quelleSoll(q))}</div>
              </th>
            {/each}
            <th class="betrag frei-h">Frei</th>
          </tr>
        </thead>
        <tbody>
          {#each sortiert as b (b.id)}
            {@const frei = belegFrei(b)}
            <tr>
              <td class="num">{anzeigeNr(b)}</td>
              <td class="beleg">
                <div class="b-emp">{b.empfaenger || "—"}</div>
                {#if b.zweck}<div class="b-zw">{b.zweck}</div>{/if}
              </td>
              <td class="betrag">{betragFormat(belegBrutto(b))}</td>
              {#each quellen as q (q.id)}
                <td class="zelle">
                  <input
                    type="text"
                    inputmode="decimal"
                    value={zellenWert(b, q.id)}
                    onchange={(e) => setzeZelle(b, q.id, e.currentTarget.value)}
                    placeholder="–"
                  />
                  {#if frei > 0.005}
                    <button class="rest" title="freien Rest hierhin" onclick={() => restHierhin(b, q.id)}>+</button>
                  {/if}
                </td>
              {/each}
              <td class="betrag frei" class:ueber={frei < -0.005} class:offen={frei > 0.005}>
                {betragFormat(frei)}
              </td>
            </tr>
          {/each}
        </tbody>
        <tfoot>
          <tr>
            <td colspan="3" class="summe-label">Zugeordnet</td>
            {#each quellen as q (q.id)}
              <td class="betrag">{betragFormat(zugeordnetQ.get(q.id) ?? 0)}</td>
            {/each}
            <td></td>
          </tr>
          <tr>
            <td colspan="3" class="summe-label">Rest (Soll − zugeordnet)</td>
            {#each quellen as q (q.id)}
              {@const rest = quelleSoll(q) - (zugeordnetQ.get(q.id) ?? 0)}
              <td class="betrag" class:ueber={rest < -0.005} class:offen={rest > 0.005}>
                {betragFormat(rest)}
              </td>
            {/each}
            <td></td>
          </tr>
        </tfoot>
      </table>
    </div>

    <p class="hinweis">
      Tipp: Das <strong>+</strong> in einer Zelle ordnet den noch freien Restbetrag des Belegs ganz
      dieser Quelle zu. „Frei" zeigt, was an einem Beleg noch offen ist (rot = mehr zugeordnet als
      der Beleg hergibt). Eigenmittel zählen wie eine Quelle.
    </p>
  {/if}
</div>

<style>
  .bereich {
    max-width: 1100px;
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
    color: #172b4d;
  }
  .untertitel {
    margin: 0;
    color: #5e6c84;
    font-size: 0.9rem;
    max-width: 600px;
    line-height: 1.5;
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

  .status {
    border-radius: 8px;
    padding: 9px 14px;
    font-size: 0.88rem;
    margin-bottom: 14px;
  }
  .status.ok {
    background: #dcfff1;
    color: #14794e;
  }
  .status.warn {
    background: #fff7d6;
    color: #7a5b00;
    display: flex;
    gap: 18px;
    flex-wrap: wrap;
  }

  .scroll {
    overflow-x: auto;
  }
  table.matrix {
    border-collapse: collapse;
    font-size: 0.88rem;
    min-width: 100%;
  }
  table.matrix th,
  table.matrix td {
    padding: 7px 9px;
    border-bottom: 1px solid #ebedf0;
    text-align: left;
    vertical-align: middle;
  }
  table.matrix th {
    color: #5e6c84;
    font-weight: 600;
    font-size: 0.76rem;
  }
  .num {
    width: 56px;
    color: #44546f;
    font-variant-numeric: tabular-nums;
    white-space: nowrap;
  }
  .beleg {
    min-width: 150px;
  }
  .b-emp {
    color: #172b4d;
  }
  .b-zw {
    color: #8590a2;
    font-size: 0.78rem;
  }
  .betrag {
    text-align: right;
    white-space: nowrap;
    font-variant-numeric: tabular-nums;
  }
  th.q {
    text-align: right;
    min-width: 96px;
  }
  .qname {
    color: #172b4d;
    font-weight: 600;
    overflow: hidden;
    text-overflow: ellipsis;
    max-width: 130px;
    margin-left: auto;
  }
  .qsoll {
    color: #8590a2;
    font-weight: 500;
  }
  td.zelle {
    text-align: right;
    white-space: nowrap;
    position: relative;
  }
  td.zelle input {
    width: 78px;
    padding: 5px 7px;
    border: 2px solid #dfe1e6;
    border-radius: 7px;
    font-family: inherit;
    font-size: 0.85rem;
    text-align: right;
    color: #172b4d;
    font-variant-numeric: tabular-nums;
  }
  td.zelle input:focus {
    outline: none;
    border-color: #4f6df5;
  }
  .rest {
    margin-left: 4px;
    width: 20px;
    height: 26px;
    border: none;
    border-radius: 6px;
    background: #eef1ff;
    color: #3d5bf0;
    font-weight: 700;
    cursor: pointer;
    vertical-align: middle;
  }
  .rest:hover {
    background: #4f6df5;
    color: #fff;
  }
  .frei.ueber,
  .ueber {
    color: #ca3521;
    font-weight: 700;
  }
  .frei.offen,
  .offen {
    color: #7a5b00;
  }
  tfoot td {
    border-top: 2px solid #dfe1e6;
    font-weight: 600;
    color: #172b4d;
  }
  .summe-label {
    text-align: right;
    color: #44546f;
  }
  .hinweis {
    margin-top: 14px;
    color: #5e6c84;
    font-size: 0.84rem;
    line-height: 1.5;
  }
</style>
