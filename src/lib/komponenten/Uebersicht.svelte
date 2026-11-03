<script>
  // Übersicht: die Startseite nach dem Entsperren. Beantwortet die Frage
  // „Was braucht jetzt meine Aufmerksamkeit?". Sie zeigt nur an und führt
  // weiter; gearbeitet wird in den bestehenden Bereichen. Liest nur aus dem
  // entsperrten Tresor und schickt nichts übers Netz.
  import KalenderKachel from "./KalenderKachel.svelte";
  import { termineSammeln, naechsteTermine, tageBis } from "$lib/termine";
  import { alsNaechstes, antraegeNachStatus, abrechnungStand, tageText } from "$lib/uebersicht";
  import { belegFrei, quelleSoll, zugeordnetJeQuelle, betragFormat } from "$lib/abrechnung";

  let {
    projekt = null,
    foerderungen = [],
    teamBoard = null,
    meineProjektIds = [],
    foerderungLabel = null,
    interneAnlegen,
    zuZiel, // (ziel) – springt in den passenden Bereich
    projektErstellen,
  } = $props();

  let gemerkte = $derived(
    (projekt?.merkliste ?? []).map((id) => foerderungen.find((f) => f.id === id)).filter(Boolean)
  );
  let antraege = $derived(projekt?.antraege ?? {});
  let termine = $derived(
    termineSammeln({
      gemerkte,
      antraege,
      interneFristen: projekt?.interneFristen ?? [],
      quellen: projekt?.abrechnung?.quellen ?? [],
      teamBoard,
      meineProjektIds,
      foerderungLabel,
    })
  );
  let naechstes = $derived(alsNaechstes({ termine, gemerkte, antraege }));
  let antragStatus = $derived(antraegeNachStatus(gemerkte, antraege));
  let abrechnung = $derived(
    abrechnungStand(projekt?.abrechnung, { belegFrei, quelleSoll, zugeordnetJeQuelle })
  );
  let naechsterTermin = $derived(naechsteTermine(termine, 1)[0] ?? null);
  // Ein neues Projekt hat noch nichts, worüber man informieren könnte.
  let projektLeer = $derived(
    gemerkte.length === 0 && (projekt?.interneFristen ?? []).length === 0 && !abrechnung.sichtbar
  );

  function istDringend(tage) {
    return tage !== null && tage <= 14;
  }
  function anzahlText(n) {
    return n === 1 ? "1 offener Punkt" : `${n} offene Punkte`;
  }
</script>

<div class="bereich">
  {#if !projekt}
    <div class="leer">
      <h2>Willkommen bei Antrag 3000</h2>
      <p>Merkliste, Fristen und Abrechnung gehören immer zu einem Projekt.</p>
      <button class="primaer" onclick={projektErstellen}>Projekt erstellen</button>
    </div>
  {:else}
    <div class="kopf">
      <h2>{projekt.name}</h2>
      {#if !projektLeer}
        <p>
          {anzahlText(naechstes.gesamt)}{#if naechsterTermin}{` · nächster Termin ${tageText(tageBis(naechsterTermin.datum))}`}{/if}
        </p>
      {/if}
    </div>

    <div class="kacheln">
      {#if projektLeer}
        <section class="kachel">
          <h3>Erste Schritte</h3>
          <button class="zeile" onclick={() => zuZiel({ bereich: "foerderungen", ansicht: "passend" })}>
            <span class="nr">1</span>
            <span class="text">Fragebogen ausfüllen<small>Findet die Förderungen, die zu deinem Projekt passen</small></span>
          </button>
          <button class="zeile" onclick={() => zuZiel({ bereich: "foerderungen", ansicht: "alle" })}>
            <span class="nr">2</span>
            <span class="text">Förderungen merken<small>Ihre Fristen erscheinen dann im Kalender</small></span>
          </button>
          <button class="zeile" onclick={() => zuZiel({ bereich: "stammdaten" })}>
            <span class="nr">3</span>
            <span class="text">Stammdaten ergänzen<small>Name und Adresse für deine Anträge</small></span>
          </button>
        </section>
      {:else}
        <section class="kachel">
          <h3>Als Nächstes</h3>
          {#each naechstes.eintraege as e (e.id)}
            <button class="zeile" onclick={() => zuZiel(e.ziel)}>
              <span class="text">{e.titel}{#if e.untertitel}<small>{e.untertitel}</small>{/if}</span>
              {#if e.tage !== null}
                <span class="wert" class:dringend={istDringend(e.tage)}>{tageText(e.tage)}</span>
              {/if}
            </button>
          {:else}
            <p class="ruhe">Gerade ist nichts dringend.</p>
          {/each}
          {#if naechstes.gesamt > naechstes.eintraege.length}
            <p class="mehr">und {naechstes.gesamt - naechstes.eintraege.length} weitere</p>
          {/if}
        </section>
      {/if}

      <KalenderKachel {termine} {interneAnlegen} {zuZiel} />

      {#if antragStatus.length}
        <section class="kachel">
          <h3>Meine Anträge</h3>
          {#each antragStatus as s (s.key)}
            <button class="zeile" onclick={() => zuZiel({ bereich: "merkliste" })}>
              <span class="status-punkt farbe-{s.farbe}" aria-hidden="true"></span>
              <span class="text">{s.label}</span>
              <span class="wert">{s.anzahl}</span>
            </button>
          {/each}
        </section>
      {/if}

      {#if abrechnung.sichtbar}
        <section class="kachel">
          <h3>Abrechnung</h3>
          <button class="zeile" onclick={() => zuZiel({ bereich: "belege" })}>
            <span class="text">Belege noch nicht ganz zugeordnet</span>
            <span class="wert">{abrechnung.belegeOffen}</span>
          </button>
          {#each abrechnung.quellen as q (q.id)}
            <button class="zeile" onclick={() => zuZiel({ bereich: "zuteilung" })}>
              <span class="text">{q.name}</span>
              <span class="wert">{betragFormat(q.zugeordnet)} von {betragFormat(q.soll)}</span>
            </button>
          {/each}
        </section>
      {/if}
    </div>
  {/if}
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
  .kacheln {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(min(340px, 100%), 1fr));
    gap: 14px;
    align-items: start;
  }
  .kachel {
    min-width: 0;
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
  button.zeile {
    width: 100%;
    margin: 0;
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 8px 6px;
    font-family: inherit;
    font-size: 0.88rem;
    font-weight: 400;
    text-align: left;
    color: var(--text);
    background: none;
    border: none;
    border-top: 1px solid var(--flaeche-3);
    border-radius: 0;
    cursor: pointer;
  }
  button.zeile:hover {
    background: var(--flaeche-b);
  }
  .text {
    flex: 1;
    min-width: 0;
    overflow-wrap: anywhere;
  }
  small {
    display: block;
    margin-top: 1px;
    font-size: 0.78rem;
    color: var(--text-muted);
  }
  .wert {
    flex: none;
    font-size: 0.82rem;
    color: var(--text-muted);
    font-variant-numeric: tabular-nums;
  }
  .wert.dringend {
    color: var(--gefahr-text);
    font-weight: 600;
  }
  .nr {
    flex: none;
    width: 22px;
    height: 22px;
    line-height: 22px;
    text-align: center;
    font-size: 0.78rem;
    font-weight: 600;
    color: var(--akzent-text);
    background: var(--akzent-bg);
    border-radius: 50%;
  }
  .status-punkt {
    flex: none;
    width: 9px;
    height: 9px;
    border-radius: 50%;
  }
  .farbe-blau { background: var(--akzent); }
  .farbe-lila { background: var(--lila); }
  .farbe-gruen { background: var(--erfolg); }
  .farbe-rot { background: var(--gefahr); }
  .farbe-gelb { background: var(--warnung); }
  .farbe-grau { background: var(--grau-4); }
  .ruhe,
  .mehr {
    margin: 0;
    padding: 8px 6px 0;
    font-size: 0.86rem;
    color: var(--text-muted);
    border-top: 1px solid var(--flaeche-3);
  }
  .leer {
    max-width: 460px;
    margin: 40px auto;
    text-align: center;
  }
  .leer p {
    margin: 0 0 16px;
    color: var(--text-muted);
  }
  button.primaer {
    width: auto;
    margin: 0;
    padding: 10px 18px;
    font-family: inherit;
    font-size: 0.92rem;
    font-weight: 600;
    color: var(--auf-farbe);
    background: var(--akzent);
    border: none;
    border-radius: 8px;
    cursor: pointer;
  }
  button.primaer:hover {
    background: var(--akzent-d);
  }
</style>
