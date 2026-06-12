<script>
  // Eingangs-Fragebogen (Multiple Choice). Eine Frage pro Karte.
  // Liefert am Ende die Antworten an den Aufrufer (fertig-Callback);
  // gespeichert wird dort - im Tresor, denn die Budgetangabe ist sensibel.
  import { SPARTEN, PROJEKTARTEN } from "$lib/begriffe";

  let { start = null, fertig, abbrechen = null } = $props();

  const FRAGEN = [
    {
      key: "wohnsitz",
      frage: "Wo hast du deinen Wohnsitz?",
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
    background: #fff;
    border-radius: 12px;
    box-shadow: 0 1px 3px rgba(9, 30, 66, 0.12), 0 8px 24px rgba(9, 30, 66, 0.08);
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
    color: #8590a2;
  }
  h2 {
    margin: 0 0 6px;
    font-size: 1.3rem;
    font-weight: 600;
  }
  .hinweis {
    margin: 0 0 12px;
    color: #5e6c84;
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
    color: #172b4d;
    background: #fafbfc;
    border: 2px solid #dfe1e6;
    border-radius: 10px;
    cursor: pointer;
    transition: border-color 0.12s, background 0.12s;
    font-family: inherit;
  }
  .option:hover {
    border-color: #4f6df5;
  }
  .option.gewaehlt {
    border-color: #4f6df5;
    background: #eef1ff;
  }

  .primaer {
    width: 100%;
    margin-top: 20px;
    padding: 11px;
    font-size: 1rem;
    font-weight: 600;
    color: #fff;
    background: #4f6df5;
    border: none;
    border-radius: 8px;
    cursor: pointer;
  }
  .primaer:hover:not(:disabled) {
    background: #3d5bf0;
  }
  .primaer:disabled {
    background: #c1c7d0;
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
    color: #5e6c84;
    font-size: 0.875rem;
    cursor: pointer;
    padding: 6px;
    font-family: inherit;
  }
  .leise:hover {
    color: #172b4d;
    text-decoration: underline;
  }
</style>
