<script>
  // Kostenfinanzplan-Editor: Kategorien mit Positionen, automatische
  // Summen, Fehlbedarfs-Anzeige. Tresor-Inhalt (Budget ist sensibel).
  import { tick } from "svelte";
  import { fade } from "svelte/transition";
  import {
    vorlageKfp,
    neuePostenId,
    betragFormat,
    istFormel,
    formelAuswerten,
    kategorieSumme,
    seitenSumme,
    differenz,
    postenBetrag,
    finanzBezeichnung,
  } from "$lib/kfp";
  import {
    ALLE,
    GEMEINSAM,
    bausteinVon,
    kostenGefiltert,
    summeGefiltert,
    bausteinNamePruefen,
    bausteinEntfernen,
  } from "$lib/bausteine";
  import { istAufgeteilt, aufteilungRest } from "$lib/teilantrag";
  import TeilBand from "./TeilBand.svelte";
  import { ungespeichertMelden } from "$lib/ungespeichert";

  let {
    kfp,
    bausteine = [],
    // Deckt ein Antrag nur einzelne Teile ab? Nur dann gibt es „Auf Teile
    // aufteilen"; eine schon begonnene Aufteilung bleibt immer sichtbar.
    teilAntraege = false,
    speichern,
    merkliste = [],
    foerderungen = [],
    projektName = "",
    excelErzeugen,
    hinweisAusblenden = false,
    hinweisMerken,
    // Im Projektmenü gewählter Teil (oder null): Dann steht der Filter fest
    // auf diesem Teil; Teile verwalten und Filter wechseln geht in der
    // ganzen Ausgabe.
    teil = null,
    ganzeAusgabe = null,
  } = $props();

  // Gemerkte Förderungen dieses Projekts (als Quellen wählbar) –
  // inkl. eigener Förderungen aus der kombinierten Liste.
  let merklisteFoerderungen = $derived(
    merkliste.map((id) => foerderungen.find((f) => f.id === id)).filter(Boolean)
  );

  function foerderName(id) {
    const f = foerderungen.find((x) => x.id === id);
    return f ? `${f.name} (${f.foerdergeber})` : null;
  }

  // Beim Wechsel der Quelle den Namen als Rückfall mitspeichern, damit
  // auch eigene Förderungen in Word/Excel/JSON aufgelöst werden.
  function quelleGewechselt(posten) {
    posten.bezeichnung = posten.foerderId ? foerderName(posten.foerderId) ?? "" : "";
  }

  let kopie = $state(structuredClone($state.snapshot(kfp)));
  // Bausteine (Teile des Projekts, siehe bausteine.js) werden mit dem Plan
  // bearbeitet und gespeichert.
  let bausteineKopie = $state(structuredClone($state.snapshot(bausteine)));
  // Teile, die inzwischen außerhalb dazukamen (z. B. im Projektmenü), kommen
  // in die Kopie – sonst gingen sie beim nächsten Speichern verloren. Hier
  // gelöschte Teile bleiben gelöscht, weil ihre Kennung schon bekannt ist.
  const bekannteTeile = new Set(bausteineKopie.map((b) => b.id));
  $effect(() => {
    for (const b of bausteine) {
      if (bekannteTeile.has(b.id)) continue;
      bekannteTeile.add(b.id);
      bausteineKopie.push(structuredClone($state.snapshot(b)));
    }
  });
  // Welche Kosten zu sehen sind: ALLE, GEMEINSAM oder eine Baustein-id.
  let filter = $state(ALLE);
  // Der gewählte Teil gibt den Filter vor; zurück in der ganzen Ausgabe
  // sind wieder alle Kosten zu sehen.
  $effect(() => {
    filter = teil ? teil.id : ALLE;
  });
  let einmalGespeichert = $state(false);
  let beschaeftigt = $state(false);

  // Excel-Erzeugung (bewusst, mit Sensibel-Hinweis)
  let hinweisOffen = $state(false);
  let nichtMehrZeigen = $state(false);
  let erzeugtPfad = $state("");
  let erzeugFehler = $state("");
  let erzeugtTimer = null; // blendet die Erfolgsmeldung nach 3 s aus

  let veraendert = $derived(
    JSON.stringify($state.snapshot(kopie)) !== JSON.stringify($state.snapshot(kfp)) ||
      JSON.stringify($state.snapshot(bausteineKopie)) !== JSON.stringify($state.snapshot(bausteine))
  );
  // Vor einem Reiter-/Projektwechsel nachfragen statt still zu verwerfen.
  $effect(() => {
    ungespeichertMelden("kostenplan", "im Kostenplan", veraendert);
    return () => ungespeichertMelden("kostenplan", "", false);
  });

  // Jeder Baustein braucht einen eigenen Namen, sonst lässt sich nicht speichern.
  // Verglichen wird nur mit den Bausteinen davor: Bei einem doppelten Namen ist
  // so nur der neuere markiert.
  let bausteinFehler = $derived(
    bausteineKopie.map((b, i) => bausteinNamePruefen(b.name, bausteineKopie.slice(0, i)))
  );
  let bausteineOk = $derived(bausteinFehler.every((p) => p.ok));
  let gefiltert = $derived(kostenGefiltert(kopie.kosten, filter, bausteineKopie));
  let filterName = $derived(
    filter === GEMEINSAM ? "gemeinsame Kosten" : (bausteineKopie.find((b) => b.id === filter)?.name ?? "")
  );

  // Der neue Baustein bekommt gleich den Cursor, damit man lostippen kann.
  async function bausteinHinzufuegen() {
    const id = neuePostenId();
    bekannteTeile.add(id);
    bausteineKopie.push({ id, name: "" });
    await tick();
    document.getElementById(`baustein-name-${id}`)?.focus();
  }
  function bausteinLoeschen(id) {
    bausteinEntfernen(bausteineKopie, kopie, id);
    if (filter === id) filter = ALLE;
  }
  // Zuordnung eines Postens; "" = gemeinsam.
  function bausteinSetzen(posten, id) {
    posten.bausteinId = id;
  }

  let leer = $derived(kopie.kosten.length === 0 && kopie.finanzierung.length === 0);
  let diff = $derived(differenz(kopie));

  function kategorieHinzufuegen(seite) {
    kopie[seite].push({ name: "", posten: [] });
  }
  function kategorieEntfernen(seite, index) {
    kopie[seite].splice(index, 1);
  }
  function postenHinzufuegen(seite, kategorie) {
    const posten =
      seite === "kosten"
        ? { id: neuePostenId(), bezeichnung: "", erlaeuterung: "", betrag: "" }
        : { bezeichnung: "", betrag: "", foerderId: "" };
    // Im Filter auf einen Baustein gehört die neue Position gleich dazu.
    if (seite === "kosten" && filter !== ALLE && filter !== GEMEINSAM) posten.bausteinId = filter;
    kategorie.posten.push(posten);
  }

  // Kurz-Hinweis "= 3.000,00 €" neben einem Betragsfeld, wenn dort
  // eine Rechnung statt einer reinen Zahl steht.
  function rechenHinweis(betrag) {
    if (!istFormel(betrag)) return "";
    const { wert, fehler } = formelAuswerten(betrag);
    return fehler ? "= ?" : "= " + betragFormat(wert);
  }
  function postenEntfernen(kategorie, index) {
    kategorie.posten.splice(index, 1);
  }

  // --- Finanzierung als flache Förderer-Liste (intern EINE Sammel-Ebene) ---
  // Alle Finanzierungs-Positionen (Förderer/Quellen) als eine Liste. Kategorien
  // gibt es hier nicht mehr; die einzelne Sammel-Kategorie trägt sie nur.
  let finanzPosten = $derived(kopie.finanzierung[0]?.posten ?? []);
  function finanzKategorie() {
    if (kopie.finanzierung.length === 0) kopie.finanzierung.push({ name: "", posten: [] });
    return kopie.finanzierung[0];
  }
  function foerdererHinzufuegen() {
    finanzKategorie().posten.push({ bezeichnung: "", betrag: "", foerderId: "" });
  }
  function foerdererEntfernen(pi) {
    kopie.finanzierung[0]?.posten.splice(pi, 1);
  }

  // Aufteilung einer Geldquelle auf die Bausteine (für Teil-Anträge, siehe
  // teilantrag.js). Offen ist sie, sobald ein Anteil eingetragen ist oder
  // man „auf Teile aufteilen" geklickt hat.
  let aufteilungOffen = $state([]);
  function aufteilungZeigen(posten) {
    return istAufgeteilt(posten, bausteineKopie) || aufteilungOffen.includes(posten);
  }
  function aufteilungOeffnen(posten) {
    aufteilungOffen.push(posten);
  }
  function aufteilungEntfernen(posten) {
    delete posten.anteile;
    aufteilungOffen = aufteilungOffen.filter((p) => p !== posten);
  }
  function anteilSetzen(posten, id, wert) {
    if (!posten.anteile) posten.anteile = {};
    posten.anteile[id] = wert;
  }
  function restText(posten) {
    const rest = aufteilungRest(posten, bausteineKopie);
    if (Math.abs(rest) < 0.005) return { text: "✓ genau verteilt", ok: true };
    return rest > 0
      ? { text: `noch nicht verteilt: ${betragFormat(rest)}`, ok: false }
      : { text: `${betragFormat(-rest)} zu viel verteilt`, ok: false };
  }

  // Reihenfolge verschieben (Position in der Kategorie bzw. Kategorie
  // auf ihrer Seite); richtung = -1 (hoch) oder +1 (runter).
  function verschieben(liste, i, richtung) {
    const j = i + richtung;
    if (j < 0 || j >= liste.length) return;
    const [el] = liste.splice(i, 1);
    liste.splice(j, 0, el);
  }

  let vorschauOffen = $state(false);

  // Betraege bleiben als Text erhalten (damit Rechnungen wie
  // "50 × 4 × 5 × 3" bearbeitbar bleiben); ausgerechnet wird live.
  function saubereBausteine() {
    return structuredClone($state.snapshot(bausteineKopie)).map((b) => ({ ...b, name: b.name.trim() }));
  }
  function saubereKopie() {
    const sauber = structuredClone($state.snapshot(kopie));
    for (const seite of ["kosten", "finanzierung"]) {
      for (const k of sauber[seite]) {
        for (const p of k.posten) {
          p.betrag = String(p.betrag ?? "").trim();
          // Leere Anteile weglassen; ganz ohne Anteile ist die Quelle nicht aufgeteilt.
          if (p.anteile) {
            for (const [id, w] of Object.entries(p.anteile)) {
              const t = String(w ?? "").trim();
              if (t) p.anteile[id] = t;
              else delete p.anteile[id];
            }
            if (Object.keys(p.anteile).length === 0) delete p.anteile;
          }
        }
      }
    }
    return sauber;
  }

  async function speichernKlick() {
    beschaeftigt = true;
    try {
      await speichern(saubereKopie(), saubereBausteine());
      einmalGespeichert = true;
    } finally {
      beschaeftigt = false;
    }
  }

  // "KFP generieren": zeigt erst den Sensibel-Hinweis (sofern für
  // dieses Projekt nicht ausgeblendet), erzeugt danach die Excel.
  function generierenKlick() {
    erzeugtPfad = "";
    erzeugFehler = "";
    if (hinweisAusblenden) {
      generieren();
    } else {
      nichtMehrZeigen = false;
      hinweisOffen = true;
    }
  }

  async function verstandenKlick() {
    hinweisOffen = false;
    if (nichtMehrZeigen) await hinweisMerken();
    await generieren();
  }

  async function generieren() {
    beschaeftigt = true;
    if (erzeugtTimer) {
      clearTimeout(erzeugtTimer);
      erzeugtTimer = null;
    }
    try {
      const sauber = saubereKopie();
      const sauberBausteine = saubereBausteine();
      await speichern(sauber, sauberBausteine); // Tresor mit dem erzeugten Stand gleichziehen
      einmalGespeichert = true;
      erzeugFehler = "";
      erzeugtPfad = await excelErzeugen(sauber, sauberBausteine);
      // Nach 5 s ausblenden, damit eine erneute Erzeugung sichtbar wird.
      erzeugtTimer = setTimeout(() => {
        erzeugtPfad = "";
        erzeugtTimer = null;
      }, 5000);
    } catch (e) {
      erzeugFehler = String(e);
    } finally {
      beschaeftigt = false;
    }
  }
</script>

<div class="bereich">
  {#if teil}
    <TeilBand {teil} ausgabeName={projektName} {ganzeAusgabe} />
  {/if}
  <div class="kopfzeile">
    <div class="titel-block">
      <h2>Kostenfinanzplan</h2>
      <p class="untertitel">
        Ausgaben und Finanzierung in Kategorien, mit automatischen Summen.
        Bleibt <strong>verschlüsselt im Tresor</strong>. Eine Excel-Datei
        entsteht nur, wenn du sie ausdrücklich erzeugst.
      </p>
    </div>
    <div class="speichern-bereich">
      <span class="ok" class:sichtbar={!veraendert && einmalGespeichert}>
        ✓ verschlüsselt gespeichert
      </span>
      <button class="zweit" disabled={leer} onclick={() => (vorschauOffen = true)}>
        Vorschau
      </button>
      <button class="zweit" disabled={beschaeftigt || !bausteineOk} onclick={generierenKlick}>
        KFP generieren (Excel)
      </button>
      <button class="primaer" disabled={!veraendert || beschaeftigt || !bausteineOk} onclick={speichernKlick}>
        {beschaeftigt ? "Speichert …" : "Speichern"}
      </button>
    </div>
  </div>

  {#if erzeugtPfad}
    <div class="erzeugt-ok" transition:fade={{ duration: 250 }}>
      ✓ Excel erstellt: <code>{erzeugtPfad}</code>
    </div>
  {/if}
  {#if erzeugFehler}
    <div class="erzeugt-fehler">
      ⚠ Die Excel konnte nicht erstellt werden (ist sie gerade in Excel geöffnet?).<br />
      {erzeugFehler}
    </div>
  {/if}

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
      <!-- Die Bilanz gilt immer für die ganze Ausgabe, auch beim Blick auf einen Teil. -->
      {teil ? "Ganze Ausgabe: " : ""}
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

    <!-- Bausteine: optionale Teile des Projekts (z. B. einzelne Konzerte) -->
    {#if bausteineKopie.length && !teil}
      <section>
        <div class="seiten-kopf">
          <h3>Teile</h3>
          <button class="leise" onclick={bausteinHinzufuegen}>+ Teil</button>
        </div>
        <div class="karte">
          <p class="baustein-hinweis">
            Ordne jede Position einem Teil zu. Was zu allen gehört (z. B. Plakate
            für die ganze Reihe), bleibt <strong>gemeinsam</strong>.
          </p>
          {#each bausteineKopie as baustein, bi (baustein.id)}
            <div class="baustein-zeile">
              <input
                type="text"
                placeholder="Name des Teils (z. B. Konzert März)"
                aria-label="Name des Teils"
                id={`baustein-name-${baustein.id}`}
                bind:value={baustein.name}
              />
              <span class="summe">{betragFormat(summeGefiltert(kopie.kosten, baustein.id, bausteineKopie, postenBetrag))}</span>
              <button
                class="entfernen"
                title="Teil entfernen – seine Positionen bleiben und werden gemeinsam"
                aria-label={`Teil „${baustein.name}“ entfernen`}
                onclick={() => bausteinLoeschen(baustein.id)}
              >
                ✕
              </button>
            </div>
            {#if bausteinFehler[bi]?.fehler}
              <p class="baustein-fehler" role="alert">{bausteinFehler[bi].fehler}</p>
            {:else if !baustein.name.trim()}
              <p class="baustein-fehler">Bitte gib dem Teil einen Namen.</p>
            {/if}
          {/each}
          <div class="baustein-zeile gemeinsam-zeile">
            <span>gemeinsam</span>
            <span class="summe">{betragFormat(summeGefiltert(kopie.kosten, GEMEINSAM, bausteineKopie, postenBetrag))}</span>
          </div>
        </div>
      </section>
    {/if}

    <!-- Ausgaben: Kategorien mit Positionen -->
    <section>
      <div class="seiten-kopf">
        <h3>
          Ausgaben
          {#if filter === ALLE}
            <span class="summe">{betragFormat(seitenSumme(kopie.kosten))}</span>
          {:else}
            <span class="filter-summe">
              {filterName}
              <span class="summe">{betragFormat(summeGefiltert(kopie.kosten, filter, bausteineKopie, postenBetrag))}</span>
              von {betragFormat(seitenSumme(kopie.kosten))}
            </span>
          {/if}
        </h3>
        <span class="kopf-knoepfe">
          {#if !bausteineKopie.length}
            <button class="leise" title="Teile des Projekts, z. B. die einzelnen Konzerte einer Reihe" onclick={bausteinHinzufuegen}>
              + Teil
            </button>
          {/if}
          {#if filter === ALLE}
            <button class="leise" onclick={() => kategorieHinzufuegen("kosten")}>
              + Kategorie
            </button>
          {/if}
        </span>
      </div>

      {#if bausteineKopie.length && !teil}
        <div class="filter" role="group" aria-label="Ausgaben nach Teil filtern">
          <button class:aktiv={filter === ALLE} aria-pressed={filter === ALLE} onclick={() => (filter = ALLE)}>Alle</button>
          {#each bausteineKopie as baustein (baustein.id)}
            <button class:aktiv={filter === baustein.id} aria-pressed={filter === baustein.id} onclick={() => (filter = baustein.id)}>
              {baustein.name.trim() || "(ohne Name)"}
            </button>
          {/each}
          <button class:aktiv={filter === GEMEINSAM} aria-pressed={filter === GEMEINSAM} onclick={() => (filter = GEMEINSAM)}>gemeinsam</button>
        </div>
        {#if filter !== ALLE}
          <p class="filter-hinweis">
            Die Nummern bleiben wie im ganzen Plan, weil Belege daran hängen.
            Reihenfolge und Kategorien änderst du unter <strong>Alle</strong>.
          </p>
        {/if}
      {/if}

      {#if teil}
        <p class="filter-hinweis">
          Zu sehen sind nur die Ausgaben von „{teil.name}“. Die Nummern bleiben wie
          im ganzen Plan, weil Belege daran hängen. Reihenfolge, Kategorien und Teile
          änderst du in der ganzen Ausgabe.
        </p>
      {/if}

      {#if filter !== ALLE && gefiltert.length === 0}
        <p class="finanz-leer">{teil ? "Diesem Teil" : "Diesem Filter"} ist noch keine Ausgabe zugeordnet.</p>
      {/if}

      {#each gefiltert as eintrag (eintrag.kategorie)}
        {@const kategorie = eintrag.kategorie}
        {@const ki = Number(eintrag.nummer) - 1}
        <div class="karte kategorie">
          <div class="kategorie-kopf">
            <span class="nummer">{eintrag.nummer}</span>
            {#if filter === ALLE}
              <span class="verschieben">
                <button class="pfeil" title="Kategorie nach oben" disabled={ki === 0}
                  onclick={() => verschieben(kopie.kosten, ki, -1)}>▲</button>
                <button class="pfeil" title="Kategorie nach unten" disabled={ki === kopie.kosten.length - 1}
                  onclick={() => verschieben(kopie.kosten, ki, 1)}>▼</button>
              </span>
            {/if}
            <input
              class="kategorie-name"
              type="text"
              placeholder="Name der Kategorie (z. B. Personalkosten)"
              bind:value={kategorie.name}
            />
            <span class="summe">{betragFormat(filter === ALLE ? kategorieSumme(kategorie) : summeGefiltert([kategorie], filter, bausteineKopie, postenBetrag))}</span>
            {#if filter === ALLE}
              <button
                class="entfernen"
                title="Kategorie entfernen"
                onclick={() => kategorieEntfernen("kosten", ki)}
              >
                ✕
              </button>
            {/if}
          </div>

          {#each eintrag.posten as e (e.posten)}
            {@const posten = e.posten}
            {@const pi = e.index}
            <div class="posten">
              <span class="nummer klein">{e.nummer}</span>
              {#if filter === ALLE}
                <span class="verschieben">
                  <button class="pfeil" title="Position nach oben" disabled={pi === 0}
                    onclick={() => verschieben(kategorie.posten, pi, -1)}>▲</button>
                  <button class="pfeil" title="Position nach unten" disabled={pi === kategorie.posten.length - 1}
                    onclick={() => verschieben(kategorie.posten, pi, 1)}>▼</button>
                </span>
              {/if}
              <input
                class="bezeichnung"
                type="text"
                placeholder="Bezeichnung"
                bind:value={posten.bezeichnung}
              />
              <input
                class="erlaeuterung"
                type="text"
                placeholder="Erläuterung (z. B. 625 € pro Woche)"
                bind:value={posten.erlaeuterung}
              />
              {#if bausteineKopie.length}
                <select
                  class="baustein-wahl"
                  aria-label="Teil dieser Position"
                  value={bausteinVon(posten, bausteineKopie)}
                  onchange={(ev) => bausteinSetzen(posten, ev.currentTarget.value)}
                >
                  <option value="">gemeinsam</option>
                  {#each bausteineKopie as baustein (baustein.id)}
                    <option value={baustein.id}>{baustein.name.trim() || "(ohne Name)"}</option>
                  {/each}
                </select>
              {/if}
              <div class="betrag-feld">
                <input
                  class="betrag"
                  type="text"
                  placeholder="0,00 oder 50 × 4 × 3"
                  bind:value={posten.betrag}
                />
                {#if rechenHinweis(posten.betrag)}
                  <span class="rechen-hinweis">{rechenHinweis(posten.betrag)}</span>
                {/if}
              </div>
              <button
                class="entfernen"
                title="Position entfernen"
                onclick={() => postenEntfernen(kategorie, pi)}
              >
                ✕
              </button>
            </div>
          {/each}

          <button class="leise" onclick={() => postenHinzufuegen("kosten", kategorie)}>
            + Position
          </button>
        </div>
      {/each}
    </section>

    <!-- Finanzierung: flache Förderer-Liste (Förderer aus der Merkliste + Betrag) -->
    <section>
      <div class="seiten-kopf">
        <h3>
          Finanzierung
          <span class="summe">{betragFormat(seitenSumme(kopie.finanzierung))}</span>
        </h3>
        <button class="leise" onclick={foerdererHinzufuegen}>+ Förderer</button>
      </div>

      <div class="karte kategorie">
        {#if finanzPosten.length === 0}
          <p class="finanz-leer">
            Noch keine Finanzierung. Über <strong>„+ Förderer"</strong> wählst du einen
            Förderer aus deiner <strong>Merkliste</strong> (oder trägst Eigenmittel /
            Einnahmen ein) und den Betrag, den du dort beantragst.
          </p>
        {/if}

        {#each finanzPosten as posten, pi (posten)}
          <div class="posten">
            <span class="nummer klein">{pi + 1}</span>
            <span class="verschieben">
              <button class="pfeil" title="nach oben" disabled={pi === 0}
                onclick={() => verschieben(finanzPosten, pi, -1)}>▲</button>
              <button class="pfeil" title="nach unten" disabled={pi === finanzPosten.length - 1}
                onclick={() => verschieben(finanzPosten, pi, 1)}>▼</button>
            </span>
            <select
              class="quelle"
              bind:value={posten.foerderId}
              onchange={() => quelleGewechselt(posten)}
            >
              <option value="">Eigenmittel / Einnahmen …</option>
              {#if posten.foerderId && !merkliste.includes(posten.foerderId)}
                <option value={posten.foerderId}>
                  {foerderName(posten.foerderId) ?? "(nicht mehr gemerkt)"}
                </option>
              {/if}
              {#each merklisteFoerderungen as f (f.id)}
                <option value={f.id}>{f.name} ({f.foerdergeber})</option>
              {/each}
            </select>
            {#if !posten.foerderId}
              <input
                class="bezeichnung"
                type="text"
                placeholder="z. B. Eigenmittel, Ticketeinnahmen"
                bind:value={posten.bezeichnung}
              />
            {/if}
            <div class="betrag-feld">
              <input
                class="betrag"
                type="text"
                placeholder="beantragter Betrag"
                bind:value={posten.betrag}
              />
              {#if rechenHinweis(posten.betrag)}
                <span class="rechen-hinweis">{rechenHinweis(posten.betrag)}</span>
              {/if}
            </div>
            <button
              class="entfernen"
              title="Förderer entfernen"
              onclick={() => foerdererEntfernen(pi)}
            >
              ✕
            </button>
          </div>
          {#if bausteineKopie.length}
            {#if aufteilungZeigen(posten)}
              {@const rest = restText(posten)}
              <div class="aufteilung">
                <p class="aufteilung-kopf">Auf die Teile aufgeteilt</p>
                {#each bausteineKopie as baustein (baustein.id)}
                  <label class="anteil">
                    <span>{baustein.name.trim() || "(ohne Name)"}</span>
                    <input class="betrag" type="text" placeholder="0,00"
                      value={posten.anteile?.[baustein.id] ?? ""}
                      oninput={(e) => anteilSetzen(posten, baustein.id, e.currentTarget.value)} />
                  </label>
                {/each}
                <label class="anteil">
                  <span>gemeinsame Kosten</span>
                  <input class="betrag" type="text" placeholder="0,00"
                    value={posten.anteile?.[GEMEINSAM] ?? ""}
                    oninput={(e) => anteilSetzen(posten, GEMEINSAM, e.currentTarget.value)} />
                </label>
                <div class="aufteilung-fuss">
                  <span class:rest-ok={rest.ok} class:rest-offen={!rest.ok}>{rest.text}</span>
                  <button class="leise" onclick={() => aufteilungEntfernen(posten)}>Aufteilung entfernen</button>
                </div>
              </div>
            {:else if teilAntraege}
              <button class="leise aufteilen" onclick={() => aufteilungOeffnen(posten)}>
                Auf Teile aufteilen
              </button>
            {/if}
          {/if}
        {/each}

        <button class="leise" onclick={foerdererHinzufuegen}>+ Förderer</button>
      </div>
    </section>
  {/if}
</div>

{#if hinweisOffen}
  <div class="schleier" onclick={() => (hinweisOffen = false)} role="presentation">
    <div class="dialog" onclick={(e) => e.stopPropagation()} role="presentation">
      <h2>Kostenfinanzplan als Excel erstellen</h2>
      <p>
        Antrag 3000 behandelt deinen Kostenfinanzplan als <strong>sensible
        Daten</strong> und speichert ihn deshalb verschlüsselt im Tresor.
      </p>
      <p class="warn">
        Wenn du jetzt eine Excel-Datei erzeugst, liegen diese Finanzdaten
        <strong>unverschlüsselt und offen lesbar</strong> auf deinem Computer.
      </p>
      <p class="pfad">
        Ablageort:<br />
        <code>Dokumente\Antrag 3000\{projektName}\Kostenfinanzplan.xlsx</code>
      </p>
      <p class="klein">
        Liegt dein Dokumente-Ordner in einer Cloud (z. B. OneDrive), wird die
        Datei mit synchronisiert.
      </p>

      <label class="nichtmehr">
        <input type="checkbox" bind:checked={nichtMehrZeigen} />
        Hinweis bei diesem Projekt nicht mehr anzeigen
      </label>

      <div class="dialog-knoepfe">
        <button class="leise" onclick={() => (hinweisOffen = false)}>Abbrechen</button>
        <button class="primaer" onclick={verstandenKlick}>Verstanden, Excel erstellen</button>
      </div>
    </div>
  </div>
{/if}

{#if vorschauOffen}
  <div class="schleier" onclick={() => (vorschauOffen = false)} role="presentation">
    <div class="vorschau" onclick={(e) => e.stopPropagation()} role="presentation">
      <div class="vorschau-kopf">
        <h2>Vorschau Kostenfinanzplan</h2>
        <button class="schliessen" onclick={() => (vorschauOffen = false)} aria-label="Schließen">✕</button>
      </div>

      {#if kopie.kosten.length}
        <h3>Kostenplan</h3>
        <table>
          <thead>
            <tr><th>Nr.</th><th>Ausgaben</th><th>Erläuterung</th><th class="r">Betrag</th></tr>
          </thead>
          <tbody>
            {#each kopie.kosten as k, i (k)}
              <tr class="kat">
                <td>{i + 1}</td><td colspan="2">{k.name || "(ohne Name)"}</td>
                <td class="r">{betragFormat(kategorieSumme(k))}</td>
              </tr>
              {#each k.posten as p, j (p)}
                <tr>
                  <td>{i + 1}.{j + 1}</td>
                  <td>{p.bezeichnung}</td>
                  <td class="grau">{p.erlaeuterung || ""}</td>
                  <td class="r">{betragFormat(postenBetrag(p))}</td>
                </tr>
              {/each}
            {/each}
            <tr class="summe-zeile">
              <td colspan="3">Gesamtkosten</td>
              <td class="r">{betragFormat(seitenSumme(kopie.kosten))}</td>
            </tr>
          </tbody>
        </table>
      {/if}

      {#if finanzPosten.length}
        <h3>Finanzierungsplan</h3>
        <table>
          <thead>
            <tr><th>Nr.</th><th>Förderer / Quelle</th><th class="r">Beantragt</th></tr>
          </thead>
          <tbody>
            {#each finanzPosten as p, j (p)}
              <tr>
                <td>{j + 1}</td>
                <td>{finanzBezeichnung(p)}</td>
                <td class="r">{betragFormat(postenBetrag(p))}</td>
              </tr>
            {/each}
            <tr class="summe-zeile">
              <td colspan="2">Gesamtfinanzierung</td>
              <td class="r">{betragFormat(seitenSumme(kopie.finanzierung))}</td>
            </tr>
          </tbody>
        </table>
      {/if}

      <div class="vorschau-bilanz" class:rot={diff < -0.005} class:gruen={Math.abs(diff) < 0.005}>
        {#if Math.abs(diff) < 0.005}
          Ausgeglichen
        {:else if diff < 0}
          Fehlbedarf: {betragFormat(Math.abs(diff))}
        {:else}
          Überschuss: {betragFormat(diff)}
        {/if}
      </div>
      <p class="vorschau-hinweis">
        So erscheint der Plan im Word-Antrag und in der Excel-Datei.
      </p>
    </div>
  </div>
{/if}

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
    color: var(--text-muted);
    font-size: 0.9rem;
    max-width: 480px;
    line-height: 1.5;
  }
  /* Textblock darf wachsen/schrumpfen; faellt erst unter ~260px Breite
     auf eine eigene Zeile, dann landen die Knoepfe darunter. */
  .titel-block {
    flex: 1 1 300px;
    min-width: 260px;
  }
  .speichern-bereich {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    justify-content: flex-end;
    flex-shrink: 0;
    gap: 14px;
  }
  .ok {
    color: var(--erfolg-text);
    font-size: 0.88rem;
    font-weight: 600;
    white-space: nowrap;
    opacity: 0;
    transition: opacity 0.15s;
  }
  .ok.sichtbar {
    opacity: 1;
  }
  /* feste Mindestbreite verhindert das "Springen" bei "Speichert …" */
  .speichern-bereich .primaer {
    min-width: 124px;
  }

  .erzeugt-ok {
    background: var(--erfolg-bg);
    color: var(--erfolg-text);
    border-radius: 10px;
    padding: 12px 16px;
    margin-bottom: 20px;
    font-size: 0.9rem;
    word-break: break-all;
  }
  .erzeugt-ok code {
    font-size: 0.85rem;
  }
  .erzeugt-fehler {
    background: var(--gefahr-bg);
    color: var(--gefahr-text);
    border-radius: 10px;
    padding: 12px 16px;
    margin-bottom: 20px;
    font-size: 0.9rem;
  }

  /* Sensibel-Hinweis vor der Excel-Erzeugung */
  .schleier {
    position: fixed;
    inset: 0;
    background: var(--schatten-xl);
    display: grid;
    place-items: center;
    padding: 24px;
    z-index: 30;
  }
  .dialog {
    background: var(--weiss);
    border-radius: 12px;
    padding: 32px;
    max-width: 480px;
    width: 100%;
    box-shadow: 0 12px 40px var(--schatten-lg);
  }
  .dialog h2 {
    margin: 0 0 14px;
    font-size: 1.2rem;
  }
  .dialog p {
    margin: 0 0 12px;
    font-size: 0.92rem;
    line-height: 1.55;
  }
  .dialog .warn {
    background: var(--warnung-bg);
    color: var(--warnung-text2);
    border-radius: 8px;
    padding: 10px 14px;
  }
  .dialog .pfad code {
    font-size: 0.85rem;
    word-break: break-all;
  }
  .dialog .klein {
    color: var(--text-muted);
    font-size: 0.84rem;
  }
  .nichtmehr {
    display: flex;
    align-items: center;
    gap: 8px;
    margin: 18px 0 8px;
    font-size: 0.9rem;
    color: var(--text-2);
    cursor: pointer;
  }
  .nichtmehr input {
    width: auto;
  }
  .dialog-knoepfe {
    display: flex;
    justify-content: flex-end;
    gap: 10px;
    margin-top: 16px;
  }
  .dialog-knoepfe button {
    width: auto;
  }

  .bilanz {
    border-radius: 10px;
    padding: 14px 18px;
    margin-bottom: 24px;
    font-size: 0.95rem;
    font-weight: 600;
    background: var(--warnung-bg);
    color: var(--warnung-text2);
  }
  .bilanz.rot {
    background: var(--gefahr-bg);
    color: var(--gefahr-text);
  }
  .bilanz.gruen {
    background: var(--erfolg-bg);
    color: var(--erfolg-text);
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
    color: var(--erfolg-text);
    font-weight: 700;
    margin-left: 10px;
    font-size: 0.95rem;
    white-space: nowrap;
  }

  .karte {
    background: var(--weiss);
    border-radius: 12px;
    box-shadow: 0 1px 3px var(--schatten-sm);
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
  .finanz-leer {
    margin: 2px 0 12px;
    color: var(--text-muted);
    font-size: 0.9rem;
    line-height: 1.5;
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

  .nummer {
    flex-shrink: 0;
    min-width: 22px;
    font-weight: 700;
    color: var(--text-2);
    font-size: 0.95rem;
    text-align: right;
  }
  .nummer.klein {
    min-width: 34px;
    font-weight: 600;
    color: var(--text-leise);
    font-size: 0.85rem;
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
  /* Breit genug für „gemeinsam" und kurze Namen; dafür wird der Betrag
     schmaler, sobald es Bausteine gibt. */
  .baustein-wahl {
    flex: 0 1 136px;
    min-width: 100px;
    padding: 8px 6px;
    font-size: 0.85rem;
    font-family: inherit;
    color: var(--text);
    border: 2px solid var(--rand);
    border-radius: 8px;
    background: var(--flaeche);
  }
  .kopf-knoepfe {
    display: flex;
    gap: 4px;
  }
  .filter-summe {
    margin-left: 10px;
    font-size: 0.9rem;
    font-weight: 400;
    color: var(--text-muted);
  }
  .filter-summe .summe {
    margin-left: 4px;
  }
  .filter {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    margin-bottom: 12px;
  }
  .filter button {
    padding: 5px 12px;
    font-size: 0.85rem;
    font-weight: 600;
    font-family: inherit;
    color: var(--text-2);
    background: var(--weiss);
    border: 2px solid var(--rand);
    border-radius: 99px;
    cursor: pointer;
    max-width: 100%;
    overflow-wrap: anywhere;
  }
  .filter button:hover {
    border-color: var(--akzent);
  }
  .filter button.aktiv {
    color: var(--auf-farbe);
    background: var(--akzent);
    border-color: var(--akzent);
  }
  .filter-hinweis,
  .baustein-hinweis {
    margin: 0 0 12px;
    font-size: 0.84rem;
    line-height: 1.5;
    color: var(--text-muted);
  }
  .baustein-zeile {
    display: flex;
    align-items: center;
    gap: 10px;
    margin-bottom: 8px;
  }
  .baustein-zeile input {
    flex: 1;
  }
  .gemeinsam-zeile {
    margin: 4px 0 0;
    padding: 8px 0 0;
    border-top: 1px solid var(--flaeche-2b);
    font-size: 0.9rem;
    color: var(--text-muted);
  }
  .gemeinsam-zeile span:first-child {
    flex: 1;
    padding-left: 12px;
  }
  .gemeinsam-zeile .summe {
    margin-right: 40px;
  }
  .baustein-fehler {
    margin: -4px 0 8px;
    font-size: 0.82rem;
    color: var(--gefahr-text);
  }
  .quelle {
    flex: 2;
    min-width: 0;
    padding: 8px 10px;
    font-size: 0.9rem;
    font-family: inherit;
    border: 2px solid var(--rand);
    border-radius: 8px;
    background: var(--flaeche);
  }
  .quelle:focus {
    outline: none;
    border-color: var(--akzent);
    background: var(--weiss);
  }
  .posten:has(.baustein-wahl) .betrag {
    width: 120px;
  }
  .betrag-feld {
    display: flex;
    flex-direction: column;
    align-items: flex-end;
    gap: 2px;
    flex-shrink: 0;
  }
  .betrag {
    width: 150px;
    text-align: right;
  }
  .aufteilen {
    display: block;
    margin: -4px 0 10px 52px;
    font-size: 0.82rem;
  }
  .aufteilung {
    margin: -2px 40px 14px 52px;
    padding: 10px 14px;
    background: var(--flaeche);
    border-radius: 10px;
  }
  .aufteilung-kopf {
    margin: 0 0 8px;
    font-size: 0.82rem;
    font-weight: 600;
    color: var(--text-muted);
  }
  .anteil {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
    margin-bottom: 6px;
    font-size: 0.88rem;
  }
  .anteil span {
    min-width: 0;
    overflow-wrap: anywhere;
  }
  .anteil .betrag {
    width: 130px;
    flex-shrink: 0;
  }
  .aufteilung-fuss {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
    flex-wrap: wrap;
    margin-top: 4px;
    font-size: 0.84rem;
  }
  .rest-ok {
    color: var(--erfolg-text);
    font-weight: 600;
  }
  .rest-offen {
    color: var(--warnung-text2);
    font-weight: 600;
  }
  .rechen-hinweis {
    font-size: 0.78rem;
    font-weight: 600;
    color: var(--erfolg-text);
    white-space: nowrap;
  }

  input[type="text"] {
    box-sizing: border-box;
    padding: 8px 10px;
    font-size: 0.9rem;
    font-family: inherit;
    border: 2px solid var(--rand);
    border-radius: 8px;
    background: var(--flaeche);
    transition: border-color 0.15s, background 0.15s;
    min-width: 0;
  }
  input[type="text"]:focus,
  select:focus {
    outline: none;
    border-color: var(--akzent);
    background: var(--weiss);
  }

  .entfernen {
    background: none;
    border: none;
    color: var(--text-leise);
    font-size: 0.95rem;
    cursor: pointer;
    padding: 4px 8px;
    border-radius: 6px;
    flex-shrink: 0;
  }
  .entfernen:hover {
    background: var(--gefahr-bg);
    color: var(--gefahr-text);
  }

  button.primaer {
    padding: 10px 22px;
    font-size: 0.95rem;
    font-weight: 600;
    font-family: inherit;
    color: var(--auf-farbe);
    background: var(--akzent);
    border: none;
    border-radius: 8px;
    cursor: pointer;
  }
  button.primaer:hover:not(:disabled) {
    background: var(--akzent-d);
  }
  button.primaer:disabled {
    background: var(--grau-3);
    cursor: default;
  }

  button.zweit {
    padding: 10px 22px;
    font-size: 0.95rem;
    font-weight: 600;
    font-family: inherit;
    color: var(--text);
    background: var(--weiss);
    border: 2px solid var(--rand);
    border-radius: 8px;
    cursor: pointer;
  }
  button.zweit:hover {
    border-color: var(--akzent);
  }

  button.leise {
    background: none;
    border: none;
    color: var(--text-muted);
    font-size: 0.875rem;
    cursor: pointer;
    padding: 6px;
    font-family: inherit;
  }
  button.leise:hover {
    color: var(--text);
    text-decoration: underline;
  }
  button.zweit:disabled {
    color: var(--grau-4);
    border-color: var(--flaeche-3);
    cursor: default;
  }
  button.zweit:disabled:hover {
    border-color: var(--flaeche-3);
  }

  /* Verschiebe-Pfeile */
  .verschieben {
    display: flex;
    flex-direction: column;
    flex-shrink: 0;
    line-height: 0.7;
  }
  .pfeil {
    background: none;
    border: none;
    color: var(--text-leise);
    font-size: 0.7rem;
    cursor: pointer;
    padding: 1px 3px;
    border-radius: 4px;
  }
  .pfeil:hover:not(:disabled) {
    background: var(--akzent-bg);
    color: var(--akzent-text);
  }
  .pfeil:disabled {
    color: var(--rand);
    cursor: default;
  }

  /* Vorschau */
  .vorschau {
    background: var(--weiss);
    border-radius: 12px;
    padding: 28px 32px 32px;
    max-width: 680px;
    width: 100%;
    max-height: 85vh;
    overflow-y: auto;
    box-shadow: 0 12px 40px var(--schatten-lg);
  }
  .vorschau-kopf {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: 8px;
  }
  .vorschau-kopf h2 {
    margin: 0;
    font-size: 1.2rem;
  }
  .schliessen {
    background: none;
    border: none;
    font-size: 1.05rem;
    color: var(--text-muted);
    cursor: pointer;
    padding: 6px 10px;
    border-radius: 8px;
  }
  .schliessen:hover {
    background: var(--flaeche-2b);
    color: var(--text);
  }
  .vorschau h3 {
    margin: 20px 0 8px;
    font-size: 1rem;
  }
  .vorschau table {
    width: 100%;
    border-collapse: collapse;
    font-size: 0.88rem;
  }
  .vorschau th {
    text-align: left;
    font-weight: 600;
    color: var(--text-muted);
    border-bottom: 2px solid var(--rand);
    padding: 6px 8px;
  }
  .vorschau td {
    padding: 5px 8px;
    border-bottom: 1px solid var(--flaeche-2b);
    vertical-align: top;
  }
  .vorschau td.r,
  .vorschau th.r {
    text-align: right;
    white-space: nowrap;
  }
  .vorschau td.grau {
    color: var(--text-muted);
  }
  .vorschau tr.kat td {
    font-weight: 700;
    background: var(--flaeche-b);
  }
  .vorschau tr.summe-zeile td {
    font-weight: 700;
    border-top: 2px solid var(--rand);
    border-bottom: none;
  }
  .vorschau-bilanz {
    margin-top: 18px;
    border-radius: 10px;
    padding: 12px 16px;
    font-size: 0.95rem;
    font-weight: 700;
    background: var(--warnung-bg);
    color: var(--warnung-text2);
  }
  .vorschau-bilanz.rot {
    background: var(--gefahr-bg);
    color: var(--gefahr-text);
  }
  .vorschau-bilanz.gruen {
    background: var(--erfolg-bg);
    color: var(--erfolg-text);
  }
  .vorschau-hinweis {
    margin: 12px 0 0;
    font-size: 0.82rem;
    color: var(--text-leise);
  }
</style>
