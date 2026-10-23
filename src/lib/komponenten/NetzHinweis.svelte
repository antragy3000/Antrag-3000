<script>
  // Datenschutz-Hinweis (Paket 5): erklärt, wann sich die App mit dem
  // Internet verbindet, und lässt wählen: automatisch beim Start oder nur auf
  // Knopfdruck. Erscheint beim ersten Start (netzModus noch nicht gewählt –
  // dann ohne „Abbrechen", die Wahl ist nötig) und später über die Karte
  // „Internet-Verbindungen". Die Wahl liegt im Tresor (daten.netzModus).
  import { openUrl } from "@tauri-apps/plugin-opener";
  import { sichereWebUrl } from "$lib/sicherheit";

  // aktuell: null (noch nie gewählt) | "auto" | "knopf"
  // server: Anzeigename des Update-/Katalog-Servers (z. B. sync.antrag3000.de)
  // team: true im Team-Modus (dann zusätzlicher Absatz zum Team-Abgleich)
  let { aktuell = null, server = "sync.antrag3000.de", team = false, waehlen, schliessen = null } = $props();

  const DATENSCHUTZ_URL = "https://github.com/antragy3000/Antrag-3000/blob/master/DATENSCHUTZ.md";

  let wahl = $state(aktuell ?? "auto");
  let ersterStart = $derived(aktuell == null);

  function mehrLesen() {
    const u = sichereWebUrl(DATENSCHUTZ_URL);
    if (u) openUrl(u);
  }
</script>

<div class="schleier" onclick={() => !ersterStart && schliessen?.()} role="presentation">
  <div class="dialog" onclick={(e) => e.stopPropagation()} role="presentation">
    <h2>Datenschutz: Wann geht die App ins Internet?</h2>
    <p><strong>Deine Daten bleiben auf diesem Gerät.</strong> Stammdaten, Projekte,
      Formular-Texte, Kostenplan und Bankverbindung liegen verschlüsselt im Tresor.
      Antrag 3000 schickt sie nie über das Internet.</p>
    <p>Die App holt nur zwei Dinge vom Antrag-3000-Server (<code>{server}</code>):</p>
    <ul>
      <li><strong>App-Updates</strong> – ob es eine neue Version gibt.</li>
      <li><strong>Die Förder-Datenbank</strong> – die öffentliche Liste der Förderprogramme.</li>
    </ul>
    <p class="klein">Dabei wird nichts von dir mitgeschickt. Wie bei jeder Internet-Verbindung
      sieht der Server die IP-Adresse deines Anschlusses.</p>
    {#if team}
      <p class="klein">Im <strong>Team-Modus</strong> gleicht die App zusätzlich Projektnamen,
        Status und Fristen mit eurem Team ab – aber erst, wenn du die Synchronisation startest.</p>
    {/if}

    <fieldset>
      <legend>Wann soll die App nachsehen?</legend>
      <label class="wahl" class:aktiv={wahl === "auto"}>
        <input type="radio" name="netz" value="auto" bind:group={wahl} />
        <span>
          <strong>Automatisch beim Start</strong> (empfohlen)<br />
          <span class="klein">Du bekommst Sicherheits-Updates und neue Förderungen sofort.</span>
        </span>
      </label>
      <label class="wahl" class:aktiv={wahl === "knopf"}>
        <input type="radio" name="netz" value="knopf" bind:group={wahl} />
        <span>
          <strong>Nur auf Knopfdruck</strong><br />
          <span class="klein">Die App verbindet sich nur, wenn du „⬆ Update" oder „Förder-Datenbank
            aktualisieren" drückst. Schau dann ab und zu selbst nach Updates.</span>
        </span>
      </label>
    </fieldset>

    <p class="klein">Du kannst das jederzeit ändern: <strong>Stammdaten &amp; Team →
      Internet-Verbindungen</strong>.
      <button class="link" onclick={mehrLesen}>Ausführliche Datenschutz-Erklärung</button></p>

    <div class="knoepfe">
      {#if !ersterStart}
        <button class="leise" onclick={() => schliessen?.()}>Abbrechen</button>
      {/if}
      <button class="primaer" onclick={() => waehlen(wahl)}>
        {ersterStart ? "Weiter" : "Speichern"}
      </button>
    </div>
  </div>
</div>

<style>
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
    padding: 28px 32px;
    max-width: 520px;
    width: 100%;
    max-height: 88vh;
    overflow-y: auto;
    box-shadow: 0 12px 40px var(--schatten-lg);
  }
  h2 {
    margin: 0 0 14px;
    font-size: 1.2rem;
  }
  p,
  li {
    font-size: 0.92rem;
    line-height: 1.55;
  }
  p {
    margin: 0 0 10px;
  }
  ul {
    margin: 0 0 10px;
    padding-left: 20px;
  }
  li {
    margin-bottom: 4px;
  }
  code {
    font-size: 0.85rem;
  }
  .klein {
    color: var(--text-muted);
    font-size: 0.84rem;
  }
  fieldset {
    border: none;
    margin: 14px 0 12px;
    padding: 0;
  }
  legend {
    font-size: 0.88rem;
    font-weight: 600;
    color: var(--text-2);
    margin-bottom: 8px;
    padding: 0;
  }
  .wahl {
    display: flex;
    gap: 10px;
    align-items: flex-start;
    padding: 10px 12px;
    margin-bottom: 8px;
    border: 1px solid var(--rand);
    border-radius: 8px;
    font-size: 0.9rem;
    line-height: 1.45;
    cursor: pointer;
  }
  .wahl.aktiv {
    border-color: var(--akzent);
    background: var(--akzent-bg);
  }
  .wahl input {
    width: auto;
    margin-top: 3px;
  }
  .link {
    padding: 0;
    background: none;
    border: none;
    color: var(--link);
    font-size: inherit;
    font-family: inherit;
    text-decoration: underline;
    cursor: pointer;
  }
  .knoepfe {
    display: flex;
    justify-content: flex-end;
    gap: 10px;
    margin-top: 16px;
  }
  .primaer {
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
  .primaer:hover {
    background: var(--akzent-d);
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
</style>
