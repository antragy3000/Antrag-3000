<script>
  // Zusammenarbeit im Projekt (Reiter „Dateien", oben): Arbeitsdokumente
  // zum gemeinsamen Schreiben + ein geteilter Ablageort für Dateien.
  // Die App lädt NICHTS hoch: Sie merkt sich nur die Links (verschlüsselt,
  // nur auf diesem Gerät) und öffnet den Projektordner, damit Dateien von
  // Hand in den geteilten Ordner gezogen werden können (Grundprinzip 1).
  import { openUrl } from "@tauri-apps/plugin-opener";
  import Arbeitsdokumente from "./Arbeitsdokumente.svelte";
  import { linkPruefen, hostVon, LINK_MAX } from "$lib/arbeitsdokumente";

  let { projekt, speichern, teamModus = false, ordnerOeffnen = null, foerderungName = () => "" } = $props();

  let bearbeiten = $state(false);
  let eingabe = $state("");
  let fehler = $state("");

  function ablageBearbeiten() {
    eingabe = projekt.ablageUrl ?? "";
    fehler = "";
    bearbeiten = true;
  }
  async function ablageSpeichern() {
    const url = linkPruefen(eingabe);
    if (!url) {
      fehler = "Bitte einen vollständigen https-Link angeben (z. B. https://cryptpad.fr/drive/…).";
      return;
    }
    projekt.ablageUrl = url;
    bearbeiten = false;
    await speichern();
  }
  async function ablageEntfernen() {
    if (!confirm("Geteilten Ablageort aus dem Projekt entfernen?\n\nDer Ordner beim Dienst selbst bleibt unberührt.")) return;
    projekt.ablageUrl = "";
    await speichern();
  }
  function ablageOeffnen() {
    const url = linkPruefen(projekt.ablageUrl);
    if (url) openUrl(url);
  }
</script>

<div class="zusammenarbeit">
  <Arbeitsdokumente {projekt} {speichern} {teamModus} {foerderungName} />

  <section class="karte">
    <h3>Dateien teilen</h3>
    <p class="leise">
      Ein geteilter Ordner bei einem Dienst deiner Wahl, z. B. CryptPad-Drive oder
      Nextcloud. Dateien ziehst du selbst aus dem Projektordner hinüber –
      Antrag 3000 lädt nichts hoch.
    </p>

    {#if bearbeiten}
      <label for="ablage-link">Link zum geteilten Ordner</label>
      <div class="eingabe-zeile">
        <input id="ablage-link" type="text" maxlength={LINK_MAX} placeholder="https://cryptpad.fr/drive/#/…"
          bind:value={eingabe} oninput={() => (fehler = "")} />
        <button class="primaer" onclick={ablageSpeichern}>Speichern</button>
        <button class="leise-knopf" onclick={() => (bearbeiten = false)}>Abbrechen</button>
      </div>
      {#if fehler}<p class="fehler">{fehler}</p>{/if}
      {#if teamModus}
        <p class="hinweis">
          👥 Im Team-Modus wird auch dieser Link <strong>nicht</strong> mit dem Team geteilt –
          er kann einen Zugangsschlüssel enthalten. Gib den Ordner direkt beim Dienst frei.
        </p>
      {/if}
    {:else if projekt.ablageUrl}
      <div class="ablage">
        <span class="text">Geteilter Ablageort: <span class="host">{hostVon(projekt.ablageUrl)}</span></span>
        <button class="primaer klein" onclick={ablageOeffnen}>Öffnen ↗</button>
        <button class="symbolknopf" title="Link ändern" aria-label="Ändern" onclick={ablageBearbeiten}>✎</button>
        <button class="symbolknopf gefahr" title="Link entfernen" aria-label="Entfernen" onclick={ablageEntfernen}>🗑</button>
      </div>
    {:else}
      <button class="hinzu" onclick={ablageBearbeiten}>+ Geteilten Ablageort eintragen</button>
    {/if}

    {#if ordnerOeffnen}
      <div class="ordner">
        <button class="zweit" onclick={ordnerOeffnen}>📁 Projektordner öffnen</button>
      </div>
    {/if}
  </section>
</div>

<style>
  .zusammenarbeit {
    margin-bottom: 28px;
  }
  .karte {
    background: var(--weiss);
    border-radius: 12px;
    box-shadow: 0 1px 3px var(--schatten-sm);
    padding: 20px 24px;
  }
  h3 {
    margin: 0 0 6px;
    font-size: 1rem;
  }
  .leise {
    margin: 0 0 10px;
    font-size: 0.85rem;
    color: var(--text-muted);
    line-height: 1.5;
  }
  label {
    display: block;
    margin: 4px 0 4px;
    font-size: 0.85rem;
    font-weight: 600;
    color: var(--text-2);
  }
  .eingabe-zeile {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .eingabe-zeile input {
    flex: 1;
    min-width: 0;
    padding: 8px 11px;
    font-size: 0.92rem;
    font-family: inherit;
    border: 1px solid var(--rand);
    border-radius: 8px;
  }
  .ablage {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 6px 0;
  }
  .text {
    flex: 1;
    font-size: 0.92rem;
  }
  .host {
    font-weight: 600;
    color: var(--link);
  }
  .fehler {
    margin: 6px 0 0;
    font-size: 0.85rem;
    color: var(--gefahr-text);
  }
  .hinweis {
    margin: 10px 0 0;
    padding: 8px 12px;
    font-size: 0.85rem;
    line-height: 1.5;
    background: var(--akzent-bg5);
    color: var(--link-d3);
    border-radius: 8px;
  }
  .ordner {
    margin-top: 12px;
  }
  .primaer {
    padding: 8px 16px;
    font-size: 0.88rem;
    font-weight: 600;
    font-family: inherit;
    color: var(--auf-farbe);
    background: var(--akzent);
    border: none;
    border-radius: 8px;
    cursor: pointer;
    white-space: nowrap;
  }
  .primaer.klein {
    padding: 5px 12px;
    font-size: 0.82rem;
  }
  .primaer:hover {
    background: var(--akzent-d);
  }
  .zweit {
    padding: 8px 16px;
    font-size: 0.88rem;
    font-weight: 600;
    font-family: inherit;
    color: var(--text);
    background: var(--weiss);
    border: 2px solid var(--rand);
    border-radius: 8px;
    cursor: pointer;
  }
  .zweit:hover {
    border-color: var(--akzent);
  }
  .hinzu {
    background: none;
    border: none;
    padding: 4px 0;
    font-size: 0.88rem;
    font-weight: 600;
    font-family: inherit;
    color: var(--link);
    cursor: pointer;
  }
  .hinzu:hover {
    text-decoration: underline;
  }
  .symbolknopf {
    background: none;
    border: none;
    color: var(--text-leise);
    font-size: 0.95rem;
    cursor: pointer;
    padding: 4px 6px;
    border-radius: 6px;
  }
  .symbolknopf:hover {
    background: var(--flaeche-2b);
    color: var(--text);
  }
  .symbolknopf.gefahr:hover {
    background: var(--gefahr-bg);
    color: var(--gefahr-text);
  }
  .leise-knopf {
    background: none;
    border: none;
    color: var(--text-muted);
    font-size: 0.875rem;
    cursor: pointer;
    padding: 6px;
    font-family: inherit;
  }
</style>
