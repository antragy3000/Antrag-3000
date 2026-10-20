<script>
  // Arbeitsdokumente: Links zu Texten, die ihr gemeinsam außerhalb der App
  // schreibt (z. B. CryptPad). Zwei Ansichten derselben Projekt-Liste:
  //  - Projekt (foerderungId = null): alle Dokumente, mit Zuordnungen.
  //  - Förderung (foerderungId gesetzt): nur die zugeordneten; vorhandene
  //    Projekt-Dokumente lassen sich hier zuordnen („übernehmen").
  // Gespeichert wird nur Titel + Link, verschlüsselt im Tresor – nie im
  // Team-Abgleich (Logik und Begründung: $lib/arbeitsdokumente.js).
  import { openUrl } from "@tauri-apps/plugin-opener";
  import {
    linkPruefen,
    dokumentPruefen,
    dokumentAnlegen,
    dokumentAendern,
    dokumentLoeschen,
    zuordnen,
    zuordnungLoesen,
    fuerFoerderung,
    nochNichtZugeordnet,
    hostVon,
    TITEL_MAX,
    LINK_MAX,
  } from "$lib/arbeitsdokumente";

  let {
    projekt,
    speichern,
    teamModus = false,
    foerderungId = null,
    foerderungName = () => "",
  } = $props();

  let projektAnsicht = $derived(foerderungId == null);
  let dokumente = $derived(
    projektAnsicht ? (projekt.arbeitsdokumente ?? []) : fuerFoerderung(projekt, foerderungId),
  );
  let uebernehmbar = $derived(projektAnsicht ? [] : nochNichtZugeordnet(projekt, foerderungId));

  // Dialog „Neu/Bearbeiten": null = zu.
  /** @type {{ id: string | null, titel: string, link: string, fehler: string } | null} */
  let dialog = $state(null);
  let uebernehmenOffen = $state(false);

  function neu() {
    dialog = { id: null, titel: "", link: "", fehler: "" };
  }
  function bearbeiten(d) {
    dialog = { id: d.id, titel: d.titel, link: d.url, fehler: "" };
  }
  async function dialogSpeichern() {
    const d = dialog;
    if (!d) return;
    const r = dokumentPruefen(d.titel, d.link);
    if (!r.ok) {
      d.fehler = r.fehler ?? "";
      return;
    }
    if (d.id) dokumentAendern(projekt, d.id, r.titel, r.url);
    else dokumentAnlegen(projekt, r.titel, r.url, foerderungId);
    dialog = null;
    await speichern();
  }
  function fehlerWeg() {
    if (dialog) dialog.fehler = "";
  }
  function oeffnen(d) {
    // Beim Öffnen erneut prüfen – nur https geht ans Betriebssystem.
    const url = linkPruefen(d.url);
    if (url) openUrl(url);
  }
  async function loeschen(d) {
    const anzahl = (d.foerderungen ?? []).length;
    const zusatz = anzahl ? `\nEs ist ${anzahl} Förderung(en) zugeordnet; diese Zuordnungen werden mit entfernt.` : "";
    if (!confirm(`„${d.titel}" aus dem Projekt entfernen?${zusatz}\n\nDas Dokument selbst bei CryptPad bleibt unberührt.`)) return;
    dokumentLoeschen(projekt, d.id);
    await speichern();
  }
  async function loesen(d) {
    zuordnungLoesen(projekt, d.id, foerderungId);
    await speichern();
  }
  async function uebernehmen(d) {
    zuordnen(projekt, d.id, foerderungId);
    uebernehmenOffen = false;
    await speichern();
  }
  function zuordnungsText(d) {
    return (d.foerderungen ?? []).map((id) => foerderungName(id)).filter(Boolean).join(", ");
  }
</script>

<section class="dok-karte" class:eingebettet={!projektAnsicht}>
  <h3>{projektAnsicht ? "Arbeitsdokumente – gemeinsam schreiben" : "Arbeitsdokumente für diesen Antrag"}</h3>
  {#if projektAnsicht}
    <p class="leise">
      Links zu Texten, die ihr gemeinsam schreibt, z. B. in CryptPad. Die Texte selbst
      laufen nie durch Antrag 3000. In der Detailansicht einer gemerkten Förderung
      kannst du Dokumente dem jeweiligen Antrag zuordnen.
    </p>
  {/if}

  {#each dokumente as d (d.id)}
    <div class="zeile">
      <span class="symbol" aria-hidden="true">📄</span>
      <div class="text">
        <span class="titel">{d.titel}</span>
        <span class="host">{hostVon(d.url)}</span>
        {#if projektAnsicht && zuordnungsText(d)}
          <span class="zuordnung">für: {zuordnungsText(d)}</span>
        {/if}
      </div>
      <button class="oeffnen" onclick={() => oeffnen(d)}>Öffnen ↗</button>
      <button class="symbolknopf" title="Titel oder Link ändern" aria-label="Ändern" onclick={() => bearbeiten(d)}>✎</button>
      {#if projektAnsicht}
        <button class="symbolknopf gefahr" title="Aus dem Projekt entfernen" aria-label="Entfernen" onclick={() => loeschen(d)}>🗑</button>
      {:else}
        <button class="symbolknopf" title="Von diesem Antrag lösen – bleibt in der Projekt-Liste" aria-label="Zuordnung lösen" onclick={() => loesen(d)}>✕</button>
      {/if}
    </div>
  {:else}
    <p class="leer">
      {projektAnsicht ? "Noch keine Arbeitsdokumente." : "Diesem Antrag ist noch kein Arbeitsdokument zugeordnet."}
    </p>
  {/each}

  <div class="aktionen">
    <button class="hinzu" onclick={neu}>+ {projektAnsicht ? "Arbeitsdokument hinzufügen" : "Neues Dokument"}</button>
    {#if !projektAnsicht && uebernehmbar.length}
      <button class="hinzu" onclick={() => (uebernehmenOffen = !uebernehmenOffen)}>
        Aus dem Projekt übernehmen {uebernehmenOffen ? "▴" : "▾"}
      </button>
    {/if}
  </div>
  {#if uebernehmenOffen && uebernehmbar.length}
    <div class="auswahl">
      {#each uebernehmbar as d (d.id)}
        <div class="zeile">
          <span class="symbol" aria-hidden="true">📄</span>
          <div class="text"><span class="titel">{d.titel}</span><span class="host">{hostVon(d.url)}</span></div>
          <button class="oeffnen" onclick={() => uebernehmen(d)}>Zuordnen</button>
        </div>
      {/each}
    </div>
  {/if}
</section>

{#if dialog}
  <div class="schleier" onclick={() => (dialog = null)} role="presentation">
    <div class="dialog" onclick={(e) => e.stopPropagation()} role="presentation">
      <h2>{dialog.id ? "Arbeitsdokument ändern" : "Arbeitsdokument hinzufügen"}</h2>
      <label for="dok-titel">Titel</label>
      <input id="dok-titel" type="text" maxlength={TITEL_MAX} placeholder="z. B. Projektbeschreibung (gemeinsam)"
        bind:value={dialog.titel} oninput={fehlerWeg} />
      <label for="dok-link">Link</label>
      <input id="dok-link" type="text" maxlength={LINK_MAX} placeholder="https://cryptpad.fr/pad/#/2/pad/edit/…"
        bind:value={dialog.link} oninput={fehlerWeg} />
      {#if dialog.fehler}<p class="fehler">{dialog.fehler}</p>{/if}

      <p class="hinweis">
        🔑 Den Link bekommst du in CryptPad über „Teilen". Er enthält den
        <strong>Schlüssel zum Dokument</strong> – wer ihn hat, kann mitlesen.
        Antrag 3000 speichert ihn verschlüsselt und nur auf diesem Gerät.
      </p>
      {#if teamModus}
        <p class="hinweis team">
          👥 <strong>Du bist im Team-Modus – dieser Link wird nicht mit deinem Team
          geteilt.</strong> Gib das Dokument direkt in CryptPad frei (Knopf „Teilen"),
          dann bestimmst du selbst, wer Zugriff hat. Warum? Läge der Link im
          Team-Abgleich, wäre er samt Schlüssel auch auf dem Antrag-3000-Server
          gespeichert.
        </p>
      {/if}

      <div class="dialog-knoepfe">
        <button class="leise-knopf" onclick={() => (dialog = null)}>Abbrechen</button>
        <button class="primaer" onclick={dialogSpeichern}>{dialog.id ? "Speichern" : "Hinzufügen"}</button>
      </div>
    </div>
  </div>
{/if}

<style>
  .dok-karte {
    background: var(--weiss);
    border-radius: 12px;
    box-shadow: 0 1px 3px var(--schatten-sm);
    padding: 20px 24px;
    margin-bottom: 16px;
  }
  .dok-karte.eingebettet {
    box-shadow: none;
    border: 1px solid var(--rand);
    padding: 14px 16px;
    margin: 16px 0;
  }
  h3 {
    margin: 0 0 6px;
    font-size: 1rem;
  }
  .leise,
  .leer {
    margin: 0 0 10px;
    font-size: 0.85rem;
    color: var(--text-muted);
    line-height: 1.5;
  }
  .zeile {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 8px 0;
    border-top: 1px solid var(--flaeche-2b);
  }
  .symbol {
    font-size: 1rem;
  }
  .text {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-wrap: wrap;
    align-items: baseline;
    gap: 2px 8px;
  }
  .titel {
    font-weight: 600;
    font-size: 0.92rem;
    overflow-wrap: anywhere;
  }
  .host,
  .zuordnung {
    font-size: 0.8rem;
    color: var(--text-muted);
  }
  .zuordnung {
    flex-basis: 100%;
  }
  .oeffnen {
    padding: 5px 12px;
    font-size: 0.82rem;
    font-weight: 600;
    font-family: inherit;
    color: var(--auf-farbe);
    background: var(--akzent);
    border: none;
    border-radius: 8px;
    cursor: pointer;
    white-space: nowrap;
  }
  .oeffnen:hover {
    background: var(--akzent-d);
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
  .aktionen {
    display: flex;
    flex-wrap: wrap;
    gap: 6px 16px;
    margin-top: 8px;
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
  .auswahl {
    margin-top: 6px;
    padding: 4px 12px;
    background: var(--flaeche-2b);
    border-radius: 8px;
  }
  .auswahl .zeile:first-child {
    border-top: none;
  }

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
    max-height: 85vh;
    overflow-y: auto;
    box-shadow: 0 12px 40px var(--schatten-lg);
  }
  .dialog h2 {
    margin: 0 0 14px;
    font-size: 1.2rem;
  }
  .dialog label {
    display: block;
    margin: 12px 0 4px;
    font-size: 0.85rem;
    font-weight: 600;
    color: var(--text-2);
  }
  .dialog input {
    width: 100%;
    box-sizing: border-box;
    padding: 9px 11px;
    font-size: 0.92rem;
    font-family: inherit;
    border: 1px solid var(--rand);
    border-radius: 8px;
  }
  .fehler {
    margin: 6px 0 0;
    font-size: 0.85rem;
    color: var(--gefahr-text);
  }
  .hinweis {
    margin: 14px 0 0;
    padding: 10px 12px;
    font-size: 0.85rem;
    line-height: 1.5;
    background: var(--warnung-bg);
    color: var(--warnung-text2);
    border-radius: 8px;
  }
  .hinweis.team {
    background: var(--akzent-bg5);
    color: var(--link-d3);
  }
  .dialog-knoepfe {
    display: flex;
    justify-content: flex-end;
    gap: 10px;
    margin-top: 18px;
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
  .leise-knopf {
    background: none;
    border: none;
    color: var(--text-muted);
    font-size: 0.875rem;
    cursor: pointer;
    padding: 6px;
    font-family: inherit;
  }
  .leise-knopf:hover {
    color: var(--text);
    text-decoration: underline;
  }
</style>
