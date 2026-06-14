<script>
  // Team-Synchronisation (Phase 2): Zugangs-Paket laden, Verbindung
  // testen, Ausweis entfernen. Der Ausweis (mit privatem Schlüssel) liegt
  // verschlüsselt im Tresor – diese Komponente zeigt ihn nie an.
  let { sync, laden, testen, entfernen } = $props();

  let beschaeftigt = $state(false);
  let status = $state(null); // { ok, fehler } vom Verbindungstest

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
    if (
      confirm(
        "Geräte-Ausweis wirklich entfernen? Die Synchronisation ist dann gestoppt, " +
          "bis du wieder ein Zugangs-Paket lädst."
      )
    ) {
      await entfernen();
      status = null;
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

  {#if !sync}
    <div class="karte leer">
      <p>
        Noch kein Gerät eingerichtet. Lade dein <strong>Zugangs-Paket</strong>
        (Datei <code>.a3kpaket</code>), das du von deiner Verwalter:in bekommen
        hast. Es enthält deinen Ausweis und die Team-Adresse – du musst nichts
        abtippen.
      </p>
      <button class="primaer" disabled={beschaeftigt} onclick={paketWaehlen}>
        {beschaeftigt ? "Wird geladen …" : "📥 Zugangs-Paket wählen …"}
      </button>
    </div>
  {:else}
    <div class="karte">
      <div class="zeile">
        <span class="etikett">Gerät</span>
        <span class="wert">{sync.geraetName}</span>
      </div>
      <div class="zeile">
        <span class="etikett">Team-Adresse</span>
        <span class="wert pfad">{sync.adresse}</span>
      </div>

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
        <button class="leise" disabled={beschaeftigt} onclick={entfernenKlick}>
          Ausweis entfernen
        </button>
      </div>
    </div>

    <p class="hinweis">
      Der private Schlüssel deines Ausweises liegt verschlüsselt im Tresor und
      verlässt dieses Gerät nicht. Das Abgleichen der Board-Daten folgt im
      nächsten Schritt.
    </p>
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
  .karte {
    background: #fff;
    border-radius: 12px;
    box-shadow: 0 1px 3px rgba(9, 30, 66, 0.12);
    padding: 24px;
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
    margin: 14px 0 0;
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
  }
  .primaer:hover:not(:disabled) {
    background: #3d5bf0;
  }
  .primaer:disabled {
    background: #c1c7d0;
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
  .hinweis {
    margin: 14px 2px 0;
    font-size: 0.8rem;
    color: #8590a2;
    line-height: 1.5;
  }
</style>
