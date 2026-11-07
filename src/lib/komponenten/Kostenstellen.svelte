<script>
  // Abrechnungs-Modus, Phase A3: Plan-/Ist-Übersicht je Kostenstelle.
  // Liest den KFP (Plan) und die Belege (Ist) – nur Anzeige, ändert nichts.
  import { kostenstellenUebersicht, bausteinUebersicht, betragFormat } from "$lib/abrechnung";
  import { kostenstellenVonTeil } from "$lib/teilwahl";
  import TeilBand from "./TeilBand.svelte";

  // `teil`: im Projektmenü gewählter Teil (oder null). Dann stehen nur seine
  // Kostenstellen da; die Nummern bleiben wie im ganzen Plan.
  let {
    kfp = { kosten: [], finanzierung: [] },
    belege = [],
    projektName = "",
    bausteine = [],
    teil = null,
    ganzeAusgabe = null,
  } = $props();

  let u = $derived(kostenstellenUebersicht(kfp, belege));
  // Plan/Ist je Baustein; leer, wenn das Projekt keine Bausteine hat.
  let teile = $derived(bausteinUebersicht(kfp, belege, bausteine));
  let hatKostenstellen = $derived((kfp?.kosten ?? []).some((k) => (k.posten ?? []).length));

  // Beim Blick auf einen Teil: nur seine Kostenstellen, Summen neu gerechnet.
  let teilIds = $derived(kostenstellenVonTeil(teil, kfp, bausteine));
  const summeVon = (liste, key) => liste.reduce((s, p) => s + p[key], 0);
  let kategorien = $derived(
    teil
      ? u.kategorien
          .map((kat) => {
            const posten = kat.posten.filter((p) => teilIds.has(p.id));
            const planSumme = summeVon(posten, "plan");
            const istSumme = summeVon(posten, "ist");
            return { ...kat, posten, planSumme, istSumme, restSumme: planSumme - istSumme };
          })
          .filter((kat) => kat.posten.length)
      : u.kategorien
  );
  let planGesamt = $derived(teil ? summeVon(kategorien, "planSumme") : u.planGesamt);
  let istGesamt = $derived(teil ? summeVon(kategorien, "istSumme") : u.istGesamt);

  // Negativer Rest = überzogen (Ist über Plan).
  const restKlasse = (rest) => (rest < -0.005 ? "ueber" : "");
</script>

<div class="bereich">
  {#if teil}
    <TeilBand {teil} ausgabeName={projektName} {ganzeAusgabe} />
  {/if}
  <div class="kopf">
    <h2>Kostenstellen – Plan / Ist</h2>
    <p class="untertitel">
      Geplante Kosten aus dem Kostenplan{#if projektName}{" "}von <strong>{projektName}</strong>{/if}
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
    {#if teile.length && !teil}
      <h3>Je Baustein</h3>
      <table class="ks-tab teile">
        <thead>
          <tr>
            <th>Baustein</th>
            <th class="num">Belege</th>
            <th class="betrag">Plan</th>
            <th class="betrag">Ist</th>
            <th class="betrag">Rest</th>
          </tr>
        </thead>
        <tbody>
          {#each teile as t (t.id)}
            <tr class:gemeinsam={t.id === "gemeinsam"}>
              <td>{t.name}</td>
              <td class="num">{t.anzahl || ""}</td>
              <td class="betrag">{betragFormat(t.plan)}</td>
              <td class="betrag">{betragFormat(t.ist)}</td>
              <td class="betrag {restKlasse(t.rest)}">{betragFormat(t.rest)}</td>
            </tr>
          {/each}
        </tbody>
      </table>
      <h3>Je Kostenstelle</h3>
    {/if}
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
        {#if teil && !kategorien.length}
          <tr>
            <td colspan="5" class="leer-zeile">„{teil.name}“ hat im Kostenplan noch keine Kostenstelle.</td>
          </tr>
        {/if}
        {#each kategorien as kat}
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

        {#if u.unzugeordnetAnzahl > 0 && !teil}
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
          <td>{teil ? `Gesamt „${teil.name}“` : "Gesamt"}</td>
          <td class="num"></td>
          <td class="betrag">{betragFormat(planGesamt)}</td>
          <td class="betrag">{betragFormat(istGesamt)}</td>
          <td class="betrag {restKlasse(planGesamt - istGesamt)}">
            {betragFormat(planGesamt - istGesamt)}
          </td>
        </tr>
      </tfoot>
    </table>

    {#if u.unzugeordnetAnzahl > 0 && !teil}
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
  .leer-zeile {
    color: var(--text-muted);
    font-style: italic;
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

  h3 {
    margin: 22px 0 8px;
    font-size: 0.95rem;
    font-weight: 600;
    color: var(--text);
  }
  table.teile {
    margin-bottom: 8px;
  }
  tr.gemeinsam td {
    color: var(--text-muted);
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
    /* Feste Breite: Die Tabellen „Je Baustein" und „Je Kostenstelle" stehen
       dadurch Spalte für Spalte untereinander. */
    width: 110px;
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
