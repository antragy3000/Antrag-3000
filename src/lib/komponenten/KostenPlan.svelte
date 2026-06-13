<script>
  // Kostenfinanzplan-Editor: Kategorien mit Positionen, automatische
  // Summen, Fehlbedarfs-Anzeige. Tresor-Inhalt (Budget ist sensibel).
  import {
    FINANZIERUNGS_STATUS,
    vorlageKfp,
    betragParsen,
    betragFormat,
    kategorieSumme,
    seitenSumme,
    differenz,
  } from "$lib/kfp";

  let { kfp, speichern } = $props();

  let kopie = $state(structuredClone($state.snapshot(kfp)));
  let einmalGespeichert = $state(false);
  let beschaeftigt = $state(false);

  let veraendert = $derived(
    JSON.stringify($state.snapshot(kopie)) !== JSON.stringify($state.snapshot(kfp))
  );

  let leer = $derived(kopie.kosten.length === 0 && kopie.finanzierung.length === 0);
  let diff = $derived(differenz(kopie));

  function kategorieHinzufuegen(seite) {
    kopie[seite].push({ name: "", posten: [] });
  }
  function kategorieEntfernen(seite, index) {
    kopie[seite].splice(index, 1);
  }
  function postenHinzufuegen(seite, kategorie) {
    kategorie.posten.push(
      seite === "kosten"
        ? { bezeichnung: "", erlaeuterung: "", betrag: "" }
        : { bezeichnung: "", status: "geplant", betrag: "" }
    );
  }
  function postenEntfernen(kategorie, index) {
    kategorie.posten.splice(index, 1);
  }

  async function speichernKlick() {
    beschaeftigt = true;
    try {
      // Betraege beim Speichern in Zahlen normalisieren.
      const sauber = structuredClone($state.snapshot(kopie));
      for (const seite of ["kosten", "finanzierung"]) {
        for (const k of sauber[seite]) {
          for (const p of k.posten) p.betrag = betragParsen(p.betrag);
        }
      }
      await speichern(sauber);
      einmalGespeichert = true;
    } finally {
      beschaeftigt = false;
    }
  }
</script>

<div class="bereich">
  <div class="kopfzeile">
    <div>
      <h2>Kostenfinanzplan</h2>
      <p class="untertitel">
        Ausgaben und Finanzierung in Kategorien, mit automatischen Summen.
        Erscheint im Word-Antrag als Tabelle. Bleibt verschlüsselt im Tresor.
      </p>
    </div>
    <div class="speichern-bereich">
      {#if !veraendert && einmalGespeichert}
        <span class="ok">✓ verschlüsselt gespeichert</span>
      {/if}
      <button class="primaer" disabled={!veraendert || beschaeftigt} onclick={speichernKlick}>
        {beschaeftigt ? "Speichert …" : "Speichern"}
      </button>
    </div>
  </div>

  {#if leer}
    <div class="karte start">
      <h3>Noch kein Kostenfinanzplan</h3>
      <p class="untertitel">
        Starte mit den üblichen Kategorien (Personal, Gagen, Material, Reise …)
        oder mit einem leeren Plan.
      </p>
      <div class="start-knoepfe">
        <button class="primaer" onclick={() => (kopie = vorlageKfp())}>
          Mit Vorlage starten
        </button>
        <button class="zweit" onclick={() => kategorieHinzufuegen("kosten")}>
          Leer starten
        </button>
      </div>
    </div>
  {:else}
    <div class="bilanz" class:rot={diff < -0.005} class:gruen={Math.abs(diff) < 0.005}>
      {#if Math.abs(diff) < 0.005}
        ✓ Der Plan ist ausgeglichen ({betragFormat(seitenSumme(kopie.kosten))} Kosten gedeckt).
      {:else if diff < 0}
        ⚠ Fehlbedarf: {betragFormat(Math.abs(diff))}
        (Kosten {betragFormat(seitenSumme(kopie.kosten))} ·
        Finanzierung {betragFormat(seitenSumme(kopie.finanzierung))})
      {:else}
        Überschuss: {betragFormat(diff)}
        (Kosten {betragFormat(seitenSumme(kopie.kosten))} ·
        Finanzierung {betragFormat(seitenSumme(kopie.finanzierung))})
      {/if}
    </div>

    {#each [["kosten", "Ausgaben"], ["finanzierung", "Finanzierung"]] as [seite, seitenTitel] (seite)}
      <section>
        <div class="seiten-kopf">
          <h3>
            {seitenTitel}
            <span class="summe">{betragFormat(seitenSumme(kopie[seite]))}</span>
          </h3>
          <button class="leise" onclick={() => kategorieHinzufuegen(seite)}>
            + Kategorie
          </button>
        </div>

        {#each kopie[seite] as kategorie, ki (kategorie)}
          <div class="karte kategorie">
            <div class="kategorie-kopf">
              <input
                class="kategorie-name"
                type="text"
                placeholder="Name der Kategorie (z. B. Personalkosten)"
                bind:value={kategorie.name}
              />
              <span class="summe">{betragFormat(kategorieSumme(kategorie))}</span>
              <button
                class="entfernen"
                title="Kategorie entfernen"
                onclick={() => kategorieEntfernen(seite, ki)}
              >
                ✕
              </button>
            </div>

            {#each kategorie.posten as posten, pi (posten)}
              <div class="posten">
                <input
                  class="bezeichnung"
                  type="text"
                  placeholder="Bezeichnung"
                  bind:value={posten.bezeichnung}
                />
                {#if seite === "kosten"}
                  <input
                    class="erlaeuterung"
                    type="text"
                    placeholder="Erläuterung (z. B. 625 € × 4 Wochen)"
                    bind:value={posten.erlaeuterung}
                  />
                {:else}
                  <select bind:value={posten.status}>
                    {#each FINANZIERUNGS_STATUS as s (s)}
                      <option value={s}>{s}</option>
                    {/each}
                  </select>
                {/if}
                <input
                  class="betrag"
                  type="text"
                  placeholder="0,00"
                  bind:value={posten.betrag}
                />
                <button
                  class="entfernen"
                  title="Position entfernen"
                  onclick={() => postenEntfernen(kategorie, pi)}
                >
                  ✕
                </button>
              </div>
            {/each}

            <button class="leise" onclick={() => postenHinzufuegen(seite, kategorie)}>
              + Position
            </button>
          </div>
        {/each}
      </section>
    {/each}
  {/if}
</div>

<style>
  .bereich {
    max-width: 920px;
    margin: 0 auto;
    padding: 32px 24px 64px;
  }

  .kopfzeile {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 16px;
    flex-wrap: wrap;
    margin-bottom: 20px;
  }
  h2 {
    margin: 0 0 4px;
    font-size: 1.35rem;
    font-weight: 600;
  }
  .untertitel {
    margin: 0;
    color: #5e6c84;
    font-size: 0.9rem;
    max-width: 480px;
    line-height: 1.5;
  }
  .speichern-bereich {
    display: flex;
    align-items: center;
    gap: 14px;
  }
  .ok {
    color: #216e4e;
    font-size: 0.88rem;
    font-weight: 600;
  }

  .bilanz {
    border-radius: 10px;
    padding: 14px 18px;
    margin-bottom: 24px;
    font-size: 0.95rem;
    font-weight: 600;
    background: #fff7d6;
    color: #533f04;
  }
  .bilanz.rot {
    background: #ffeceb;
    color: #ae2e24;
  }
  .bilanz.gruen {
    background: #dcfff1;
    color: #216e4e;
  }

  section {
    margin-bottom: 36px;
  }
  .seiten-kopf {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: 12px;
  }
  h3 {
    margin: 0;
    font-size: 1.1rem;
    font-weight: 600;
  }
  .summe {
    color: #216e4e;
    font-weight: 700;
    margin-left: 10px;
    font-size: 0.95rem;
    white-space: nowrap;
  }

  .karte {
    background: #fff;
    border-radius: 12px;
    box-shadow: 0 1px 3px rgba(9, 30, 66, 0.12);
    padding: 18px;
  }
  .karte.start {
    padding: 32px;
    text-align: center;
  }
  .karte.start h3 {
    margin-bottom: 6px;
  }
  .start-knoepfe {
    display: flex;
    gap: 12px;
    justify-content: center;
    margin-top: 18px;
  }

  .kategorie {
    margin-bottom: 14px;
  }
  .kategorie-kopf {
    display: flex;
    align-items: center;
    gap: 10px;
    margin-bottom: 10px;
  }
  .kategorie-name {
    flex: 1;
    font-weight: 600;
  }

  .posten {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-bottom: 8px;
  }
  .bezeichnung {
    flex: 2;
  }
  .erlaeuterung {
    flex: 2;
  }
  select {
    flex: 1;
    max-width: 140px;
    padding: 8px 10px;
    font-size: 0.9rem;
    font-family: inherit;
    border: 2px solid #dfe1e6;
    border-radius: 8px;
    background: #fafbfc;
  }
  .betrag {
    width: 110px;
    text-align: right;
  }

  input {
    box-sizing: border-box;
    padding: 8px 10px;
    font-size: 0.9rem;
    font-family: inherit;
    border: 2px solid #dfe1e6;
    border-radius: 8px;
    background: #fafbfc;
    transition: border-color 0.15s, background 0.15s;
    min-width: 0;
  }
  input:focus,
  select:focus {
    outline: none;
    border-color: #4f6df5;
    background: #fff;
  }

  .entfernen {
    background: none;
    border: none;
    color: #8590a2;
    font-size: 0.95rem;
    cursor: pointer;
    padding: 4px 8px;
    border-radius: 6px;
    flex-shrink: 0;
  }
  .entfernen:hover {
    background: #ffeceb;
    color: #ae2e24;
  }

  button.primaer {
    padding: 10px 22px;
    font-size: 0.95rem;
    font-weight: 600;
    font-family: inherit;
    color: #fff;
    background: #4f6df5;
    border: none;
    border-radius: 8px;
    cursor: pointer;
  }
  button.primaer:hover:not(:disabled) {
    background: #3d5bf0;
  }
  button.primaer:disabled {
    background: #c1c7d0;
    cursor: default;
  }

  button.zweit {
    padding: 10px 22px;
    font-size: 0.95rem;
    font-weight: 600;
    font-family: inherit;
    color: #172b4d;
    background: #fff;
    border: 2px solid #dfe1e6;
    border-radius: 8px;
    cursor: pointer;
  }
  button.zweit:hover {
    border-color: #4f6df5;
  }

  button.leise {
    background: none;
    border: none;
    color: #5e6c84;
    font-size: 0.875rem;
    cursor: pointer;
    padding: 6px;
    font-family: inherit;
  }
  button.leise:hover {
    color: #172b4d;
    text-decoration: underline;
  }
</style>
