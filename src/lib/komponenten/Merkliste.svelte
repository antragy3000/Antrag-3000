<script>
  // Merkliste des aktiven Projekts als Listenansicht. Jede Zeile zeigt
  // die Förderung samt Antrag-Status UND der benötigten Dokumente mit
  // ihrem Status. Klick auf eine Zeile öffnet die Detailansicht mit dem
  // Status-/Checklisten-Block. Warnung bei unverträglichen Förderungen.
  import { openUrl } from "@tauri-apps/plugin-opener";
  import { sichereMailUrl } from "$lib/sicherheit";
  import FoerderDetail from "./FoerderDetail.svelte";
  import EigeneFoerderung from "./EigeneFoerderung.svelte";
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
    foerderungen = [],
    hinweis = "",
    merkliste,
    umschalten,
    ordnerOeffnen = null,
    dokumentHochladen = null,
    antragsPdfVorschau = null,
    antragsPdfSpeichern = null,
    antraege = {},
    antragHolen = null,
    antragSpeichern = null,
    eigeneAnlegen = null,
    oeffneKatalog = null,
    aktualisierteIds = [],
    hinweisVerwerfen = null,
    standFuer = null,
    neuFelderFuer = null,
    logoHerunterladen = null,
  } = $props();

  let ausgewaehlt = $state(null);
  let aktuellerAntrag = $state(null);
  let eigeneOffen = $state(false);

  // Wurde für die Förderung eines der angegebenen Felder geändert?
  function neu(id, ...keys) {
    const felder = neuFelderFuer ? neuFelderFuer(id) : [];
    return keys.some((k) => felder.includes(k));
  }

  let gemerkte = $derived(
    merkliste
      .map((id) => foerderungen.find((f) => f.id === id))
      .filter(Boolean)
  );

  // Hinweis-Zähler: gemerkte Förderungen, die nicht mehr im Katalog sind
  // bzw. deren Angaben ein Update geändert hat (nur solche, die noch
  // gemerkt sind).
  let entferntAnzahl = $derived(gemerkte.filter((f) => f.nichtMehrImKatalog).length);
  let aktualisiertAnzahl = $derived(
    (aktualisierteIds ?? []).filter((id) => merkliste.includes(id)).length
  );

  // Schließen sich zwei Förderungen gegenseitig aus?
  function unvertraeglich(a, b) {
    return (
      (a.unvertraeglich_mit ?? []).includes(b.id) ||
      (b.unvertraeglich_mit ?? []).includes(a.id)
    );
  }

  // Alle Paare gemerkter Förderungen, die sich gegenseitig ausschließen.
  let konflikte = $derived.by(() => {
    const paare = [];
    for (let i = 0; i < gemerkte.length; i++) {
      for (let j = i + 1; j < gemerkte.length; j++) {
        if (unvertraeglich(gemerkte[i], gemerkte[j])) {
          paare.push([gemerkte[i], gemerkte[j]]);
        }
      }
    }
    return paare;
  });

  // Namen der mit f unverträglichen, ebenfalls gemerkten Förderungen.
  function konfliktNamen(f) {
    return gemerkte.filter((g) => g.id !== f.id && unvertraeglich(f, g)).map((g) => g.name);
  }

  // Antrag-Status-Etikett einer Förderung.
  function badgeFuer(id) {
    const a = antraege[id];
    const key = a?.status ?? ANTRAG_STANDARD;
    return {
      label: statusLabel(ANTRAG_STATUS, key, a?.statusFrei),
      farbe: statusFarbe(ANTRAG_STATUS, key),
    };
  }

  // Checkliste zum Anzeigen: sobald ein Antrag existiert, seine (eigene)
  // Liste – auch wenn sie leer ist (man darf alle Dokumente entfernen).
  // Nur wenn es noch GAR keinen Antrag gibt, zeigen wir die Vorschläge.
  function checklisteFuer(f) {
    const a = antraege[f.id];
    if (a?.checkliste) return a.checkliste;
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

  // Kontaktperson / E-Mail einer Förderung (aus dem Antrag-Eintrag).
  function kontaktName(id) {
    return (antraege[id]?.kontakt?.ansprechpartner ?? "").trim();
  }
  function kontaktEmail(id) {
    return (antraege[id]?.kontakt?.email ?? "").trim();
  }
  function mailSchreiben(id) {
    // Adresse absichern, statt sie roh hinter „mailto:" zu hängen.
    const mailto = sichereMailUrl(kontaktEmail(id));
    if (mailto) openUrl(mailto);
  }

  // Fristen zur Anzeige in der Zeile: offizielle (editierbare Übernahme
  // aus der Datenbank) plus benannte eigene Fristen. Jede einzeln, mit
  // Resttagen für die Rotfärbung.
  const HEUTE = new Date();
  HEUTE.setHours(0, 0, 0, 0);
  function tageBis(d) {
    return Math.round((new Date(d) - HEUTE) / 86400000);
  }
  function fmtDatum(d) {
    return new Date(d).toLocaleDateString("de-DE");
  }
  function fristenFuerListe(f) {
    const a = antraege[f.id];
    const offiziell = (a?.offizielleFristen ?? f.fristen ?? []).filter(Boolean);
    const eigene = a?.eigeneFristen ?? [];
    const eintraege = [];
    for (const d of offiziell) eintraege.push({ label: "Frist", datum: d, tage: tageBis(d) });
    for (const e of eigene) {
      const datum = typeof e === "string" ? e : e.datum;
      const titel = typeof e === "string" ? "" : e.titel;
      eintraege.push({ label: titel || "Eigene Frist", datum, tage: tageBis(datum) });
    }
    return eintraege;
  }
</script>

<div class="bereich">
  <div class="kopfzeile">
    <h2>Merkliste <span class="anzahl">{gemerkte.length}</span></h2>
    <div class="kopf-knoepfe">
      {#if oeffneKatalog}
        <button class="ordner" onclick={oeffneKatalog}>
          🗂 Förder-Datenbank
        </button>
      {/if}
      {#if eigeneAnlegen}
        <button class="ordner" onclick={() => (eigeneOffen = true)}>
          + Eigene Förderung
        </button>
      {/if}
      {#if ordnerOeffnen}
        <button class="ordner" onclick={() => ordnerOeffnen(null)}>
          📁 Projektordner öffnen
        </button>
      {/if}
    </div>
  </div>

  {#if entferntAnzahl > 0 || aktualisiertAnzahl > 0}
    <div class="katalog-hinweis">
      <div class="kh-text">
        {#if entferntAnzahl > 0}
          <div class="kh-zeile weg">
            <strong>⚠ {entferntAnzahl}</strong>
            {entferntAnzahl === 1 ? "gemerkte Förderung ist" : "gemerkte Förderungen sind"}
            <strong>nicht mehr im Katalog</strong> (unten rot markiert).
          </div>
        {/if}
        {#if aktualisiertAnzahl > 0}
          <div class="kh-zeile akt">
            <strong>ℹ {aktualisiertAnzahl}</strong>
            {aktualisiertAnzahl === 1 ? "gemerkte Förderung wurde" : "gemerkte Förderungen wurden"}
            durch ein Update <strong>aktualisiert</strong> – prüfe die Angaben.
          </div>
        {/if}
      </div>
      {#if aktualisiertAnzahl > 0 && hinweisVerwerfen}
        <button class="kh-ok" onclick={hinweisVerwerfen}>OK, verstanden</button>
      {/if}
    </div>
  {/if}

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
      in der Übersicht oder im Matching-Ergebnis – oder lege oben eine
      <strong>eigene Förderung</strong> an.
    </p>
  {:else}
    <div class="liste">
      {#each gemerkte as f (f.id)}
        {@const badge = badgeFuer(f.id)}
        <div
          class="zeile"
          class:weg={f.nichtMehrImKatalog}
          class:akt={aktualisierteIds.includes(f.id)}
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
          <div class="aktionen">
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
            <button
              class="mail"
              class:aus={!kontaktEmail(f.id)}
              title={kontaktEmail(f.id)
                ? "E-Mail an " + (kontaktName(f.id) || "Kontaktperson") + " schreiben"
                : "noch keine Emailadresse eingetragen"}
              aria-label="E-Mail an Kontaktperson"
              onclick={(e) => {
                e.stopPropagation();
                mailSchreiben(f.id);
              }}
            >
              ✉
            </button>
          </div>

          <div class="haupt">
            <div class="kopf">
              <span class="land land-{f.land}">{LAENDER[f.land] ?? f.land}</span>
              <h3>{f.name}{#if neu(f.id, "name")}<span class="neu-feld">NEU</span>{/if}</h3>
              <span class="status-badge farbe-{badge.farbe}">
                <span class="punkt"></span>{badge.label}
              </span>
              {#if f.nichtMehrImKatalog}
                <span class="herkunft weg">⚠ nicht mehr im Katalog</span>
              {:else if f.eigen}
                <span class="herkunft selbst">✎ selbst eingetragen</span>
              {:else if f.geteilt}
                <span class="herkunft geteilt">👥 vom Team geteilt</span>
              {/if}
              {#if aktualisierteIds.includes(f.id)}
                <span class="herkunft akt">✦ aktualisiert</span>
              {/if}
            </div>

            <p class="meta">
              <span class="meta-links">{f.foerdergeber}{#if neu(f.id, "foerdergeber")}<span class="neu-feld">NEU</span>{/if} · <span class="hoehe">{f.foerderhoehe_text}</span>{#if neu(f.id, "foerderhoehe_text")}<span class="neu-feld">NEU</span>{/if}</span>
              {#if standFuer && standFuer(f.id)}
                <span class="stand">aktualisiert {standFuer(f.id)}</span>
              {/if}
            </p>

            <div class="tag-zeile">
              <div class="chips">
                {#each f.weiche_kriterien.sparten as sp}
                  <span class="chip">{SPARTEN[sp] ?? sp}</span>
                {/each}
                {#if neu(f.id, "weiche_kriterien.sparten", "weiche_kriterien.projektarten")}
                  <span class="neu-feld">NEU</span>
                {/if}
              </div>
              <div class="fristen-rechts">
                {#each fristenFuerListe(f) as fr (fr.label + fr.datum)}
                  <span class="frist" class:dringend={fr.tage <= 14}>
                    {fr.label}: {fmtDatum(fr.datum)}
                  </span>
                {:else}
                  <span class="frist">{fristText(f)}</span>
                {/each}
                {#if neu(f.id, "fristen", "weiche_kriterien.zeitpunkt")}
                  <span class="neu-feld">NEU</span>
                {/if}
              </div>
            </div>

            <p class="kontaktperson" class:leer-kontakt={!kontaktName(f.id)}>
              👤 {kontaktName(f.id) || "keine Kontaktperson"}
            </p>

            {#if konfliktNamen(f).length}
              <p class="konflikt-zeile">
                ⚠ Unverträglich mit: {konfliktNamen(f).join(", ")}
              </p>
            {/if}

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

{#if eigeneOffen}
  <EigeneFoerderung
    anlegen={eigeneAnlegen}
    schliessen={() => (eigeneOffen = false)}
  />
{/if}

{#if ausgewaehlt}
  <FoerderDetail
    foerderung={ausgewaehlt}
    alle={foerderungen}
    hinweis={hinweis}
    gemerkt={merkliste.includes(ausgewaehlt.id)}
    umschalten={umschalten}
    ordnerOeffnen={ordnerOeffnen ? () => ordnerOeffnen(ausgewaehlt.name) : null}
    hochladen={dokumentHochladen ? (art) => dokumentHochladen(ausgewaehlt.name, art) : null}
    pdfVorschau={antragsPdfVorschau ? () => antragsPdfVorschau(ausgewaehlt) : null}
    pdfSpeichern={antragsPdfSpeichern ? () => antragsPdfSpeichern(ausgewaehlt) : null}
    antrag={aktuellerAntrag}
    antragAendern={antragSpeichern}
    stand={standFuer ? standFuer(ausgewaehlt.id) : null}
    geaenderteFelder={neuFelderFuer ? neuFelderFuer(ausgewaehlt.id) : []}
    {logoHerunterladen}
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
    color: var(--text-muted);
    font-size: 0.9rem;
    font-weight: 400;
    margin-left: 8px;
  }
  .kopf-knoepfe {
    display: flex;
    gap: 10px;
    flex-wrap: wrap;
  }
  .ordner {
    padding: 9px 16px;
    font-size: 0.9rem;
    font-weight: 600;
    font-family: inherit;
    color: var(--text);
    background: var(--weiss);
    border: 2px solid var(--rand);
    border-radius: 8px;
    cursor: pointer;
  }
  .ordner:hover {
    border-color: var(--akzent);
  }

  .konflikt {
    background: var(--gefahr-bg);
    border-left: 4px solid var(--gefahr);
    border-radius: 8px;
    padding: 14px 16px;
    margin-bottom: 16px;
    font-size: 0.92rem;
    line-height: 1.5;
    color: var(--gefahr-d2);
  }

  .leer {
    color: var(--text-muted);
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
    background: var(--weiss);
    border-radius: 12px;
    box-shadow: 0 1px 3px var(--schatten-sm);
    padding: 16px 20px;
    cursor: pointer;
    transition: box-shadow 0.15s;
  }
  .zeile:hover {
    box-shadow: 0 4px 14px var(--schatten-md);
  }
  .zeile:focus-visible {
    outline: 2px solid var(--akzent);
    outline-offset: 2px;
  }
  .zeile.weg {
    border-left: 5px solid var(--gefahr);
    background: var(--gefahr-bg4);
  }
  .zeile.akt {
    border-left: 5px solid var(--akzent);
    background: var(--flaeche-blau);
  }

  .katalog-hinweis {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 14px;
    flex-wrap: wrap;
    background: var(--warnung-bg4);
    border: 1px solid var(--warnung-rand3);
    border-radius: 10px;
    padding: 12px 16px;
    margin-bottom: 16px;
  }
  .kh-text { display: flex; flex-direction: column; gap: 4px; }
  .kh-zeile { font-size: 0.92rem; line-height: 1.45; }
  .kh-zeile.weg { color: var(--gefahr-text); }
  .kh-zeile.akt { color: var(--warnung-text4); }
  .kh-ok {
    background: var(--weiss);
    border: 2px solid var(--warnung-rand2);
    border-radius: 8px;
    padding: 7px 13px;
    font-size: 0.85rem;
    font-weight: 600;
    font-family: inherit;
    color: var(--warnung-text4);
    cursor: pointer;
    white-space: nowrap;
  }
  .kh-ok:hover { background: var(--warnung-bg5); }

  .aktionen {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 6px;
    flex-shrink: 0;
  }
  .stern {
    background: none;
    border: none;
    font-size: 1.2rem;
    line-height: 1;
    color: var(--warnung);
    cursor: pointer;
    padding: 2px 4px;
    border-radius: 6px;
    height: fit-content;
  }
  .stern:hover {
    background: var(--warnung-bg2);
  }
  .mail {
    background: none;
    border: none;
    font-size: 1.05rem;
    line-height: 1;
    color: var(--akzent);
    cursor: pointer;
    padding: 3px 4px;
    border-radius: 6px;
    height: fit-content;
  }
  .mail:hover {
    background: var(--akzent-bg);
  }
  .mail.aus {
    color: var(--grau-3);
    cursor: not-allowed;
  }
  .mail.aus:hover {
    background: none;
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
    color: var(--text-muted);
    font-size: 0.85rem;
    display: flex;
    justify-content: space-between;
    align-items: baseline;
    gap: 12px;
  }
  .meta-links { min-width: 0; }
  .hoehe {
    color: var(--erfolg-text);
    font-weight: 600;
  }

  .tag-zeile {
    display: flex;
    align-items: flex-start;
    gap: 12px;
    margin-top: 8px;
  }
  .chips {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    flex: 1;
    min-width: 0;
  }
  .chip {
    font-size: 0.75rem;
    background: var(--flaeche-2b);
    color: var(--text-2);
    padding: 3px 9px;
    border-radius: 99px;
  }
  .fristen-rechts {
    display: flex;
    flex-direction: column;
    align-items: flex-end;
    gap: 2px;
    flex-shrink: 0;
    padding-top: 2px;
  }
  .frist {
    font-size: 1rem;
    font-weight: 600;
    color: var(--text-muted);
    white-space: nowrap;
  }
  .frist.dringend {
    color: var(--gefahr-text);
    font-weight: 700;
  }
  .kontaktperson {
    margin: 8px 0 0;
    font-size: 0.82rem;
    color: var(--text-2);
  }
  .kontaktperson.leer-kontakt {
    color: var(--grau-4);
  }
  .konflikt-zeile {
    margin: 8px 0 0;
    font-size: 0.82rem;
    font-weight: 600;
    color: var(--gefahr-text);
  }

  .land {
    font-size: 0.72rem;
    font-weight: 700;
    letter-spacing: 0.03em;
    padding: 3px 9px;
    border-radius: 99px;
    text-transform: uppercase;
  }
  .land-DE { background: var(--akzent-bg3); color: var(--link); }
  .land-AT { background: var(--gefahr-bg); color: var(--gefahr-text); }
  .land-CH { background: var(--warnung-bg); color: var(--warnung-text); }
  .land-INT { background: var(--lila-bg); color: var(--lila-d); }
  .land-ANDERES { background: var(--flaeche-2b); color: var(--text-2); }

  /* Dokumente mit Status */
  .dokumente {
    margin-top: 12px;
    padding-top: 10px;
    border-top: 1px solid var(--flaeche-2b);
  }
  .dok-titel {
    font-size: 0.78rem;
    font-weight: 600;
    color: var(--text-leise);
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
    color: var(--text);
  }
  .dok-text.fertig {
    color: var(--text-muted);
    text-decoration: line-through;
  }
  .dok-status {
    color: var(--text-muted);
    font-size: 0.8rem;
  }
  .dok-status::before {
    content: "– ";
  }
  .dok-leer {
    color: var(--text-leise);
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
  .farbe-blau { background: var(--akzent); }
  .farbe-lila { background: var(--lila); }
  .farbe-gruen { background: var(--erfolg); }
  .farbe-rot { background: var(--gefahr); }
  .farbe-gelb { background: var(--warnung); }
  .farbe-grau { background: var(--grau-4); }

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
  .herkunft {
    font-size: 0.72rem;
    font-weight: 600;
    padding: 2px 8px;
    border-radius: 99px;
  }
  .herkunft.selbst { background: var(--akzent-bg); color: var(--akzent-d4); }
  .herkunft.geteilt { background: var(--akzent-bg5); color: var(--link-d3); }
  .herkunft.akt {
    font-size: 0.76rem;
    font-weight: 700;
    padding: 3px 10px;
    background: var(--akzent-bg2);
    color: var(--akzent-d3);
    border: 1px solid var(--akzent-rand);
  }
  .herkunft.weg {
    font-size: 0.76rem;
    font-weight: 700;
    padding: 3px 10px;
    background: var(--gefahr-bg);
    color: var(--gefahr-text);
    border: 1px solid var(--gefahr-rand);
  }
  .stand { color: var(--grau-5); font-size: 0.82rem; white-space: nowrap; }
  .neu-feld {
    display: inline-block;
    margin-left: 6px;
    font-size: 0.62rem;
    font-weight: 700;
    letter-spacing: 0.04em;
    vertical-align: middle;
    padding: 1px 6px;
    border-radius: 99px;
    background: var(--akzent-bg2);
    color: var(--akzent-d3);
    border: 1px solid var(--akzent-rand);
  }
  .status-badge.farbe-blau { background: var(--akzent-bg2); color: var(--akzent-d3); }
  .status-badge.farbe-lila { background: var(--lila-bg2); color: var(--lila-d2); }
  .status-badge.farbe-gruen { background: var(--erfolg-bg); color: var(--erfolg-text); }
  .status-badge.farbe-rot { background: var(--gefahr-bg); color: var(--gefahr-text); }
  .status-badge.farbe-gelb { background: var(--warnung-bg); color: var(--warnung-text); }
  .status-badge.farbe-grau { background: var(--flaeche-2b); color: var(--text-2); }
  .status-badge .punkt { width: 8px; height: 8px; }
  .status-badge.farbe-blau .punkt { background: var(--akzent); }
  .status-badge.farbe-lila .punkt { background: var(--lila); }
  .status-badge.farbe-gruen .punkt { background: var(--erfolg); }
  .status-badge.farbe-rot .punkt { background: var(--gefahr); }
  .status-badge.farbe-gelb .punkt { background: var(--warnung); }
  .status-badge.farbe-grau .punkt { background: var(--grau-4); }
</style>
