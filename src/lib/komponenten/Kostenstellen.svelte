<script>
  // Abrechnungs-Modus, Phase A3: Plan-/Ist-Übersicht je Kostenstelle.
  // Liest den KFP (Plan) und die Belege (Ist) – nur Anzeige, ändert nichts.
  import { kostenstellenUebersicht, betragFormat } from "$lib/abrechnung";

  let { kfp = { kosten: [], finanzierung: [] }, belege = [], projektName = "" } = $props();

  let u = $derived(kostenstellenUebersicht(kfp, belege));
  let hatKostenstellen = $derived((kfp?.kosten ?? []).some((k) => (k.posten ?? []).length));

  // Negativer Rest = überzogen (Ist über Plan).
  const restKlasse = (rest) => (rest < -0.005 ? "ueber" : "");
</script>

<div class="bereich">
  <div class="kopf">
    <h2>Kostenstellen – Plan / Ist</h2>
    <p class="untertitel">
      Geplante Kosten aus dem Kostenplan{#if projektName} von <strong>{projektName}</strong>{/if}
      gegenüber den tatsächlich erfassten Belegen.
    </p>
  </div>

  {#if !hatKostenstellen}
    <div class="leer">
      <p>Noch keine Kostenstellen vorhanden.</p>
      <p class="dezent">
        Lege im <strong>Kostenplan</strong> (Modus „Antrag") Kosten-Posten an – sie erscheinen
        hier automatisch als Kostenstellen.
      </p>
    </div>
  {:else}
    <table class="ks-tab">
      <thead>
        <tr>
          <th>Kostenstelle</th>
          <th class="num">Belege</th>
          <th class="betrag">Plan</th>
          <th class="betrag">Ist</th>
          <th class="betrag">Rest</th>
        </tr>
      </thead>
      <tbody>
        {#each u.kategorien as kat}
          <tr class="kat">
            <td>{kat.nummer} {kat.name}</td>
            <td class="num"></td>
            <td class="betrag">{betragFormat(kat.planSumme)}</td>
            <td class="betrag">{betragFormat(kat.istSumme)}</td>
            <td class="betrag {restKlasse(kat.restSumme)}">{betragFormat(kat.restSumme)}</td>
          </tr>
          {#each kat.posten as p}
            <tr>
              <td class="pname">{p.nummer} {p.bezeichnung}</td>
              <td class="num">{p.anzahl || ""}</td>
              <td class="betrag">{betragFormat(p.plan)}</td>
              <td class="betrag">{betragFormat(p.ist)}</td>
              <td class="betrag {restKlasse(p.rest)}">{betragFormat(p.rest)}</td>
            </tr>
          {/each}
        {/each}

        {#if u.unzugeordnetAnzahl > 0}
          <tr class="unzu">
            <td>Nicht zugeordnet</td>
            <td class="num">{u.unzugeordnetAnzahl}</td>
            <td class="betrag">—</td>
            <td class="betrag">{betragFormat(u.unzugeordnetSumme)}</td>
            <td class="betrag">—</td>
          </tr>
        {/if}
      </tbody>
      <tfoot>
        <tr>
          <td>Gesamt</td>
          <td class="num"></td>
          <td class="betrag">{betragFormat(u.planGesamt)}</td>
          <td class="betrag">{betragFormat(u.istGesamt)}</td>
          <td class="betrag {restKlasse(u.planGesamt - u.istGesamt)}">
            {betragFormat(u.planGesamt - u.istGesamt)}
          </td>
        </tr>
      </tfoot>
    </table>

    {#if u.unzugeordnetAnzahl > 0}
      <p class="hinweis">
        {u.unzugeordnetAnzahl} Beleg(e) sind noch keiner Kostenstelle zugeordnet – du kannst die
        Kostenstelle im jeweiligen Beleg nachtragen (Reiter „Belege").
      </p>
    {/if}
  {/if}
</div>

<style>
  .bereich {
    max-width: 920px;
    margin: 0 auto;
    padding: 32px 24px 64px;
  }
  .kopf {
    margin-bottom: 20px;
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
    max-width: 520px;
    line-height: 1.5;
  }

  .leer {
    text-align: center;
    color: var(--text-muted);
    padding: 44px 16px;
    border: 1px dashed var(--rand);
    border-radius: 12px;
  }
  .leer p {
    margin: 0 0 8px;
  }
  .dezent {
    font-size: 0.88rem;
  }

  table.ks-tab {
    width: 100%;
    border-collapse: collapse;
    font-size: 0.9rem;
  }
  table.ks-tab th,
  table.ks-tab td {
    text-align: left;
    padding: 8px 10px;
    border-bottom: 1px solid var(--flaeche-3);
  }
  table.ks-tab th {
    color: var(--text-muted);
    font-weight: 600;
    font-size: 0.78rem;
    text-transform: uppercase;
    letter-spacing: 0.03em;
  }
  .num {
    text-align: center;
    width: 70px;
    color: var(--text-muted);
  }
  .betrag {
    text-align: right;
    white-space: nowrap;
    font-variant-numeric: tabular-nums;
  }
  tr.kat td {
    background: var(--akzent-bg-y);
    font-weight: 600;
    color: var(--text);
  }
  .pname {
    padding-left: 22px;
    color: var(--text-2);
  }
  tr.unzu td {
    color: var(--warnung-text5);
    background: var(--warnung-bg2);
    font-style: italic;
  }
  tfoot td {
    border-top: 2px solid var(--rand);
    font-weight: 700;
    color: var(--text);
  }
  .betrag.ueber {
    color: var(--gefahr-text);
    font-weight: 700;
  }
  .hinweis {
    margin-top: 14px;
    color: var(--warnung-text5);
    background: var(--warnung-bg2);
    border-radius: 8px;
    padding: 10px 12px;
    font-size: 0.86rem;
  }
</style>
