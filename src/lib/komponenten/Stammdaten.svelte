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
    color: #5e6c84;
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
    color: #216e4e;
    font-size: 0.88rem;
    font-weight: 600;
  }

  button {
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
  button:hover:not(:disabled) {
    background: #3d5bf0;
  }
  button:disabled {
    background: #c1c7d0;
    cursor: default;
  }

  .raster {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(340px, 1fr));
    gap: 16px;
    align-items: start;
  }

  .karte {
    background: #fff;
    border-radius: 12px;
    box-shadow: 0 1px 3px rgba(9, 30, 66, 0.12);
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
    color: #5e6c84;
    margin: 14px 0 5px;
  }
  input {
    width: 100%;
    box-sizing: border-box;
    padding: 9px 12px;
    font-size: 0.95rem;
    font-family: inherit;
    border: 2px solid #dfe1e6;
    border-radius: 8px;
    background: #fafbfc;
    transition: border-color 0.15s, background 0.15s;
  }
  input:focus {
    outline: none;
    border-color: #4f6df5;
    background: #fff;
  }

  .warnung {
    margin: 6px 0 0;
    color: #ae2e24;
    font-size: 0.82rem;
  }
</style>
