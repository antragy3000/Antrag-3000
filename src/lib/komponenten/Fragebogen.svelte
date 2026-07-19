<script>
  // Eingangs-Fragebogen (Multiple Choice). Eine Frage pro Karte.
  // Liefert am Ende die Antworten an den Aufrufer (fertig-Callback);
  // gespeichert wird dort - im Tresor, denn die Budgetangabe ist sensibel.
  import { SPARTEN, PROJEKTARTEN } from "$lib/begriffe";
  import { sucheRegionen, sucheStaedte, regionName } from "$lib/daten/orte.js";
  import SuchAuswahl from "./SuchAuswahl.svelte";

  let { start = null, fertig, abbrechen = null } = $props();

  // Länder mit engerer Auswahl (Bundesland/Kanton + Stadt).
  const DACH = ["DE", "AT", "CH"];
  const istDach = (land) => DACH.includes(land);

  const FRAGEN = [
    {
      key: "wohnsitz",
      frage: "Wo hast du deinen Wohnsitz?",
      ort: true,
      regionKey: "wohnsitzRegion",
      stadtKey: "wohnsitzStadt",
      mehrfach: false,
      optionen: [
        { wert: "DE", label: "Deutschland" },
        { wert: "AT", label: "Österreich" },
        { wert: "CH", label: "Schweiz" },
        { wert: "ANDERES", label: "anderes Land" },
      ],
    },
    {
      key: "durchfuehrungsort",
      frage: "Wo findet dein Projekt überwiegend statt?",
      ort: true,
      regionKey: "durchfuehrungRegion",
      stadtKey: "durchfuehrungStadt",
      mehrfach: false,
      optionen: [
        { wert: "DE", label: "Deutschland" },
        { wert: "AT", label: "Österreich" },
        { wert: "CH", label: "Schweiz" },
        { wert: "INT", label: "International / im Ausland" },
      ],
    },
    {
      key: "traegerschaft",
      frage: "Wer stellt den Antrag?",
      mehrfach: false,
      optionen: [
        { wert: "einzelperson", label: "Ich als Einzelperson" },
        { wert: "gruppe", label: "Wir als Gruppe / GbR" },
        { wert: "organisation", label: "Ein Verein / eine Organisation" },
      ],
    },
    {
      key: "studentisch",
      frage: "Bist du derzeit Student:in?",
      hinweis: "Manche Förderungen schließen Studierende aus.",
      mehrfach: false,
      optionen: [
        { wert: true, label: "Ja" },
        { wert: false, label: "Nein" },
      ],
    },
    {
      key: "sparten",
      frage: "In welchen Sparten bewegt sich dein Projekt?",
      hinweis: "Mehrfachauswahl möglich.",
      mehrfach: true,
      optionen: Object.entries(SPARTEN).map(([wert, label]) => ({ wert, label })),
    },
    {
      key: "projektarten",
      frage: "Was für ein Vorhaben ist es?",
      hinweis: "Mehrfachauswahl möglich.",
      mehrfach: true,
      optionen: Object.entries(PROJEKTARTEN).map(([wert, label]) => ({ wert, label })),
    },
    {
      key: "budget",
      frage: "Wie groß ist das Projektbudget ungefähr?",
      hinweis: "In Euro bzw. Franken. Diese Angabe wird verschlüsselt im Tresor gespeichert.",
      mehrfach: false,
      optionen: [
        { wert: "unter_5000", label: "unter 5.000" },
        { wert: "von_5000_bis_15000", label: "5.000 – 15.000" },
        { wert: "von_15000_bis_50000", label: "15.000 – 50.000" },
        { wert: "ueber_50000", label: "über 50.000" },
      ],
    },
    {
      key: "zeitpunkt",
      frage: "Wann möchtest du einreichen?",
      mehrfach: false,
      optionen: [
        { wert: "bald", label: "in den nächsten 3 Monaten" },
        { wert: "mittel", label: "in 3 – 6 Monaten" },
        { wert: "spaeter", label: "später" },
        { wert: "flexibel", label: "ich bin flexibel" },
      ],
    },
  ];

  let index = $state(0);
  let antworten = $state(
    start ? structuredClone($state.snapshot(start)) : { sparten: [], projektarten: [] }
  );

  let frage = $derived(FRAGEN[index]);
  let letzte = $derived(index === FRAGEN.length - 1);

  function weiter() {
    if (letzte) {
      fertig(structuredClone($state.snapshot(antworten)));
    } else {
      index += 1;
    }
  }

  function einfachWaehlen(wert) {
    antworten[frage.key] = wert;
    weiter();
  }

  // Orts-Frage: Land wählen, aber NICHT automatisch weiter (es folgen
  // optional Region/Stadt). Beim Landwechsel die engere Auswahl leeren.
  function landWaehlen(wert) {
    antworten[frage.key] = wert;
    antworten[frage.regionKey] = null;
    antworten[frage.stadtKey] = null;
  }
  function regionGewaehlt(o) {
    antworten[frage.regionKey] = o ? o.wert : null;
    antworten[frage.stadtKey] = null; // Stadt hängt an der Region
  }
  function stadtGewaehlt(o) {
    antworten[frage.stadtKey] = o ? o.wert : null;
  }

  function mehrfachUmschalten(wert) {
    const liste = antworten[frage.key];
    antworten[frage.key] = liste.includes(wert)
      ? liste.filter((x) => x !== wert)
      : [...liste, wert];
  }
</script>

<div class="buehne">
  <div class="karte">
    <p class="schritt">Frage {index + 1} von {FRAGEN.length}</p>
    <h2>{frage.frage}</h2>
    {#if frage.hinweis}<p class="hinweis">{frage.hinweis}</p>{/if}

    {#if frage.ort}
      <div class="optionen kompakt">
        {#each frage.optionen as o (String(o.wert))}
          <button
            class="option"
            class:gewaehlt={antworten[frage.key] === o.wert}
            onclick={() => landWaehlen(o.wert)}
          >
            {o.label}
          </button>
        {/each}
      </div>

      {#if istDach(antworten[frage.key])}
        <div class="ort-felder">
          <label for="ort-region">Bundesland / Kanton <span class="opt">(optional)</span></label>
          <SuchAuswahl
            platzhalter="z. B. Bayern, Zürich, Tirol …"
            label={regionName(antworten[frage.key], antworten[frage.regionKey])}
            suche={(t) =>
              sucheRegionen(antworten[frage.key], t).map((r) => ({ wert: r.code, label: r.name }))}
            onwaehlen={regionGewaehlt}
          />
          <label for="ort-stadt">Stadt <span class="opt">(optional)</span></label>
          <SuchAuswahl
            platzhalter="Stadt suchen …"
            label={antworten[frage.stadtKey] ?? ""}
            suche={(t) =>
              sucheStaedte(antworten[frage.key], antworten[frage.regionKey], t).map((s) => ({
                wert: s.name,
                label: s.name,
              }))}
            onwaehlen={stadtGewaehlt}
          />
        </div>
      {/if}

      <button class="primaer" disabled={!antworten[frage.key]} onclick={weiter}>
        {letzte ? "Ergebnis anzeigen" : "Weiter"}
      </button>
    {:else}
      <div class="optionen" class:kompakt={frage.optionen.length > 5}>
        {#each frage.optionen as o (String(o.wert))}
          {#if frage.mehrfach}
            <button
              class="option"
              class:gewaehlt={antworten[frage.key].includes(o.wert)}
              onclick={() => mehrfachUmschalten(o.wert)}
            >
              {o.label}
            </button>
          {:else}
            <button
              class="option"
              class:gewaehlt={antworten[frage.key] === o.wert}
              onclick={() => einfachWaehlen(o.wert)}
            >
              {o.label}
            </button>
          {/if}
        {/each}
      </div>

      {#if frage.mehrfach}
        <button class="primaer" disabled={antworten[frage.key].length === 0} onclick={weiter}>
          {letzte ? "Ergebnis anzeigen" : "Weiter"}
        </button>
      {/if}
    {/if}

    <div class="fusszeile">
      {#if index > 0}
        <button class="leise" onclick={() => (index -= 1)}>← Zurück</button>
      {/if}
      {#if abbrechen}
        <button class="leise" onclick={abbrechen}>Abbrechen</button>
      {/if}
    </div>
  </div>
</div>

<style>
  .buehne {
    display: grid;
    place-items: center;
    padding: 48px 24px;
  }
  .karte {
    background: var(--weiss);
    border-radius: 12px;
    box-shadow: 0 1px 3px var(--schatten-sm), 0 8px 24px var(--schatten-xs);
    padding: 40px;
    width: 100%;
    max-width: 520px;
    box-sizing: border-box;
  }

  .schritt {
    margin: 0 0 8px;
    font-size: 0.8rem;
    font-weight: 700;
    letter-spacing: 0.04em;
    text-transform: uppercase;
    color: var(--text-leise);
  }
  h2 {
    margin: 0 0 6px;
    font-size: 1.3rem;
    font-weight: 600;
  }
  .hinweis {
    margin: 0 0 12px;
    color: var(--text-muted);
    font-size: 0.88rem;
  }

  .optionen {
    display: flex;
    flex-direction: column;
    gap: 10px;
    margin-top: 18px;
  }
  .optionen.kompakt {
    display: grid;
    grid-template-columns: 1fr 1fr;
  }

  .option {
    text-align: left;
    padding: 12px 16px;
    font-size: 0.98rem;
    font-weight: 500;
    color: var(--text);
    background: var(--flaeche);
    border: 2px solid var(--rand);
    border-radius: 10px;
    cursor: pointer;
    transition: border-color 0.12s, background 0.12s;
    font-family: inherit;
  }
  .option:hover {
    border-color: var(--akzent);
  }
  .option.gewaehlt {
    border-color: var(--akzent);
    background: var(--akzent-bg);
  }

  .ort-felder {
    margin-top: 18px;
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .ort-felder label {
    font-size: 0.82rem;
    font-weight: 600;
    color: var(--text-muted);
    margin: 10px 0 2px;
  }
  .ort-felder .opt { color: var(--text-leise); font-weight: 400; }

  .primaer {
    width: 100%;
    margin-top: 20px;
    padding: 11px;
    font-size: 1rem;
    font-weight: 600;
    color: var(--auf-farbe);
    background: var(--akzent);
    border: none;
    border-radius: 8px;
    cursor: pointer;
  }
  .primaer:hover:not(:disabled) {
    background: var(--akzent-d);
  }
  .primaer:disabled {
    background: var(--grau-3);
    cursor: default;
  }

  .fusszeile {
    display: flex;
    justify-content: space-between;
    margin-top: 14px;
  }
  .leise {
    background: none;
    border: none;
    color: var(--text-muted);
    font-size: 0.875rem;
    cursor: pointer;
    padding: 6px;
    font-family: inherit;
  }
  .leise:hover {
    color: var(--text);
    text-decoration: underline;
  }
</style>
