<script>
  // Team-Synchronisation (Phase 2):
  //  - Mein Gerät: Zugangs-Paket laden / Verbindung testen.
  //  - Verwaltung: Team-CA und Zugangs-Pakete direkt in der App erzeugen
  //    (für die einrichtende Person). Der CA-Schlüssel bleibt verschlüsselt
  //    im Tresor und wird nie angezeigt.
  let {
    sync,
    teamCa,
    laden,
    testen,
    entfernen,
    caErstellen,
    caExportieren,
    paketErstellen,
    geraetEinrichten,
  } = $props();

  let beschaeftigt = $state(false);
  let status = $state(null);
  let verwaltungOffen = $state(false);
  let caAdresse = $state("");
  let neuerGeraetName = $state("");
  let meinGeraetName = $state("");

  async function paketWaehlen() {
    beschaeftigt = true;
    status = null;
    try {
      const info = await laden();
      if (info) status = await testen();
    } finally {
      beschaeftigt = false;
    }
  }
  async function verbindungTesten() {
    beschaeftigt = true;
    status = null;
    try {
      status = await testen();
    } finally {
      beschaeftigt = false;
    }
  }
  async function entfernenKlick() {
    if (confirm("Geräte-Ausweis entfernen? Die Synchronisation ist dann gestoppt.")) {
      await entfernen();
      status = null;
    }
  }

  async function caErstellenKlick() {
    if (!caAdresse.trim()) return;
    beschaeftigt = true;
    try {
      await caErstellen(caAdresse.trim());
    } finally {
      beschaeftigt = false;
    }
  }
  async function paketErstellenKlick() {
    if (!neuerGeraetName.trim()) return;
    beschaeftigt = true;
    try {
      await paketErstellen(neuerGeraetName.trim());
      neuerGeraetName = "";
    } finally {
      beschaeftigt = false;
    }
  }
  async function geraetEinrichtenKlick() {
    if (!meinGeraetName.trim()) return;
    beschaeftigt = true;
    status = null;
    try {
      const info = await geraetEinrichten(meinGeraetName.trim());
      if (info) status = await testen();
    } finally {
      beschaeftigt = false;
    }
  }
</script>

<div class="bereich">
  <h2>Team-Synchronisation</h2>
  <p class="untertitel">
    Teilt nur <strong>unkritische</strong> Daten (Förder-Status, Merklisten,
    Fristen) mit deinem Team über deine eigene NAS. Sensible Daten bleiben
    verschlüsselt auf diesem Gerät.
  </p>

  <h3 class="abschnitt">Mein Gerät</h3>
  {#if !sync}
    <div class="karte leer">
      <p>
        Noch kein Gerät eingerichtet. Lade dein <strong>Zugangs-Paket</strong>
        (Datei <code>.a3kpaket</code>) von deiner Verwalter:in – es enthält
        deinen Ausweis und die Team-Adresse.
      </p>
      <button class="primaer" disabled={beschaeftigt} onclick={paketWaehlen}>
        {beschaeftigt ? "Wird geladen …" : "📥 Zugangs-Paket wählen …"}
      </button>
    </div>
  {:else}
    <div class="karte">
      <div class="zeile"><span class="etikett">Gerät</span><span class="wert">{sync.geraetName}</span></div>
      <div class="zeile"><span class="etikett">Team-Adresse</span><span class="wert pfad">{sync.adresse}</span></div>
      {#if status}
        {#if status.ok}
          <p class="ok">✓ Verbunden – der Team-Server ist erreichbar.</p>
        {:else}
          <p class="fehler">⚠ {status.fehler ?? "Nicht erreichbar."}</p>
        {/if}
      {/if}
      <div class="knoepfe">
        <button class="primaer" disabled={beschaeftigt} onclick={verbindungTesten}>
          {beschaeftigt ? "Prüft …" : "Verbindung testen"}
        </button>
        <button class="leise" disabled={beschaeftigt} onclick={entfernenKlick}>Ausweis entfernen</button>
      </div>
    </div>
  {/if}

  <button class="verwaltung-toggle" onclick={() => (verwaltungOffen = !verwaltungOffen)}>
    {verwaltungOffen ? "▾" : "▸"} Team verwalten <span class="dezent">(für die einrichtende Person)</span>
  </button>

  {#if verwaltungOffen}
    <div class="karte verwaltung">
      {#if !teamCa}
        <p>
          Hier richtest du <strong>einmalig</strong> das Team ein: Die App
          erzeugt die <strong>Team-CA</strong> (den „Aussteller-Stempel").
          <strong>Nur eine Person</strong> im Team macht das – der Schlüssel
          bleibt verschlüsselt in deinem Tresor.
        </p>
        <label for="ca-adr">Team-Adresse (DDNS deiner NAS)</label>
        <input id="ca-adr" type="text" placeholder="deinteam.synology.me" bind:value={caAdresse} />
        <button class="primaer" disabled={!caAdresse.trim() || beschaeftigt} onclick={caErstellenKlick}>
          {beschaeftigt ? "Wird erstellt …" : "Team-CA erstellen"}
        </button>
      {:else}
        <p class="ok">✓ Team-CA aktiv · Adresse <code>{teamCa.adresse}</code></p>

        <div class="block">
          <span class="block-titel">1 · Für die NAS</span>
          <p class="dezent">Das öffentliche CA-Zertifikat kommt zu Caddy auf die NAS.</p>
          <button class="zweit" disabled={beschaeftigt} onclick={() => caExportieren()}>
            Team-CA-Zertifikat speichern …
          </button>
        </div>

        <div class="block">
          <span class="block-titel">2 · Gerät hinzufügen</span>
          <p class="dezent">Erzeugt ein Zugangs-Paket, das du offline an die Person gibst.</p>
          <div class="reihe">
            <input type="text" placeholder="z. B. Laptop-Anna" bind:value={neuerGeraetName} />
            <button class="zweit" disabled={!neuerGeraetName.trim() || beschaeftigt} onclick={paketErstellenKlick}>
              Zugangs-Paket erstellen …
            </button>
          </div>
        </div>

        {#if !sync}
          <div class="block">
            <span class="block-titel">3 · Dieses Gerät direkt einrichten</span>
            <p class="dezent">Ohne Umweg über eine Datei – richtet sofort dieses Gerät ein.</p>
            <div class="reihe">
              <input type="text" placeholder="z. B. Laptop-Jenny" bind:value={meinGeraetName} />
              <button class="primaer" disabled={!meinGeraetName.trim() || beschaeftigt} onclick={geraetEinrichtenKlick}>
                Einrichten
              </button>
            </div>
          </div>
        {/if}
      {/if}
    </div>
  {/if}
</div>

<style>
  .bereich {
    max-width: 680px;
    margin: 0 auto;
    padding: 32px 24px 64px;
  }
  h2 {
    margin: 0 0 4px;
    font-size: 1.35rem;
    font-weight: 600;
  }
  .untertitel {
    margin: 0 0 20px;
    color: #5e6c84;
    font-size: 0.92rem;
    line-height: 1.55;
    max-width: 560px;
  }
  .abschnitt {
    margin: 0 0 10px;
    font-size: 1.02rem;
    font-weight: 600;
  }
  .karte {
    background: #fff;
    border-radius: 12px;
    box-shadow: 0 1px 3px rgba(9, 30, 66, 0.12);
    padding: 22px;
  }
  .karte.leer p {
    margin: 0 0 16px;
    font-size: 0.92rem;
    line-height: 1.55;
    color: #44546f;
  }
  code {
    background: #f1f2f4;
    border-radius: 4px;
    padding: 1px 5px;
    font-size: 0.85rem;
  }
  .zeile {
    display: flex;
    gap: 14px;
    padding: 8px 0;
    border-bottom: 1px solid #f1f2f4;
    font-size: 0.95rem;
  }
  .etikett {
    flex: 0 0 130px;
    color: #5e6c84;
    font-weight: 600;
  }
  .wert {
    color: #172b4d;
  }
  .pfad {
    font-family: "Consolas", "Courier New", monospace;
    font-size: 0.88rem;
  }
  .ok {
    margin: 0 0 4px;
    color: #216e4e;
    font-weight: 600;
  }
  .fehler {
    margin: 14px 0 0;
    color: #ae2e24;
    font-weight: 600;
  }
  .knoepfe {
    display: flex;
    align-items: center;
    gap: 12px;
    margin-top: 18px;
  }

  .verwaltung-toggle {
    margin: 22px 0 0;
    background: none;
    border: none;
    color: #44546f;
    font-size: 0.95rem;
    font-weight: 600;
    font-family: inherit;
    cursor: pointer;
    padding: 6px 0;
  }
  .verwaltung-toggle:hover {
    color: #172b4d;
  }
  .dezent {
    color: #8590a2;
    font-weight: 400;
    font-size: 0.85rem;
  }
  .verwaltung {
    margin-top: 8px;
  }
  .verwaltung > p {
    margin: 0 0 14px;
    font-size: 0.9rem;
    line-height: 1.55;
    color: #44546f;
  }
  .block {
    padding: 14px 0;
    border-top: 1px solid #f1f2f4;
  }
  .block-titel {
    font-weight: 700;
    font-size: 0.9rem;
    color: #172b4d;
  }
  .block .dezent {
    display: block;
    margin: 3px 0 8px;
    line-height: 1.5;
  }

  label {
    display: block;
    font-size: 0.82rem;
    font-weight: 600;
    color: #5e6c84;
    margin: 6px 0 5px;
  }
  input {
    width: 100%;
    box-sizing: border-box;
    padding: 9px 12px;
    font-size: 0.92rem;
    font-family: inherit;
    border: 2px solid #dfe1e6;
    border-radius: 8px;
    background: #fafbfc;
  }
  input:focus {
    outline: none;
    border-color: #4f6df5;
    background: #fff;
  }
  .reihe {
    display: flex;
    gap: 10px;
    align-items: center;
  }
  .reihe input {
    flex: 1;
  }

  .primaer {
    padding: 10px 18px;
    font-size: 0.93rem;
    font-weight: 600;
    font-family: inherit;
    color: #fff;
    background: #4f6df5;
    border: none;
    border-radius: 8px;
    cursor: pointer;
    white-space: nowrap;
  }
  .primaer:hover:not(:disabled) {
    background: #3d5bf0;
  }
  .primaer:disabled {
    background: #c1c7d0;
    cursor: default;
  }
  .zweit {
    padding: 9px 16px;
    font-size: 0.9rem;
    font-weight: 600;
    font-family: inherit;
    color: #172b4d;
    background: #fff;
    border: 2px solid #dfe1e6;
    border-radius: 8px;
    cursor: pointer;
    white-space: nowrap;
  }
  .zweit:hover:not(:disabled) {
    border-color: #4f6df5;
  }
  .zweit:disabled {
    color: #b3bac5;
    background: #f4f5f7;
    cursor: default;
  }
  .leise {
    background: none;
    border: none;
    color: #5e6c84;
    font-size: 0.9rem;
    font-family: inherit;
    cursor: pointer;
    padding: 8px;
  }
  .leise:hover:not(:disabled) {
    color: #ae2e24;
    text-decoration: underline;
  }
</style>
