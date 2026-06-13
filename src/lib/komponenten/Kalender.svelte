<script>
  // Fristen / Kalender: die Einreichfristen der gemerkten Förderungen,
  // nach Datum sortiert, mit Countdown ("noch X Tage") und Antrag-
  // Status. Klick auf eine Zeile öffnet die Detailansicht.
  import datenbank from "$lib/daten/foerderungen.json";
  import FoerderDetail from "./FoerderDetail.svelte";
  import { LAENDER } from "$lib/begriffe";
  import {
    ANTRAG_STATUS,
    ANTRAG_STANDARD,
    statusLabel,
    statusFarbe,
  } from "$lib/status";

  let {
    merkliste,
    umschalten,
    ordnerOeffnen = null,
    antragErzeugen = null,
    antraege = {},
    antragHolen = null,
    antragSpeichern = null,
  } = $props();

  let ausgewaehlt = $state(null);
  let aktuellerAntrag = $state(null);

  const HEUTE = new Date();
  HEUTE.setHours(0, 0, 0, 0);

  function tageBis(d) {
    return Math.round((new Date(d) - HEUTE) / 86400000);
  }
  function tag(d) {
    return new Date(d).toLocaleDateString("de-DE", { day: "2-digit" });
  }
  function monat(d) {
    return new Date(d).toLocaleDateString("de-DE", { month: "short" });
  }
  function jahr(d) {
    return new Date(d).getFullYear();
  }

  let gemerkte = $derived(
    merkliste
      .map((id) => datenbank.foerderungen.find((f) => f.id === id))
      .filter(Boolean)
  );

  // Pro Förderung: nächste zukünftige bzw. letzte vergangene Frist.
  function fristInfo(f) {
    const laufend = f.weiche_kriterien.zeitpunkt === "laufend";
    const mitTagen = (f.fristen ?? []).map((d) => ({ d, t: tageBis(d) }));
    const zukunft = mitTagen.filter((x) => x.t >= 0).sort((a, b) => a.t - b.t);
    const verg = mitTagen.filter((x) => x.t < 0).sort((a, b) => b.t - a.t);
    if (zukunft.length) return { typ: "anstehend", frist: zukunft[0].d, tage: zukunft[0].t };
    if (laufend) return { typ: "laufend" };
    if (verg.length) return { typ: "vergangen", frist: verg[0].d, tage: verg[0].t };
    return { typ: "offen" }; // keine feste Frist hinterlegt
  }

  let anstehend = $derived(
    gemerkte
      .map((f) => ({ f, info: fristInfo(f) }))
      .filter((x) => x.info.typ === "anstehend")
      .sort((a, b) => a.info.tage - b.info.tage)
  );
  let vergangen = $derived(
    gemerkte
      .map((f) => ({ f, info: fristInfo(f) }))
      .filter((x) => x.info.typ === "vergangen")
      .sort((a, b) => b.info.tage - a.info.tage)
  );
  let ohneFrist = $derived(
    gemerkte
      .map((f) => ({ f, info: fristInfo(f) }))
      .filter((x) => x.info.typ === "laufend" || x.info.typ === "offen")
  );

  function dringlichkeit(tage) {
    if (tage <= 14) return "rot";
    if (tage <= 30) return "gelb";
    return "gruen";
  }

  function badgeFuer(id) {
    const a = antraege[id];
    const key = a?.status ?? ANTRAG_STANDARD;
    return {
      label: statusLabel(ANTRAG_STATUS, key, a?.statusFrei),
      farbe: statusFarbe(ANTRAG_STATUS, key),
    };
  }

  function oeffnen(f) {
    aktuellerAntrag = antragHolen ? antragHolen(f) : null;
    ausgewaehlt = f;
  }
</script>

<div class="bereich">
  <h2>Fristen <span class="anzahl">{anstehend.length} anstehend</span></h2>

  {#if gemerkte.length === 0}
    <p class="leer">
      Noch nichts gemerkt. Sobald du Förderungen merkst, erscheinen ihre
      Einreichfristen hier – nach Datum sortiert.
    </p>
  {:else}
    {#if anstehend.length}
      <div class="liste">
        {#each anstehend as { f, info } (f.id)}
          {@const badge = badgeFuer(f.id)}
          <div
            class="zeile"
            role="button"
            tabindex="0"
            onclick={() => oeffnen(f)}
            onkeydown={(e) => {
              if (e.key === "Enter" || e.key === " ") {
                e.preventDefault();
                oeffnen(f);
              }
            }}
          >
            <div class="datum farbe-{dringlichkeit(info.tage)}">
              <span class="tag">{tag(info.frist)}</span>
              <span class="monat">{monat(info.frist)}</span>
              <span class="jahr">{jahr(info.frist)}</span>
            </div>
            <div class="haupt">
              <div class="kopf">
                <span class="land land-{f.land}">{LAENDER[f.land] ?? f.land}</span>
                <h3>{f.name}</h3>
                <span class="status-badge farbe-{badge.farbe}">
                  <span class="punkt"></span>{badge.label}
                </span>
              </div>
              <p class="meta">{f.foerdergeber}</p>
            </div>
            <div class="countdown farbe-{dringlichkeit(info.tage)}">
              {#if info.tage === 0}
                heute!
              {:else if info.tage === 1}
                morgen
              {:else}
                noch {info.tage} Tage
              {/if}
            </div>
          </div>
        {/each}
      </div>
    {:else}
      <p class="leer">Keine anstehenden Fristen bei deinen gemerkten Förderungen.</p>
    {/if}

    {#if ohneFrist.length}
      <h3 class="gruppe">Laufend / ohne feste Frist</h3>
      <div class="liste">
        {#each ohneFrist as { f } (f.id)}
          <div class="zeile schlicht" role="button" tabindex="0"
            onclick={() => oeffnen(f)}
            onkeydown={(e) => { if (e.key === "Enter" || e.key === " ") { e.preventDefault(); oeffnen(f); } }}
          >
            <div class="datum farbe-grau"><span class="infinity">∞</span></div>
            <div class="haupt">
              <div class="kopf">
                <span class="land land-{f.land}">{LAENDER[f.land] ?? f.land}</span>
                <h3>{f.name}</h3>
              </div>
              <p class="meta">{f.foerdergeber} · laufend einreichbar</p>
            </div>
          </div>
        {/each}
      </div>
    {/if}

    {#if vergangen.length}
      <h3 class="gruppe">Bereits vergangen</h3>
      <div class="liste">
        {#each vergangen as { f, info } (f.id)}
          <div class="zeile schlicht verg" role="button" tabindex="0"
            onclick={() => oeffnen(f)}
            onkeydown={(e) => { if (e.key === "Enter" || e.key === " ") { e.preventDefault(); oeffnen(f); } }}
          >
            <div class="datum farbe-grau">
              <span class="tag">{tag(info.frist)}</span>
              <span class="monat">{monat(info.frist)}</span>
              <span class="jahr">{jahr(info.frist)}</span>
            </div>
            <div class="haupt">
              <div class="kopf">
                <span class="land land-{f.land}">{LAENDER[f.land] ?? f.land}</span>
                <h3>{f.name}</h3>
              </div>
              <p class="meta">{f.foerdergeber} · vor {Math.abs(info.tage)} Tagen</p>
            </div>
          </div>
        {/each}
      </div>
    {/if}
  {/if}
</div>

{#if ausgewaehlt}
  <FoerderDetail
    foerderung={ausgewaehlt}
    alle={datenbank.foerderungen}
    hinweis={datenbank.hinweis}
    gemerkt={merkliste.includes(ausgewaehlt.id)}
    umschalten={umschalten}
    ordnerOeffnen={ordnerOeffnen ? () => ordnerOeffnen(ausgewaehlt.name) : null}
    antragErzeugen={antragErzeugen ? () => antragErzeugen(ausgewaehlt) : null}
    antrag={aktuellerAntrag}
    antragAendern={antragSpeichern}
    schliessen={() => {
      ausgewaehlt = null;
      aktuellerAntrag = null;
    }}
  />
{/if}

<style>
  .bereich {
    max-width: 1080px;
    margin: 0 auto;
    padding: 32px 24px 64px;
  }
  h2 {
    margin: 0 0 20px;
    font-size: 1.35rem;
    font-weight: 600;
  }
  .anzahl {
    color: #5e6c84;
    font-size: 0.9rem;
    font-weight: 400;
    margin-left: 8px;
  }
  .gruppe {
    margin: 32px 0 12px;
    font-size: 1.05rem;
    font-weight: 600;
    color: #44546f;
  }
  .leer {
    color: #5e6c84;
  }

  .liste {
    display: flex;
    flex-direction: column;
    gap: 12px;
  }
  .zeile {
    display: flex;
    align-items: center;
    gap: 16px;
    background: #fff;
    border-radius: 12px;
    box-shadow: 0 1px 3px rgba(9, 30, 66, 0.12);
    padding: 14px 18px;
    cursor: pointer;
    transition: box-shadow 0.15s;
  }
  .zeile:hover {
    box-shadow: 0 4px 14px rgba(9, 30, 66, 0.18);
  }
  .zeile:focus-visible {
    outline: 2px solid #4f6df5;
    outline-offset: 2px;
  }
  .zeile.verg {
    opacity: 0.7;
  }

  /* Datumsblock links */
  .datum {
    flex-shrink: 0;
    width: 60px;
    height: 60px;
    border-radius: 10px;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    line-height: 1.1;
  }
  .datum .tag {
    font-size: 1.4rem;
    font-weight: 700;
  }
  .datum .monat {
    font-size: 0.72rem;
    font-weight: 600;
    text-transform: uppercase;
  }
  .datum .jahr {
    font-size: 0.66rem;
    opacity: 0.8;
  }
  .datum .infinity {
    font-size: 1.6rem;
    font-weight: 700;
  }
  .datum.farbe-rot { background: #ffeceb; color: #ae2e24; }
  .datum.farbe-gelb { background: #fff7d6; color: #7f5f01; }
  .datum.farbe-gruen { background: #dcfff1; color: #216e4e; }
  .datum.farbe-grau { background: #f1f2f4; color: #5e6c84; }

  .haupt {
    flex: 1;
    min-width: 0;
  }
  .kopf {
    display: flex;
    align-items: center;
    gap: 10px;
    flex-wrap: wrap;
  }
  .kopf h3 {
    margin: 0;
    font-size: 1.02rem;
    font-weight: 600;
  }
  .meta {
    margin: 4px 0 0;
    color: #5e6c84;
    font-size: 0.85rem;
  }

  .countdown {
    flex-shrink: 0;
    font-size: 0.88rem;
    font-weight: 700;
    padding: 6px 12px;
    border-radius: 99px;
    white-space: nowrap;
  }
  .countdown.farbe-rot { background: #ffeceb; color: #ae2e24; }
  .countdown.farbe-gelb { background: #fff7d6; color: #7f5f01; }
  .countdown.farbe-gruen { background: #dcfff1; color: #216e4e; }

  .land {
    font-size: 0.72rem;
    font-weight: 700;
    letter-spacing: 0.03em;
    padding: 3px 9px;
    border-radius: 99px;
    text-transform: uppercase;
  }
  .land-DE { background: #e9f2ff; color: #0055cc; }
  .land-AT { background: #ffeceb; color: #ae2e24; }
  .land-CH { background: #fff7d6; color: #7f5f01; }
  .land-INT { background: #f3f0ff; color: #5e4db2; }

  .status-badge {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 3px 10px 3px 8px;
    border-radius: 99px;
    font-size: 0.78rem;
    font-weight: 600;
  }
  .status-badge .punkt {
    width: 8px;
    height: 8px;
    border-radius: 50%;
  }
  .status-badge.farbe-blau { background: #e9f0ff; color: #2b46c4; }
  .status-badge.farbe-blau .punkt { background: #4f6df5; }
  .status-badge.farbe-lila { background: #f1edff; color: #5e44b0; }
  .status-badge.farbe-lila .punkt { background: #8270db; }
  .status-badge.farbe-gruen { background: #dcfff1; color: #216e4e; }
  .status-badge.farbe-gruen .punkt { background: #22a06b; }
  .status-badge.farbe-rot { background: #ffeceb; color: #ae2e24; }
  .status-badge.farbe-rot .punkt { background: #ca3521; }
  .status-badge.farbe-gelb { background: #fff7d6; color: #7f5f01; }
  .status-badge.farbe-gelb .punkt { background: #e2a400; }
  .status-badge.farbe-grau { background: #f1f2f4; color: #44546f; }
  .status-badge.farbe-grau .punkt { background: #b3bac5; }
</style>
