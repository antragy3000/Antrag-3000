<script>
  // Förder-Datenbank (Phase 3 / Etappe 2):
  //  - Stand der aktiven Katalog-Fassung anzeigen.
  //  - Update aus Datei automatisch übernehmen + Änderungs-Hinweis.
  //  - Auf Werkszustand zurücksetzen.
  //  - Fehler/veraltete Daten melden (lokale Warteschlange; Versand an
  //    den Server kommt mit der NAS in Etappe 3).
  import { katalog } from "$lib/katalog.svelte.js";

  let {
    schliessen,
    updateAusDatei,
    updateVomServer = null,
    syncEingerichtet = false,
    zuruecksetzen,
    meldungen = [],
    meldungAnlegen,
    meldungEntfernen,
  } = $props();

  let beschaeftigt = $state(false);
  let ergebnis = $state(null);   // { ok, diff, stand } | { ok:false, fehler }
  let resetFrage = $state(false);

  // Melde-Formular
  let mFoerderung = $state("");
  let mArt = $state("veraltet");
  let mText = $state("");
  let mFehler = $state(null);

  const ART_LABEL = {
    veraltet: "veraltete Angaben",
    falsch: "falsche Angaben",
    weg: "Förderung gibt es nicht mehr",
    sonstiges: "Sonstiges",
  };

  let foerderListe = $derived(
    [...(katalog.daten.foerderungen ?? [])].sort((a, b) => a.name.localeCompare(b.name)),
  );

  function standText(iso) {
    if (!iso) return "–";
    const d = new Date(iso);
    return isNaN(d) ? iso : d.toLocaleDateString("de-CH");
  }
  function zeitText(iso) {
    const d = new Date(iso);
    return isNaN(d) ? iso : d.toLocaleString("de-CH", {
      day: "2-digit", month: "2-digit", year: "numeric", hour: "2-digit", minute: "2-digit",
    });
  }

  async function updateLaden() {
    beschaeftigt = true;
    ergebnis = null;
    try {
      const r = await updateAusDatei();
      if (r) ergebnis = r; // null = Dialog abgebrochen
    } finally {
      beschaeftigt = false;
    }
  }

  async function updateServer() {
    beschaeftigt = true;
    ergebnis = null;
    try {
      ergebnis = await updateVomServer();
    } finally {
      beschaeftigt = false;
    }
  }

  async function zuruecksetzenJetzt() {
    beschaeftigt = true;
    try {
      const r = await zuruecksetzen();
      ergebnis = r.ok ? { ok: true, zurueckgesetzt: true } : r;
      resetFrage = false;
    } finally {
      beschaeftigt = false;
    }
  }

  function melden() {
    mFehler = null;
    const f = foerderListe.find((x) => x.id === mFoerderung);
    const r = meldungAnlegen(mFoerderung, f?.name ?? "", mArt, mText);
    if (!r.ok) { mFehler = r.fehler; return; }
    mText = "";
    mFoerderung = "";
  }

  let summe = $derived(
    ergebnis?.ok && ergebnis.diff
      ? ergebnis.diff.neu.length + ergebnis.diff.geaendert.length + ergebnis.diff.entfernt.length
      : 0,
  );
</script>

<div class="schleier" onclick={schliessen} role="presentation">
  <div class="dialog" onclick={(e) => e.stopPropagation()} role="presentation">
    <h2>Förder-Datenbank</h2>

    <section>
      <h3>Aktueller Stand</h3>
      <div class="zeile"><span class="etikett">Quelle</span>
        <span>{katalog.quelle === "server"
          ? "aktualisiert vom Team-Server"
          : katalog.quelle === "datei"
            ? "aktualisierte Fassung (Datei)"
            : "mitgelieferte Standard-Fassung"}</span></div>
      <div class="zeile"><span class="etikett">Stand</span><span>{standText(katalog.daten.stand)}</span></div>
      <div class="zeile"><span class="etikett">Förderungen</span><span>{katalog.daten.foerderungen?.length ?? 0}</span></div>
    </section>

    <section>
      <h3>Update einspielen</h3>
      <p class="dezent">
        Ein Update wird nach einer Prüfung <strong>automatisch übernommen</strong>;
        anschließend siehst du, was sich geändert hat.
      </p>
      <div class="knoepfe">
        {#if updateVomServer}
          <button class="primaer" disabled={beschaeftigt || !syncEingerichtet} onclick={updateServer}>
            {beschaeftigt ? "Wird geprüft …" : "🔄 Vom Server holen"}
          </button>
        {/if}
        <button class="zweit" disabled={beschaeftigt} onclick={updateLaden}>
          Update aus Datei laden …
        </button>
        <button class="leise" disabled={beschaeftigt} onclick={() => (resetFrage = true)}>
          Auf Werkszustand zurücksetzen
        </button>
      </div>
      {#if updateVomServer && !syncEingerichtet}
        <p class="dezent klein">Für den Abruf zuerst im Reiter „Stammdaten &amp; Team" den Modus einrichten (Einzelplatz-Server oder Team-Paket).</p>
      {/if}

      {#if resetFrage}
        <div class="hinweisbox warn">
          Den mitgelieferten Stand wiederherstellen und das Update verwerfen?
          <div class="knoepfe" style="margin-top:8px">
            <button class="zweit" onclick={() => (resetFrage = false)}>Abbrechen</button>
            <button class="gefahr" disabled={beschaeftigt} onclick={zuruecksetzenJetzt}>Ja, zurücksetzen</button>
          </div>
        </div>
      {/if}

      {#if ergebnis && !ergebnis.ok}
        <p class="fehler">⚠ {ergebnis.fehler}</p>
      {:else if ergebnis?.zurueckgesetzt}
        <p class="ok">✓ Auf die mitgelieferte Standard-Fassung zurückgesetzt.</p>
      {:else if ergebnis?.ok}
        {#if summe === 0}
          <p class="ok">✓ Übernommen – keine inhaltlichen Änderungen.</p>
        {:else}
          <div class="hinweisbox ok-box">
            <strong>✓ Update übernommen</strong> (Stand {standText(ergebnis.stand)}):
            {ergebnis.diff.neu.length} neu · {ergebnis.diff.geaendert.length} geändert ·
            {ergebnis.diff.entfernt.length} entfernt.
            {#if ergebnis.diff.neu.length}
              <div class="gruppe"><span class="g-titel neu">Neu</span>
                {#each ergebnis.diff.neu as e}<div class="g-zeile">{e.name}</div>{/each}</div>
            {/if}
            {#if ergebnis.diff.geaendert.length}
              <div class="gruppe"><span class="g-titel geaendert">Geändert</span>
                {#each ergebnis.diff.geaendert as e}<div class="g-zeile">{e.name}</div>{/each}</div>
            {/if}
            {#if ergebnis.diff.entfernt.length}
              <div class="gruppe"><span class="g-titel entfernt">Entfernt</span>
                {#each ergebnis.diff.entfernt as e}<div class="g-zeile">{e.name}</div>{/each}</div>
              <p class="dezent klein">Hinweis: Entfernte Förderungen verschwinden aus den Listen, aber deine gespeicherten Status-/Checklisten-Daten dazu werden <strong>nicht gelöscht</strong> – sie sind wieder da, falls die Förderung zurückkommt. (Eine deutliche „nicht mehr im Katalog"-Kennzeichnung folgt in 2b.)</p>
            {/if}
          </div>
        {/if}
      {/if}
    </section>

    <section>
      <h3>Problem melden</h3>
      <p class="dezent">
        Stimmt etwas nicht (veraltete Frist, falscher Geber, Förderung gibt es
        nicht mehr)? Melde es – die Meldung wird beim <strong>laufenden
        Synchronisieren</strong> automatisch ans Team/an die Pflege gesendet
        (sonst bleibt sie „noch nicht gesendet").
      </p>
      <label for="m-f">Förderung</label>
      <select id="m-f" bind:value={mFoerderung}>
        <option value="">– bitte wählen –</option>
        {#each foerderListe as f (f.id)}<option value={f.id}>{f.name}</option>{/each}
      </select>
      <label for="m-a">Art</label>
      <select id="m-a" bind:value={mArt}>
        {#each Object.entries(ART_LABEL) as [k, v]}<option value={k}>{v}</option>{/each}
      </select>
      <label for="m-t">Anmerkung (optional)</label>
      <textarea id="m-t" rows="2" maxlength="500" bind:value={mText} placeholder="Was stimmt nicht?"></textarea>
      {#if mFehler}<p class="fehler klein">⚠ {mFehler}</p>{/if}
      <div class="knoepfe">
        <button class="zweit" disabled={!mFoerderung} onclick={melden}>Melden</button>
      </div>

      {#if meldungen.length > 0}
        <div class="meldungen">
          {#each meldungen as m (m.id)}
            <div class="m-zeile">
              <div>
                <span class="m-art">{ART_LABEL[m.art] ?? m.art}</span> · {m.foerderungName || m.foerderungId}
                {#if m.text}<div class="dezent klein">„{m.text}"</div>{/if}
                <div class="dezent klein">{zeitText(m.zeit)} · {m.gesendet ? "gesendet" : "noch nicht gesendet"}</div>
              </div>
              <button class="loeschen" onclick={() => meldungEntfernen(m.id)} title="Meldung entfernen">✕</button>
            </div>
          {/each}
        </div>
      {/if}
    </section>

    <div class="fuss">
      <button class="leise" onclick={schliessen}>Schließen</button>
    </div>
  </div>
</div>

<style>
  .schleier {
    position: fixed; inset: 0; background: rgba(9, 30, 66, 0.45);
    display: grid; place-items: center; padding: 24px; z-index: 40;
  }
  .dialog {
    background: #fff; border-radius: 12px; padding: 32px;
    max-width: 520px; width: 100%; max-height: 86vh; overflow-y: auto;
    box-shadow: 0 12px 40px rgba(9, 30, 66, 0.3);
  }
  h2 { margin: 0 0 16px; font-size: 1.2rem; }
  h3 { margin: 0 0 6px; font-size: 1rem; font-weight: 600; }
  section { padding: 16px 0; border-top: 1px solid #f1f2f4; }
  section:first-of-type { border-top: none; }
  p { margin: 0 0 10px; }
  .dezent { color: #5e6c84; font-size: 0.9rem; line-height: 1.5; }
  .klein { font-size: 0.82rem; }
  .zeile { display: flex; gap: 12px; padding: 4px 0; font-size: 0.95rem; }
  .etikett { flex: 0 0 110px; color: #5e6c84; font-weight: 600; }
  .knoepfe { display: flex; gap: 10px; align-items: center; flex-wrap: wrap; margin-top: 6px; }
  .ok { color: #216e4e; font-weight: 600; margin: 10px 0 0; }
  .fehler { color: #ae2e24; font-weight: 600; margin: 10px 0 0; }
  .hinweisbox { border-radius: 8px; padding: 10px 12px; margin-top: 12px; font-size: 0.9rem; }
  .hinweisbox.warn { background: #fff7d6; color: #533f04; }
  .ok-box { background: #e3fcef; color: #143d2b; }
  .gruppe { margin-top: 8px; }
  .g-titel { font-size: 0.72rem; font-weight: 700; text-transform: uppercase; letter-spacing: .03em; }
  .g-titel.neu { color: #216e4e; }
  .g-titel.geaendert { color: #a54800; }
  .g-titel.entfernt { color: #ae2e24; }
  .g-zeile { font-size: 0.88rem; padding: 1px 0 1px 8px; }

  label { display: block; font-size: 0.8rem; font-weight: 600; color: #5e6c84; margin: 10px 0 4px; }
  select, textarea {
    width: 100%; box-sizing: border-box; padding: 8px 10px; font-size: 0.92rem;
    font-family: inherit; border: 2px solid #dfe1e6; border-radius: 8px; background: #fafbfc;
  }
  select:focus, textarea:focus { outline: none; border-color: #4f6df5; background: #fff; }

  .meldungen { margin-top: 12px; display: flex; flex-direction: column; gap: 6px; }
  .m-zeile {
    display: flex; justify-content: space-between; gap: 10px; align-items: flex-start;
    border: 1px solid #f1f2f4; border-radius: 8px; padding: 8px 10px; font-size: 0.9rem;
  }
  .m-art { font-weight: 600; color: #172b4d; }
  .loeschen { background: none; border: none; color: #8590a2; cursor: pointer; font-size: 0.95rem; }
  .loeschen:hover { color: #ae2e24; }

  .primaer {
    padding: 9px 16px; font-size: 0.92rem; font-weight: 600; font-family: inherit;
    color: #fff; background: #4f6df5; border: none; border-radius: 8px; cursor: pointer;
  }
  .primaer:hover:not(:disabled) { background: #3d5bf0; }
  .primaer:disabled { background: #c1c7d0; cursor: default; }
  .zweit {
    padding: 8px 14px; font-size: 0.9rem; font-weight: 600; font-family: inherit;
    color: #172b4d; background: #fff; border: 2px solid #dfe1e6; border-radius: 8px; cursor: pointer;
  }
  .zweit:hover:not(:disabled) { border-color: #4f6df5; }
  .zweit:disabled { color: #b3bac5; background: #f4f5f7; cursor: default; }
  .gefahr {
    padding: 8px 14px; font-size: 0.9rem; font-weight: 600; font-family: inherit;
    color: #fff; background: #ca3521; border: none; border-radius: 8px; cursor: pointer;
  }
  .gefahr:hover:not(:disabled) { background: #ae2e24; }
  .leise {
    background: none; border: none; color: #5e6c84; font-size: 0.9rem;
    font-family: inherit; cursor: pointer; padding: 8px;
  }
  .leise:hover { color: #172b4d; text-decoration: underline; }
  .fuss { display: flex; justify-content: flex-end; margin-top: 14px; }
</style>
