<script>
  // Stammdaten: wiederverwendbare sensible Angaben (Tresor-Inhalt).
  // Bearbeitet wird eine Arbeitskopie; erst "Speichern" schreibt sie
  // verschlüsselt zurück.
  let { stammdaten, speichern } = $props();

  let kopie = $state(structuredClone($state.snapshot(stammdaten)));
  let einmalGespeichert = $state(false);
  let beschaeftigt = $state(false);

  let veraendert = $derived(
    JSON.stringify($state.snapshot(kopie)) !==
      JSON.stringify($state.snapshot(stammdaten))
  );

  const GRUPPEN = [
    {
      key: "person",
      titel: "Person / Träger",
      felder: [
        ["vorname", "Vorname"],
        ["nachname", "Nachname"],
        ["kuenstlername", "Künstler:innenname (optional)"],
        ["organisation", "Organisation / Trägername (optional)"],
      ],
    },
    {
      key: "kontakt",
      titel: "Kontakt",
      felder: [
        ["strasse", "Straße und Hausnummer"],
        ["plz", "PLZ"],
        ["ort", "Ort"],
        ["land", "Land"],
        ["email", "E-Mail"],
        ["telefon", "Telefon"],
        ["webseite", "Webseite (optional)"],
      ],
    },
    {
      key: "bank",
      titel: "Bankverbindung",
      felder: [
        ["kontoinhaber", "Kontoinhaber:in"],
        ["iban", "IBAN"],
        ["bic", "BIC"],
        ["bank", "Name der Bank"],
      ],
    },
    {
      key: "steuer",
      titel: "Steuer",
      felder: [
        ["steuernummer", "Steuernummer"],
        ["ustid", "USt-IdNr. (optional)"],
        ["finanzamt", "Zuständiges Finanzamt"],
      ],
    },
  ];

  // Rechnerische IBAN-Prüfung (Prüfziffern-Verfahren Modulo 97).
  // Nur ein Hinweis bei Tippfehlern - blockiert nichts.
  function ibanGueltig(roh) {
    const s = roh.replace(/\s+/g, "").toUpperCase();
    if (!/^[A-Z]{2}[0-9]{2}[A-Z0-9]{11,30}$/.test(s)) return false;
    const umgestellt = s.slice(4) + s.slice(0, 4);
    let rest = 0;
    for (const zeichen of umgestellt) {
      const teil =
        zeichen >= "0" && zeichen <= "9"
          ? zeichen
          : (zeichen.charCodeAt(0) - 55).toString();
      for (const ziffer of teil) rest = (rest * 10 + Number(ziffer)) % 97;
    }
    return rest === 1;
  }

  let ibanWarnung = $derived(
    kopie.bank.iban.trim() !== "" && !ibanGueltig(kopie.bank.iban)
  );

  async function speichernKlick() {
    beschaeftigt = true;
    try {
      await speichern(structuredClone($state.snapshot(kopie)));
      einmalGespeichert = true;
    } finally {
      beschaeftigt = false;
    }
  }

  // --- Logo / Briefkopf ---
  // Das Logo wird als Data-URL in den Stammdaten gehalten (Tresor) und in
  // PDF/Word als Briefkopf eingefügt. Limit, damit der Tresor schlank bleibt.
  const MAX_LOGO = 1_500_000; // ~1,5 MB
  let logoFehler = $state("");

  function logoGewaehlt(e) {
    logoFehler = "";
    const f = e.target.files?.[0];
    e.target.value = ""; // erlaubt erneutes Wählen derselben Datei
    if (!f) return;
    if (!/^image\/(png|jpeg)$/.test(f.type)) {
      logoFehler = "Bitte ein PNG- oder JPG-Bild wählen.";
      return;
    }
    if (f.size > MAX_LOGO) {
      logoFehler = "Das Bild ist zu groß (max. 1,5 MB). Bitte kleiner speichern.";
      return;
    }
    const leser = new FileReader();
    leser.onload = () => { kopie.logo = leser.result; };
    leser.onerror = () => { logoFehler = "Das Bild konnte nicht gelesen werden."; };
    leser.readAsDataURL(f);
  }

  function logoEntfernen() {
    kopie.logo = "";
    logoFehler = "";
  }
</script>

<div class="bereich">
  <div class="kopfzeile">
    <div>
      <h2>Stammdaten</h2>
      <p class="untertitel">
        Diese Angaben bleiben verschlüsselt auf diesem Gerät und füllen
        später Formulare und Word-Dokumente automatisch.
      </p>
    </div>
    <div class="speichern-bereich">
      {#if !veraendert && einmalGespeichert}
        <span class="ok">✓ verschlüsselt gespeichert</span>
      {/if}
      <button disabled={!veraendert || beschaeftigt} onclick={speichernKlick}>
        {beschaeftigt ? "Speichert …" : "Speichern"}
      </button>
    </div>
  </div>

  <section class="karte logo-karte">
    <h3>Briefkopf / Logo</h3>
    <p class="logo-hinweis">
      Wird oben in die erzeugten Dokumente eingefügt (Antrags-PDF und
      Word-Projektbeschrieb) – <strong>nicht</strong> in Excel-Listen.
    </p>
    <div class="logo-zeile">
      {#if kopie.logo}
        <img class="logo-vorschau" src={kopie.logo} alt="Logo-Vorschau" />
      {:else}
        <div class="logo-platzhalter">kein Logo</div>
      {/if}
      <div class="logo-knoepfe">
        <label class="datei-knopf">
          {kopie.logo ? "Logo ersetzen …" : "Logo wählen …"}
          <input type="file" accept="image/png,image/jpeg" onchange={logoGewaehlt} hidden />
        </label>
        {#if kopie.logo}
          <button type="button" class="entfernen" onclick={logoEntfernen}>Entfernen</button>
        {/if}
        <span class="logo-tipp">PNG oder JPG, max. 1,5 MB. Nicht vergessen: „Speichern".</span>
      </div>
    </div>
    {#if logoFehler}<p class="warnung">{logoFehler}</p>{/if}
  </section>

  <div class="raster">
    {#each GRUPPEN as gruppe (gruppe.key)}
      <section class="karte">
        <h3>{gruppe.titel}</h3>
        {#each gruppe.felder as [feld, beschriftung] (feld)}
          <label for={gruppe.key + "-" + feld}>{beschriftung}</label>
          <input
            id={gruppe.key + "-" + feld}
            type={feld === "email" ? "email" : "text"}
            bind:value={kopie[gruppe.key][feld]}
          />
          {#if feld === "iban" && ibanWarnung}
            <p class="warnung">
              Diese IBAN scheint rechnerisch ungültig zu sein – bitte prüfen.
            </p>
          {/if}
        {/each}
      </section>
    {/each}
  </div>
</div>

<style>
  .bereich {
    max-width: 1080px;
    margin: 0 auto;
    padding: 32px 24px 64px;
  }

  .kopfzeile {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 16px;
    flex-wrap: wrap;
    margin-bottom: 24px;
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
    max-width: 520px;
    line-height: 1.5;
  }

  .speichern-bereich {
    display: flex;
    align-items: center;
    gap: 14px;
  }
  .ok {
    color: var(--erfolg-text);
    font-size: 0.88rem;
    font-weight: 600;
  }

  button {
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
  button:hover:not(:disabled) {
    background: var(--akzent-d);
  }
  button:disabled {
    background: var(--grau-3);
    cursor: default;
  }

  .raster {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(340px, 1fr));
    gap: 16px;
    align-items: start;
  }

  .karte {
    background: var(--weiss);
    border-radius: 12px;
    box-shadow: 0 1px 3px var(--schatten-sm);
    padding: 24px;
  }
  h3 {
    margin: 0 0 6px;
    font-size: 1.05rem;
    font-weight: 600;
  }

  label {
    display: block;
    font-size: 0.82rem;
    font-weight: 600;
    color: var(--text-muted);
    margin: 14px 0 5px;
  }
  input {
    width: 100%;
    box-sizing: border-box;
    padding: 9px 12px;
    font-size: 0.95rem;
    font-family: inherit;
    border: 2px solid var(--rand);
    border-radius: 8px;
    background: var(--flaeche);
    transition: border-color 0.15s, background 0.15s;
  }
  input:focus {
    outline: none;
    border-color: var(--akzent);
    background: var(--weiss);
  }

  .warnung {
    margin: 6px 0 0;
    color: var(--gefahr-text);
    font-size: 0.82rem;
  }

  .logo-karte { margin-bottom: 16px; }
  .logo-hinweis { margin: 0 0 14px; color: var(--text-muted); font-size: 0.88rem; line-height: 1.5; }
  .logo-zeile { display: flex; gap: 18px; align-items: center; flex-wrap: wrap; }
  .logo-vorschau {
    max-width: 220px; max-height: 90px; object-fit: contain;
    border: 1px solid var(--rand); border-radius: 8px; padding: 8px; background: var(--weiss);
  }
  .logo-platzhalter {
    width: 220px; height: 90px; display: grid; place-items: center;
    border: 2px dashed var(--rand); border-radius: 8px; color: var(--text-leise); font-size: 0.85rem; background: var(--flaeche);
  }
  .logo-knoepfe { display: flex; flex-direction: column; gap: 8px; align-items: flex-start; }
  .datei-knopf {
    display: inline-block; padding: 9px 16px; font-size: 0.9rem; font-weight: 600;
    color: var(--text); background: var(--weiss); border: 2px solid var(--rand); border-radius: 8px; cursor: pointer;
  }
  .datei-knopf:hover { border-color: var(--akzent); }
  .entfernen {
    padding: 7px 14px; font-size: 0.85rem; background: var(--weiss); color: var(--gefahr-text);
    border: 2px solid var(--gefahr-rand2); border-radius: 8px;
  }
  .entfernen:hover:not(:disabled) { background: var(--gefahr-bg); }
  .logo-tipp { color: var(--text-leise); font-size: 0.8rem; }
</style>
