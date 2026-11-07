<script>
  // Übersicht einer Projektgruppe: alle Ausgaben nebeneinander.
  // B · Zahlen je Ausgabe (neueste oben), A · Förderer über die Jahre.
  // Nur Ansicht – ein Klick auf eine Ausgabe öffnet sie zum Bearbeiten.
  // Die Zahlen rechnet gruppenUebersicht.js (mit Node-Test).
  import { betragFormat } from "$lib/kfpRechnen";
  import { datumText } from "$lib/abrechnung";

  let { uebersicht, aktivId, ausgabeOeffnen } = $props();

  // Tabelle B: neueste Ausgabe zuerst.
  let zeilenB = $derived(
    uebersicht.ausgaben.map((p, i) => ({ p, z: uebersicht.zahlen[i] })).reverse()
  );
  let archiviert = $derived(uebersicht.ausgaben.filter((p) => p.archiviert).length);

  /** @param {any} p */
  function marke(p) {
    if (p.archiviert) return "Archiv";
    return p.id === aktivId ? "aktiv" : "";
  }
</script>

<div class="bereich">
  <div class="kopf">
    <h2>📚 {uebersicht.gruppe.name}</h2>
    <p>
      Projektgruppe · {uebersicht.ausgaben.length}
      {uebersicht.ausgaben.length === 1 ? "Ausgabe" : "Ausgaben"}{archiviert ? ` (${archiviert} im Archiv)` : ""}
      · nur Ansicht. Bearbeitet wird in der einzelnen Ausgabe: Ein Klick auf ihren Namen öffnet sie.
    </p>
  </div>

  <section class="kachel">
    <h3>Zahlen je Ausgabe</h3>
    <div class="rahmen">
      <table>
        <thead>
          <tr>
            <th>Ausgabe</th>
            <th class="zahl" title="Summe des Kostenplans">Kosten</th>
            <th class="zahl" title="Alle Anträge auf der Merkliste, auch die noch in Arbeit; bei einem Teil-Antrag nur sein Anteil">Beantragt</th>
            <th class="zahl" title="Nur Anträge mit „Zusage erhalten“; steht der Förderer als Geldquelle mit Soll in der Abrechnung, gilt dieser Betrag">Bewilligt</th>
            <th class="zahl" title="In der Zuteilung Belegen zugeordnet, nur bei Förderern">Abgerechnet</th>
            <th class="zahl" title="Nächste Frist für einen Verwendungsnachweis">Nachweis bis</th>
          </tr>
        </thead>
        <tbody>
          {#each zeilenB as { p, z } (p.id)}
            <tr class:grau={p.archiviert}>
              <td>
                <button class="ausgabe-link" onclick={() => ausgabeOeffnen(p.id)} title="Diese Ausgabe öffnen">{p.archiviert ? "🔒 " : ""}{p.name}</button>
                {#if marke(p)}<span class="marke">{marke(p)}</span>{/if}
              </td>
              <td class="zahl">{betragFormat(z.kosten)}</td>
              <td class="zahl">{betragFormat(z.beantragt)}</td>
              <td class="zahl">{betragFormat(z.bewilligt)}</td>
              <td class="zahl">{betragFormat(z.abgerechnet)}</td>
              <td class="zahl">
                {#if z.nachweis.naechste}
                  <span class="frist">{datumText(z.nachweis.naechste)}</span>{z.nachweis.offen > 1 ? ` +${z.nachweis.offen - 1}` : ""}
                {:else}
                  <span class="leise">—</span>
                {/if}
              </td>
            </tr>
          {/each}
        </tbody>
        {#if uebersicht.ausgaben.length > 1}
          <tfoot>
            <tr>
              <td>Alle Ausgaben</td>
              <td class="zahl">{betragFormat(uebersicht.summe.kosten)}</td>
              <td class="zahl">{betragFormat(uebersicht.summe.beantragt)}</td>
              <td class="zahl">{betragFormat(uebersicht.summe.bewilligt)}</td>
              <td class="zahl">{betragFormat(uebersicht.summe.abgerechnet)}</td>
              <td></td>
            </tr>
          </tfoot>
        {/if}
      </table>
    </div>
  </section>

  <section class="kachel">
    <h3>Förderer im Vergleich</h3>
    {#if uebersicht.vergleich.length}
      <div class="rahmen">
        <table class="vergleich">
          <thead>
            <tr>
              <th>Förderer</th>
              {#each uebersicht.ausgaben as p (p.id)}
                <th class:grau={p.archiviert}>{p.name}{marke(p) ? ` · ${marke(p)}` : ""}</th>
              {/each}
            </tr>
          </thead>
          <tbody>
            {#each uebersicht.vergleich as r (r.id)}
              <tr>
                <td class="name">{r.name}</td>
                {#each r.zellen as c, i (uebersicht.ausgaben[i].id)}
                  <td class:grau={uebersicht.ausgaben[i].archiviert}>
                    {#if c.beantragt}
                      <span class="status-badge farbe-{c.farbe}">{c.label}</span>
                      <small>{betragFormat(c.betrag)}</small>
                    {:else}
                      <span class="leise">nicht beantragt</span>
                    {/if}
                  </td>
                {/each}
              </tr>
            {/each}
          </tbody>
        </table>
      </div>
    {:else}
      <p class="leer">In keiner Ausgabe steht bisher eine Förderung auf der Merkliste.</p>
    {/if}
  </section>
</div>

<style>
  .bereich {
    max-width: 1080px;
    margin: 0 auto;
    padding: 32px 24px 64px;
  }
  .kopf {
    margin-bottom: 18px;
  }
  h2 {
    margin: 0 0 4px;
    font-size: 1.35rem;
    font-weight: 600;
    color: var(--text);
    overflow-wrap: anywhere;
  }
  .kopf p {
    margin: 0;
    font-size: 0.9rem;
    color: var(--text-muted);
  }
  .kachel {
    min-width: 0;
    margin-bottom: 14px;
    background: var(--weiss);
    border: 1px solid var(--rand);
    border-radius: 12px;
    padding: 16px 18px;
  }
  h3 {
    margin: 0 0 8px;
    font-size: 1rem;
    font-weight: 600;
    color: var(--text);
  }
  .rahmen {
    overflow-x: auto;
  }
  table {
    width: 100%;
    border-collapse: collapse;
    font-size: 0.88rem;
  }
  th {
    padding: 6px 8px;
    font-size: 0.76rem;
    font-weight: 600;
    text-align: left;
    color: var(--text-muted);
    border-bottom: 1px solid var(--rand);
    white-space: nowrap;
  }
  td {
    padding: 8px;
    color: var(--text);
    border-top: 1px solid var(--flaeche-3);
    vertical-align: middle;
  }
  .zahl {
    text-align: right;
    white-space: nowrap;
    font-variant-numeric: tabular-nums;
  }
  tfoot td {
    font-weight: 600;
    border-top: 1px solid var(--rand);
  }
  tr.grau td,
  td.grau,
  th.grau {
    color: var(--text-muted);
  }
  .ausgabe-link {
    width: auto;
    margin: 0;
    padding: 0;
    font: inherit;
    font-weight: 600;
    color: var(--link, var(--akzent-text));
    background: none;
    border: none;
    cursor: pointer;
    text-align: left;
  }
  tr.grau .ausgabe-link {
    font-weight: 400;
    color: var(--text-muted);
  }
  .ausgabe-link:hover:not(:disabled) {
    background: none;
    text-decoration: underline;
  }
  .marke {
    margin-left: 8px;
    font-size: 0.74rem;
    color: var(--text-muted);
  }
  .frist {
    color: var(--warnung-text);
    font-weight: 600;
  }
  .leise {
    color: var(--text-muted);
    font-size: 0.82rem;
  }
  .vergleich td.name {
    font-weight: 600;
    white-space: nowrap;
  }
  .vergleich td small {
    display: block;
    margin-top: 3px;
    font-size: 0.78rem;
    color: var(--text-muted);
    font-variant-numeric: tabular-nums;
  }
  .leer {
    margin: 0;
    font-size: 0.88rem;
    color: var(--text-muted);
  }
  .status-badge {
    display: inline-block;
    padding: 2px 9px;
    border-radius: 99px;
    font-size: 0.76rem;
    font-weight: 600;
    white-space: nowrap;
  }
  .status-badge.farbe-blau { background: var(--akzent-bg2); color: var(--akzent-d3); }
  .status-badge.farbe-lila { background: var(--lila-bg2); color: var(--lila-d2); }
  .status-badge.farbe-gruen { background: var(--erfolg-bg); color: var(--erfolg-text); }
  .status-badge.farbe-rot { background: var(--gefahr-bg); color: var(--gefahr-text); }
  .status-badge.farbe-gelb { background: var(--warnung-bg); color: var(--warnung-text); }
  .status-badge.farbe-grau { background: var(--flaeche-2b); color: var(--text-2); }
</style>
