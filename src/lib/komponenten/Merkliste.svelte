<script>
  // Merkliste des aktiven Projekts als Listenansicht. Jede Zeile zeigt
  // die Förderung samt Antrag-Status UND der benötigten Dokumente mit
  // ihrem Status. Klick auf eine Zeile öffnet die Detailansicht mit dem
  // Status-/Checklisten-Block. Warnung bei unverträglichen Förderungen.
  import datenbank from "$lib/daten/foerderungen.json";
  import FoerderDetail from "./FoerderDetail.svelte";
  import { LAENDER, SPARTEN, fristText } from "$lib/begriffe";
  import {
    ANTRAG_STATUS,
    ANTRAG_STANDARD,
    CHECK_STATUS,
    CHECK_STANDARD,
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

  let gemerkte = $derived(
    merkliste
      .map((id) => datenbank.foerderungen.find((f) => f.id === id))
      .filter(Boolean)
  );

  // Alle Paare gemerkter Förderungen, die sich gegenseitig ausschließen.
  let konflikte = $derived.by(() => {
    const paare = [];
    for (let i = 0; i < gemerkte.length; i++) {
      for (let j = i + 1; j < gemerkte.length; j++) {
        const a = gemerkte[i];
        const b = gemerkte[j];
        if (a.unvertraeglich_mit.includes(b.id) || b.unvertraeglich_mit.includes(a.id)) {
          paare.push([a, b]);
        }
      }
    }
    return paare;
  });

  // Antrag-Status-Etikett einer Förderung.
  function badgeFuer(id) {
    const a = antraege[id];
    const key = a?.status ?? ANTRAG_STANDARD;
    return {
      label: statusLabel(ANTRAG_STATUS, key, a?.statusFrei),
      farbe: statusFarbe(ANTRAG_STATUS, key),
    };
  }

  // Checkliste zum Anzeigen: bearbeiteter Stand oder Vorschlag der Förderung.
  function checklisteFuer(f) {
    const a = antraege[f.id];
    if (a?.checkliste?.length) return a.checkliste;
    return (f.checkliste_vorschlag ?? []).map((t) => ({
      text: t,
      status: CHECK_STANDARD,
      statusFrei: "",
    }));
  }

  function oeffnen(f) {
    aktuellerAntrag = antragHolen ? antragHolen(f) : null;
    ausgewaehlt = f;
  }
</script>

<div class="bereich">
  <div class="kopfzeile">
    <h2>Merkliste <span class="anzahl">{gemerkte.length}</span></h2>
    {#if ordnerOeffnen}
      <button class="ordner" onclick={() => ordnerOeffnen(null)}>
        📁 Projektordner öffnen
      </button>
    {/if}
  </div>

  {#each konflikte as [a, b]}
    <div class="konflikt">
      <strong>⚠ Unverträglich:</strong>
      „{a.name}" ({a.foerdergeber}) und „{b.name}" ({b.foerdergeber})
      schließen sich gegenseitig aus. Für die Antragstellung musst du
      dich für eine der beiden entscheiden.
    </div>
  {/each}

  {#if gemerkte.length === 0}
    <p class="leer">
      Noch nichts gemerkt. Markiere Förderungen mit dem ☆-Stern –
      in der Übersicht oder im Matching-Ergebnis.
    </p>
  {:else}
    <div class="liste">
      {#each gemerkte as f (f.id)}
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
          <button
            class="stern"
            title="Von der Merkliste entfernen"
            aria-label="Von der Merkliste entfernen"
            onclick={(e) => {
              e.stopPropagation();
              umschalten(f.id);
            }}
          >
            ★
          </button>

          <div class="haupt">
            <div class="kopf">
              <span class="land land-{f.land}">{LAENDER[f.land] ?? f.land}</span>
              <h3>{f.name}</h3>
              <span class="status-badge farbe-{badge.farbe}">
                <span class="punkt"></span>{badge.label}
              </span>
            </div>

            <p class="meta">
              {f.foerdergeber} · <span class="hoehe">{f.foerderhoehe_text}</span> · {fristText(f)}
            </p>

            <div class="chips">
              {#each f.weiche_kriterien.sparten as sp}
                <span class="chip">{SPARTEN[sp] ?? sp}</span>
              {/each}
            </div>

            <div class="dokumente">
              <span class="dok-titel">Benötigte Dokumente:</span>
              <ul>
                {#each checklisteFuer(f) as p (p.text)}
                  <li>
                    <span class="punkt farbe-{statusFarbe(CHECK_STATUS, p.status)}"></span>
                    <span class="dok-text" class:fertig={p.status === "abgeschlossen"}>
                      {p.text}
                    </span>
                    <span class="dok-status">{statusLabel(CHECK_STATUS, p.status, p.statusFrei)}</span>
                  </li>
                {:else}
                  <li class="dok-leer">noch keine Dokumente</li>
                {/each}
              </ul>
            </div>
          </div>
        </div>
      {/each}
    </div>
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

  .kopfzeile {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 16px;
    flex-wrap: wrap;
    margin-bottom: 20px;
  }
  h2 {
    margin: 0;
    font-size: 1.35rem;
    font-weight: 600;
  }
  .anzahl {
    color: #5e6c84;
    font-size: 0.9rem;
    font-weight: 400;
    margin-left: 8px;
  }
  .ordner {
    padding: 9px 16px;
    font-size: 0.9rem;
    font-weight: 600;
    font-family: inherit;
    color: #172b4d;
    background: #fff;
    border: 2px solid #dfe1e6;
    border-radius: 8px;
    cursor: pointer;
  }
  .ordner:hover {
    border-color: #4f6df5;
  }

  .konflikt {
    background: #ffeceb;
    border-left: 4px solid #ca3521;
    border-radius: 8px;
    padding: 14px 16px;
    margin-bottom: 16px;
    font-size: 0.92rem;
    line-height: 1.5;
    color: #5d1f1a;
  }

  .leer {
    color: #5e6c84;
  }

  /* Listenansicht */
  .liste {
    display: flex;
    flex-direction: column;
    gap: 12px;
  }
  .zeile {
    display: flex;
    gap: 14px;
    background: #fff;
    border-radius: 12px;
    box-shadow: 0 1px 3px rgba(9, 30, 66, 0.12);
    padding: 16px 20px;
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

  .stern {
    background: none;
    border: none;
    font-size: 1.2rem;
    line-height: 1;
    color: #e2a400;
    cursor: pointer;
    padding: 2px 4px;
    border-radius: 6px;
    height: fit-content;
    flex-shrink: 0;
  }
  .stern:hover {
    background: #fffaf0;
  }

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
    flex: 1;
    min-width: 0;
  }

  .meta {
    margin: 6px 0 0;
    color: #5e6c84;
    font-size: 0.85rem;
  }
  .hoehe {
    color: #216e4e;
    font-weight: 600;
  }

  .chips {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    margin-top: 8px;
  }
  .chip {
    font-size: 0.75rem;
    background: #f1f2f4;
    color: #44546f;
    padding: 3px 9px;
    border-radius: 99px;
  }

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

  /* Dokumente mit Status */
  .dokumente {
    margin-top: 12px;
    padding-top: 10px;
    border-top: 1px solid #f1f2f4;
  }
  .dok-titel {
    font-size: 0.78rem;
    font-weight: 600;
    color: #8590a2;
    text-transform: uppercase;
    letter-spacing: 0.03em;
  }
  .dokumente ul {
    list-style: none;
    margin: 8px 0 0;
    padding: 0;
    display: grid;
    gap: 5px;
  }
  .dokumente li {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 0.85rem;
  }
  .dok-text {
    color: #172b4d;
  }
  .dok-text.fertig {
    color: #5e6c84;
    text-decoration: line-through;
  }
  .dok-status {
    color: #5e6c84;
    font-size: 0.8rem;
  }
  .dok-status::before {
    content: "– ";
  }
  .dok-leer {
    color: #8590a2;
    font-size: 0.82rem;
  }

  /* farbiger Statuspunkt */
  .punkt {
    width: 9px;
    height: 9px;
    border-radius: 50%;
    flex-shrink: 0;
    display: inline-block;
  }
  .farbe-blau { background: #4f6df5; }
  .farbe-lila { background: #8270db; }
  .farbe-gruen { background: #22a06b; }
  .farbe-rot { background: #ca3521; }
  .farbe-gelb { background: #e2a400; }
  .farbe-grau { background: #b3bac5; }

  /* Antrag-Status-Etikett */
  .status-badge {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 3px 10px 3px 8px;
    border-radius: 99px;
    font-size: 0.78rem;
    font-weight: 600;
  }
  .status-badge.farbe-blau { background: #e9f0ff; color: #2b46c4; }
  .status-badge.farbe-lila { background: #f1edff; color: #5e44b0; }
  .status-badge.farbe-gruen { background: #dcfff1; color: #216e4e; }
  .status-badge.farbe-rot { background: #ffeceb; color: #ae2e24; }
  .status-badge.farbe-gelb { background: #fff7d6; color: #7f5f01; }
  .status-badge.farbe-grau { background: #f1f2f4; color: #44546f; }
  .status-badge .punkt { width: 8px; height: 8px; }
  .status-badge.farbe-blau .punkt { background: #4f6df5; }
  .status-badge.farbe-lila .punkt { background: #8270db; }
  .status-badge.farbe-gruen .punkt { background: #22a06b; }
  .status-badge.farbe-rot .punkt { background: #ca3521; }
  .status-badge.farbe-gelb .punkt { background: #e2a400; }
  .status-badge.farbe-grau .punkt { background: #b3bac5; }
</style>
