<script>
  import { onMount } from "svelte";
  import { ANTRAG_STATUS, statusLabel, statusFarbe } from "$lib/status";

  // Team-Synchronisation (Phase 2):
  //  - Mein Gerät: Zugangs-Paket laden / Verbindung testen.
  //  - Abgleich: fortlaufende Synchronisation starten/stoppen + Team-Übersicht.
  //  - Verwaltung: Team-CA und Zugangs-Pakete direkt in der App erzeugen
  //    (für die einrichtende Person). Der CA-Schlüssel bleibt verschlüsselt
  //    im Tresor und wird nie angezeigt.
  let {
    sync,
    teamCa,
    laden,
    testen,
    entfernen,
    adresseAendern,
    einladungAnnehmen,
    mitgliedEinladen,
    mitgliederHolen,
    mitgliedStatusSetzen,
    standardEnrollUrl = "",
    caErstellen,
    caExportieren,
    serverZert,
    paketErstellen,
    geraetEinrichten,
    starten,
    stoppen,
    pruefen,
    syncLaeuft = false,
    syncVerbunden = false,
    syncMeldung = null,
    zuletztGeprueft = null,
    protokoll = [],
    trockenlaufBauen,
    trockenlaufSenden,
    teamBoard,
    letzterAbgleich,
    meineProjektIds = [],
    foerderungLabel,
  } = $props();

  let beschaeftigt = $state(false);
  let status = $state(null);
  let pruefe = $state(false);
  let verwaltungOffen = $state(false);
  let pruefenOffen = $state(false);
  let caAdresse = $state("");
  let nasAdresse = $state("");
  let neuerGeraetName = $state("");
  let meinGeraetName = $state("");
  let vorschau = $state(null);          // Array der Sende-Körper (Trockenlauf)
  let mitschnittUrl = $state("http://127.0.0.1:8099");
  let mitschnittStatus = $state(null);

  function vorschauAnzeigen() {
    vorschau = trockenlaufBauen();
  }
  async function mitschnittSenden() {
    beschaeftigt = true;
    mitschnittStatus = null;
    try {
      mitschnittStatus = await trockenlaufSenden(mitschnittUrl.trim());
    } finally {
      beschaeftigt = false;
    }
  }

  // Beim Öffnen des Tabs einmal die Verbindung prüfen, damit der
  // Start-Knopf den richtigen Zustand zeigt (aktiv nur, wenn erreichbar).
  onMount(() => {
    if (sync && !syncLaeuft) verbindungPruefen();
  });

  async function verbindungPruefen() {
    pruefe = true;
    try {
      await pruefen();
    } finally {
      pruefe = false;
    }
  }

  function zeitText(iso) {
    if (!iso) return "noch nie";
    const d = new Date(iso);
    if (isNaN(d)) return iso;
    return d.toLocaleString("de-CH", {
      day: "2-digit", month: "2-digit", year: "numeric",
      hour: "2-digit", minute: "2-digit",
    });
  }

  function istMeines(projektId) {
    return meineProjektIds.includes(projektId);
  }

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

  // Gehostetes Modell (4b): Einladung annehmen / Mitglied einladen.
  let annehmenName = $state("");
  let einladenOffen = $state(false);
  let einladenName = $state("");
  let enrollUrl = $state(standardEnrollUrl);

  async function einladungAnnehmenKlick() {
    if (!annehmenName.trim()) return;
    beschaeftigt = true;
    status = null;
    try {
      const info = await einladungAnnehmen(annehmenName.trim());
      if (info) {
        annehmenName = "";
        status = await testen();
      }
    } finally {
      beschaeftigt = false;
    }
  }
  async function mitgliedEinladenKlick() {
    beschaeftigt = true;
    try {
      await mitgliedEinladen(enrollUrl.trim(), einladenName.trim());
      einladenName = "";
      if (mitgliederOffen) mitglieder = await mitgliederHolen();
    } finally {
      beschaeftigt = false;
    }
  }

  // Mitglieder verwalten (Eigentümer): Geräte auflisten + sperren/entsperren.
  let mitgliederOffen = $state(false);
  let mitglieder = $state(null);
  let mitgliederLaedt = $state(false);
  async function mitgliederUmschalten() {
    mitgliederOffen = !mitgliederOffen;
    if (mitgliederOffen && mitglieder === null) await mitgliederLaden();
  }
  async function mitgliederLaden() {
    mitgliederLaedt = true;
    try {
      mitglieder = await mitgliederHolen();
    } finally {
      mitgliederLaedt = false;
    }
  }
  async function geraetSperreUmschalten(m) {
    const neu = m.status === "gesperrt" ? "aktiv" : "gesperrt";
    if (neu === "gesperrt" && !confirm(`Gerät „${m.bezeichnung}" sperren? Es kann dann nicht mehr synchronisieren.`)) return;
    beschaeftigt = true;
    try {
      const ok = await mitgliedStatusSetzen(m.id, neu);
      if (ok) await mitgliederLaden();
    } finally {
      beschaeftigt = false;
    }
  }

  let adresseOffen = $state(false);
  let neueAdresse = $state("");
  async function adresseAendernKlick() {
    const a = neueAdresse.trim();
    if (!a) return;
    beschaeftigt = true;
    status = null;
    try {
      const ok = await adresseAendern(a);
      if (ok) {
        adresseOffen = false;
        neueAdresse = "";
        status = await testen(); // mit der neuen Adresse gleich neu testen
      }
    } finally {
      beschaeftigt = false;
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
  async function serverZertKlick() {
    const adr = (nasAdresse || teamCa?.adresse || "").trim();
    if (!adr) return;
    beschaeftigt = true;
    try {
      await serverZert(adr);
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
      <div class="block block-erste">
        <span class="block-titel">Mit einer Einladung verbinden</span>
        <p class="dezent">
          Du hast eine <strong>Einladung</strong> (Datei
          <code>.a3keinladung</code>) bekommen? Gib diesem Gerät einen Namen und
          verbinde dich. Dein Geräteschlüssel entsteht dabei nur auf diesem
          Gerät und verlässt es nie.
        </p>
        <div class="reihe">
          <input
            type="text"
            placeholder="Name dieses Geräts, z. B. Laptop-Anna"
            bind:value={annehmenName}
            onkeydown={(e) => { if (e.key === 'Enter' && annehmenName.trim()) einladungAnnehmenKlick(); }}
          />
          <button class="primaer" disabled={!annehmenName.trim() || beschaeftigt} onclick={einladungAnnehmenKlick}>
            {beschaeftigt ? "Verbindet …" : "Einladung annehmen …"}
          </button>
        </div>
      </div>
      <div class="block">
        <span class="block-titel">Oder: Zugangs-Paket</span>
        <p class="dezent">
          Von einer Verwalter:in ein <strong>Zugangs-Paket</strong> (Datei
          <code>.a3kpaket</code>) erhalten? Lade es hier.
        </p>
        <button class="zweit" disabled={beschaeftigt} onclick={paketWaehlen}>
          {beschaeftigt ? "Wird geladen …" : "📥 Zugangs-Paket wählen …"}
        </button>
      </div>
    </div>
  {:else}
    <div class="karte">
      <div class="zeile"><span class="etikett">Gerät</span><span class="wert">{sync.geraetName}</span></div>
      <div class="zeile">
        <span class="etikett">Team-Adresse</span>
        <span class="wert pfad">{sync.adresse}</span>
        <button class="leise klein" disabled={beschaeftigt} onclick={() => { adresseOffen = !adresseOffen; neueAdresse = sync.adresse; }}>
          {adresseOffen ? "abbrechen" : "ändern"}
        </button>
      </div>
      {#if adresseOffen}
        <div class="reihe adr-aendern">
          <input
            type="text"
            placeholder={sync.adresse}
            bind:value={neueAdresse}
            onkeydown={(e) => { if (e.key === "Enter" && neueAdresse.trim()) adresseAendernKlick(); }}
          />
          <button class="zweit" disabled={beschaeftigt || !neueAdresse.trim()} onclick={adresseAendernKlick}>
            Adresse speichern
          </button>
        </div>
        <p class="dezent klein">
          Nur die NAS-Adresse ändern – z. B. nach einem Tailscale-Wechsel. Dein
          Geräte-Ausweis und die Team-CA bleiben unverändert.
        </p>
      {/if}
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

  {#if sync}
    <h3 class="abschnitt zwischen">Team-Übersicht</h3>
    <div class="karte">
      <div class="sync-steuer">
        {#if !syncLaeuft}
          <button class="primaer" disabled={!syncVerbunden || pruefe} onclick={() => starten()}>
            ▶ Synchronisieren starten
          </button>
        {:else}
          <button class="stop" onclick={() => stoppen()}>■ Synchronisieren stoppen</button>
        {/if}
        <span class="licht {syncLaeuft ? (syncVerbunden ? 'an' : 'warn') : 'aus'}"></span>
        <span class="dezent zustand">
          {#if syncLaeuft}
            {syncVerbunden ? "Live – synchronisiert fortlaufend" : "Verbindung unterbrochen – versuche weiter …"}
          {:else if pruefe}
            Prüfe Verbindung …
          {:else if !syncVerbunden}
            Verbindung steht noch nicht – oben „Verbindung testen“.
          {:else}
            Bereit zum Start.
          {/if}
        </span>
      </div>

      {#if syncMeldung}
        <p class="klein meldung {syncMeldung.art === 'ok' ? 'ok' : syncMeldung.art === 'warn' ? 'fehler' : 'dezent'}">
          {syncMeldung.text}
        </p>
      {/if}
      <p class="dezent klein takt">
        Letzte Änderung übernommen: {zeitText(letzterAbgleich)}{#if zuletztGeprueft} · zuletzt geprüft: {zeitText(zuletztGeprueft)}{/if}
      </p>

      {#if !teamBoard || teamBoard.length === 0}
        <p class="dezent leer-hinweis">
          Noch keine Team-Projekte. Sobald die Synchronisation läuft, werden
          deine Projekte hochgeladen und die der anderen hier angezeigt.
        </p>
      {:else}
        <p class="hinweis-box">
          Schreibgeschützte Sicht. Hier erscheinen <strong>nur unkritische</strong>
          Felder (Projektname, Förder-Status, Fristen, Förderer-Kontakt) – keine
          Stammdaten, Budgets oder Projektbeschriebe.
        </p>
        <ul class="board">
          {#each teamBoard as eintrag (eintrag.projekt_id)}
            <li class="board-projekt">
              <div class="bp-kopf">
                <span class="bp-name">{eintrag.inhalt?.name || "Ohne Namen"}</span>
                {#if istMeines(eintrag.projekt_id)}
                  <span class="badge eigen">dieses Team-Gerät</span>
                {/if}
                <span class="bp-zeit">{zeitText(eintrag.geaendert_am)}</span>
              </div>
              {#if (eintrag.inhalt?.eintraege ?? []).length === 0}
                <p class="dezent klein nichts">keine gemerkten Förderungen</p>
              {:else}
                <ul class="foerderliste">
                  {#each eintrag.inhalt.eintraege as f}
                    <li class="foerd">
                      <span class="foerd-name">{foerderungLabel(f)}</span>
                      <span class="chip {statusFarbe(ANTRAG_STATUS, f.status)}">
                        {statusLabel(ANTRAG_STATUS, f.status, f.statusFrei)}
                      </span>
                    </li>
                  {/each}
                </ul>
              {/if}
            </li>
          {/each}
        </ul>
      {/if}
    </div>

    <button class="verwaltung-toggle" onclick={() => (einladenOffen = !einladenOffen)}>
      {einladenOffen ? "▾" : "▸"} Mitglied einladen
      <span class="dezent">(weiteres Gerät verbinden)</span>
    </button>

    {#if einladenOffen}
      <div class="karte verwaltung">
        <p>
          Erstellt eine <strong>Einladung</strong> (Datei
          <code>.a3keinladung</code>) für ein weiteres Gerät. Sie ist nur
          <strong>einmal</strong> nutzbar – gib sie offline weiter (der Code
          darin ist wie ein Schlüssel). Nur der Team-Eigentümer kann einladen.
        </p>
        <label for="enroll-url">Öffentliche Verbindungs-Adresse</label>
        <input id="enroll-url" type="text" placeholder="https://sync.antrag3000.de" bind:value={enrollUrl} />
        <div class="block">
          <span class="block-titel">Neues Gerät</span>
          <p class="dezent">Optionaler Name, damit du die Einladung zuordnen kannst.</p>
          <div class="reihe">
            <input type="text" placeholder="z. B. Tablet-Ben" bind:value={einladenName} />
            <button class="zweit" disabled={!enrollUrl.trim() || beschaeftigt} onclick={mitgliedEinladenKlick}>
              {beschaeftigt ? "Erstellt …" : "Einladung erstellen …"}
            </button>
          </div>
        </div>
      </div>
    {/if}

    <button class="verwaltung-toggle" onclick={mitgliederUmschalten}>
      {mitgliederOffen ? "▾" : "▸"} Mitglieder verwalten
      <span class="dezent">(Geräte im Team sperren)</span>
    </button>

    {#if mitgliederOffen}
      <div class="karte verwaltung">
        {#if mitgliederLaedt && mitglieder === null}
          <p class="dezent">Lädt …</p>
        {:else if !mitglieder}
          <p class="fehler">Konnte nicht geladen werden. Nur der Team-Eigentümer sieht die Mitglieder.</p>
          <button class="zweit" disabled={beschaeftigt} onclick={mitgliederLaden}>Erneut versuchen</button>
        {:else if mitglieder.length === 0}
          <p class="dezent">Noch keine Geräte.</p>
        {:else}
          <ul class="mitglieder">
            {#each mitglieder as m (m.id)}
              <li class="mitglied">
                <div class="m-info">
                  <span class="m-name">{m.bezeichnung}</span>
                  {#if m.ist_eigentuemer}<span class="badge eigen">Eigentümer</span>{/if}
                  {#if m.dieses_geraet}<span class="badge selbst">dieses Gerät</span>{/if}
                  <span class="chip {m.status === 'gesperrt' ? 'rot' : 'gruen'}">
                    {m.status === "gesperrt" ? "gesperrt" : "aktiv"}
                  </span>
                </div>
                <div class="m-zeile2">
                  <span class="dezent klein">zuletzt gesehen: {zeitText(m.zuletzt_gesehen)}</span>
                  {#if !m.dieses_geraet}
                    <button
                      class={m.status === "gesperrt" ? "zweit" : "leise sperren"}
                      disabled={beschaeftigt}
                      onclick={() => geraetSperreUmschalten(m)}
                    >
                      {m.status === "gesperrt" ? "Entsperren" : "Sperren"}
                    </button>
                  {/if}
                </div>
              </li>
            {/each}
          </ul>
        {/if}
      </div>
    {/if}

    <button class="verwaltung-toggle" onclick={() => (pruefenOffen = !pruefenOffen)}>
      {pruefenOffen ? "▾" : "▸"} Prüfen &amp; Protokoll
      <span class="dezent">(was verlässt das Gerät?)</span>
    </button>

    {#if pruefenOffen}
      <div class="karte verwaltung">
        <p>
          Hier kannst du <strong>selbst nachprüfen</strong>, welche Felder die
          App ins Netz geben würde. Sensible Daten (Stammdaten, IBAN, Steuer,
          Budget/KFP, Formular-Texte, Projektbeschriebe) sind baulich
          ausgeschlossen.
        </p>

        <div class="block">
          <span class="block-titel">Sende-Vorschau (Trockenlauf)</span>
          <p class="dezent">Zeigt die exakten Daten – ohne etwas zu senden.</p>
          <button class="zweit" onclick={vorschauAnzeigen}>Sende-Vorschau anzeigen</button>
          {#if vorschau}
            {#if vorschau.length === 0}
              <p class="dezent klein nichts">Keine Projekte – es würde nichts gesendet.</p>
            {:else}
              {#each vorschau as koerper}
                <pre class="payload">{koerper}</pre>
              {/each}
            {/if}
          {/if}
        </div>

        <div class="block">
          <span class="block-titel">Unabhängiger Mitschnitt</span>
          <p class="dezent">
            Starte im Projektordner <code>node tools/echo-server.mjs</code> und
            sende die Daten an diesen lokalen Server – im Terminal siehst du
            jedes Byte, das die App schicken würde (ohne NAS).
          </p>
          <div class="reihe">
            <input type="text" bind:value={mitschnittUrl} />
            <button class="zweit" disabled={beschaeftigt} onclick={mitschnittSenden}>
              {beschaeftigt ? "Sendet …" : "An lokalen Mitschnitt senden"}
            </button>
          </div>
          {#if mitschnittStatus}
            {#if mitschnittStatus.ok}
              <p class="ok klein">✓ {mitschnittStatus.n} Sende-Körper an den Mitschnitt geschickt – prüfe das Terminal.</p>
            {:else}
              <p class="fehler klein">⚠ {mitschnittStatus.fehler} (Läuft der Mitschnitt-Server?)</p>
            {/if}
          {/if}
        </div>

        <div class="block">
          <span class="block-titel">Sync-Protokoll</span>
          <p class="dezent">Was bei der laufenden Synchronisation tatsächlich gesendet/gelöscht wurde.</p>
          {#if protokoll.length === 0}
            <p class="dezent klein nichts">Noch nichts gesendet.</p>
          {:else}
            <ul class="protokoll">
              {#each protokoll as e}
                <li class="prot">
                  <span class="prot-zeit">{zeitText(e.zeit)}</span>
                  <span class="prot-text">
                    {#if e.trockenlauf}Trockenlauf → {e.ziel}{/if}
                    {#each e.zeilen as z}
                      <span class="prot-zeile">{z.aktion}: {z.projektId}{#if z.bytes} ({z.bytes} B){/if}</span>
                    {/each}
                  </span>
                </li>
              {/each}
            </ul>
          {/if}
        </div>
      </div>
    {/if}
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
          <span class="block-titel">1b · NAS-Server-Zertifikat (Tailscale)</span>
          <p class="dezent">
            Erzeugt <code>server.crt</code> + <code>server.key</code> für die
            Tailscale-Adresse deiner NAS (von deiner Team-CA signiert). Beide
            Dateien kommen ebenfalls zu Caddy. Diese Adresse wird auch für die
            Zugangs-Pakete verwendet.
          </p>
          <div class="reihe">
            <input
              type="text"
              placeholder={teamCa.adresse || "z. B. nas.dein-tailnet.ts.net"}
              bind:value={nasAdresse}
            />
            <button class="zweit" disabled={beschaeftigt} onclick={serverZertKlick}>
              Server-Zertifikat speichern …
            </button>
          </div>
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
    color: var(--text-muted);
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
    background: var(--weiss);
    border-radius: 12px;
    box-shadow: 0 1px 3px var(--schatten-sm);
    padding: 22px;
  }
  .karte.leer p {
    margin: 0 0 16px;
    font-size: 0.92rem;
    line-height: 1.55;
    color: var(--text-2);
  }
  code {
    background: var(--flaeche-2b);
    border-radius: 4px;
    padding: 1px 5px;
    font-size: 0.85rem;
  }
  .zeile {
    display: flex;
    gap: 14px;
    padding: 8px 0;
    border-bottom: 1px solid var(--flaeche-2b);
    font-size: 0.95rem;
  }
  .etikett {
    flex: 0 0 130px;
    color: var(--text-muted);
    font-weight: 600;
  }
  .wert {
    color: var(--text);
  }
  .pfad {
    font-family: "Consolas", "Courier New", monospace;
    font-size: 0.88rem;
  }
  .ok {
    margin: 0 0 4px;
    color: var(--erfolg-text);
    font-weight: 600;
  }
  .fehler {
    margin: 14px 0 0;
    color: var(--gefahr-text);
    font-weight: 600;
  }
  .knoepfe {
    display: flex;
    align-items: center;
    gap: 12px;
    margin-top: 18px;
  }

  .zwischen {
    margin-top: 28px;
  }
  .sync-steuer {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: 10px;
  }
  .licht {
    width: 10px;
    height: 10px;
    border-radius: 50%;
    flex: 0 0 auto;
  }
  .licht.aus { background: var(--grau-3); }
  .licht.an {
    background: var(--erfolg-2);
    box-shadow: 0 0 0 4px var(--erfolg-glow);
  }
  .licht.warn {
    background: var(--warnung-2);
    box-shadow: 0 0 0 4px var(--warnung-glow);
  }
  .zustand {
    font-size: 0.88rem;
  }
  .stop {
    padding: 10px 18px;
    font-size: 0.93rem;
    font-weight: 600;
    font-family: inherit;
    color: var(--weiss);
    background: var(--gefahr-2);
    border: none;
    border-radius: 8px;
    cursor: pointer;
    white-space: nowrap;
  }
  .stop:hover {
    background: var(--gefahr-text);
  }
  .klein {
    font-size: 0.85rem;
  }
  .meldung {
    margin: 12px 0 0;
  }
  .takt {
    margin: 8px 0 0;
  }
  .leer-hinweis {
    margin: 14px 0 0;
    line-height: 1.5;
  }
  .hinweis-box {
    margin: 14px 0 6px;
    padding: 10px 12px;
    background: var(--akzent-bg-x);
    border: 1px solid var(--akzent-bg8);
    border-radius: 8px;
    font-size: 0.82rem;
    line-height: 1.5;
    color: var(--text-2);
  }
  .board {
    list-style: none;
    margin: 8px 0 0;
    padding: 0;
  }
  .board-projekt {
    padding: 12px 0;
    border-top: 1px solid var(--flaeche-2b);
  }
  .bp-kopf {
    display: flex;
    align-items: center;
    gap: 8px;
    flex-wrap: wrap;
  }
  .bp-name {
    font-weight: 600;
    color: var(--text);
  }
  .bp-zeit {
    margin-left: auto;
    font-size: 0.78rem;
    color: var(--text-leise);
  }
  .badge.eigen {
    font-size: 0.72rem;
    font-weight: 600;
    color: var(--link);
    background: var(--akzent-bg3);
    border-radius: 10px;
    padding: 1px 8px;
  }
  .badge.selbst {
    font-size: 0.72rem;
    font-weight: 600;
    color: var(--text-muted);
    background: var(--flaeche-2b);
    border-radius: 10px;
    padding: 1px 8px;
  }
  .mitglieder {
    list-style: none;
    margin: 4px 0 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 10px;
  }
  .mitglied {
    padding: 12px 0;
    border-top: 1px solid var(--flaeche-2b);
  }
  .mitglied:first-child {
    border-top: none;
  }
  .m-info {
    display: flex;
    align-items: center;
    gap: 8px;
    flex-wrap: wrap;
  }
  .m-name {
    font-weight: 600;
    color: var(--text);
  }
  .m-info .chip {
    margin-left: auto;
  }
  .m-zeile2 {
    display: flex;
    align-items: center;
    gap: 10px;
    margin-top: 6px;
  }
  .m-zeile2 .dezent {
    margin-right: auto;
  }
  .leise.sperren {
    color: var(--gefahr-text);
    font-weight: 600;
  }
  .nichts {
    margin: 4px 0 0;
  }
  .foerderliste {
    list-style: none;
    margin: 8px 0 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 5px;
  }
  .foerd {
    display: flex;
    align-items: center;
    gap: 10px;
    font-size: 0.88rem;
  }
  .foerd-name {
    color: var(--text-2);
  }
  .chip {
    margin-left: auto;
    font-size: 0.74rem;
    font-weight: 600;
    border-radius: 10px;
    padding: 2px 9px;
    white-space: nowrap;
  }
  .chip.blau { background: var(--akzent-bg3); color: var(--link); }
  .chip.lila { background: var(--lila-bg3); color: var(--lila-d3); }
  .chip.gruen { background: var(--erfolg-bg2); color: var(--erfolg-text); }
  .chip.rot { background: var(--gefahr-bg2); color: var(--gefahr-text); }
  .chip.gelb { background: var(--warnung-bg3); color: var(--orange-d); }
  .chip.grau { background: var(--flaeche-2b); color: var(--text-muted); }

  .payload {
    margin: 8px 0 0;
    padding: 10px 12px;
    background: var(--text-tief);
    color: var(--akzent-bg7);
    border-radius: 8px;
    font-size: 0.78rem;
    line-height: 1.45;
    white-space: pre-wrap;
    word-break: break-word;
    max-height: 220px;
    overflow: auto;
  }
  .protokoll {
    list-style: none;
    margin: 8px 0 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .prot {
    display: flex;
    gap: 10px;
    font-size: 0.82rem;
  }
  .prot-zeit {
    flex: 0 0 64px;
    color: var(--text-leise);
    font-variant-numeric: tabular-nums;
  }
  .prot-text {
    display: flex;
    flex-direction: column;
    gap: 2px;
    color: var(--text-2);
  }
  .prot-zeile {
    color: var(--text);
  }

  .verwaltung-toggle {
    margin: 22px 0 0;
    background: none;
    border: none;
    color: var(--text-2);
    font-size: 0.95rem;
    font-weight: 600;
    font-family: inherit;
    cursor: pointer;
    padding: 6px 0;
  }
  .verwaltung-toggle:hover {
    color: var(--text);
  }
  .dezent {
    color: var(--text-leise);
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
    color: var(--text-2);
  }
  .block {
    padding: 14px 0;
    border-top: 1px solid var(--flaeche-2b);
  }
  .block.block-erste {
    border-top: none;
    padding-top: 0;
  }
  .block-titel {
    font-weight: 700;
    font-size: 0.9rem;
    color: var(--text);
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
    color: var(--text-muted);
    margin: 6px 0 5px;
  }
  input {
    width: 100%;
    box-sizing: border-box;
    padding: 9px 12px;
    font-size: 0.92rem;
    font-family: inherit;
    border: 2px solid var(--rand);
    border-radius: 8px;
    background: var(--flaeche);
  }
  input:focus {
    outline: none;
    border-color: var(--akzent);
    background: var(--weiss);
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
    color: var(--weiss);
    background: var(--akzent);
    border: none;
    border-radius: 8px;
    cursor: pointer;
    white-space: nowrap;
  }
  .primaer:hover:not(:disabled) {
    background: var(--akzent-d);
  }
  .primaer:disabled {
    background: var(--grau-3);
    cursor: default;
  }
  .zweit {
    padding: 9px 16px;
    font-size: 0.9rem;
    font-weight: 600;
    font-family: inherit;
    color: var(--text);
    background: var(--weiss);
    border: 2px solid var(--rand);
    border-radius: 8px;
    cursor: pointer;
    white-space: nowrap;
  }
  .zweit:hover:not(:disabled) {
    border-color: var(--akzent);
  }
  .zweit:disabled {
    color: var(--grau-4);
    background: var(--flaeche-2);
    cursor: default;
  }
  .leise {
    background: none;
    border: none;
    color: var(--text-muted);
    font-size: 0.9rem;
    font-family: inherit;
    cursor: pointer;
    padding: 8px;
  }
  .leise:hover:not(:disabled) {
    color: var(--gefahr-text);
    text-decoration: underline;
  }
  .zeile .leise.klein {
    margin-left: auto;
    padding: 0 6px;
    color: var(--akzent);
  }
  .zeile .leise.klein:hover:not(:disabled) {
    color: var(--akzent-d2);
  }
  .adr-aendern {
    margin: 8px 0 4px;
  }
</style>
