<script>
  // Antrag-Status (eine Förderoption) + Checkliste der benötigten
  // Dokumente, je mit eigenem Status. Wird in der Detailansicht einer
  // gemerkten Förderung gezeigt. Mutiert das antrag-Objekt (Tresor)
  // und ruft danach aendern() zum verschlüsselten Speichern.
  import { openUrl } from "@tauri-apps/plugin-opener";
  import { ANTRAG_STATUS, CHECK_STATUS, statusFarbe } from "$lib/status";
  import { sichereWebUrl, sichereMailUrl } from "$lib/sicherheit";

  let {
    antrag,
    aendern,
    hochladen = null,
    pdfVorschau = null,
    pdfSpeichern = null,
    // Manche Förderer nehmen Anträge NUR über ihr eigenes Online-Formular
    // entgegen. Dann gibt es statt „per Mail senden" einen Verweis dorthin.
    einreichOnline = false,
    einreichUrl = "",
  } = $props();

  // Die Einreich-Adresse kann aus dem (synchronisierten) Katalog stammen
  // und ist deshalb nicht vertrauenswürdig: nur eine echte http/https-URL
  // wird geöffnet (sonst null → der Online-Knopf erscheint gar nicht).
  let onlineUrl = $derived(sichereWebUrl(einreichUrl));
  function onlineFormularOeffnen() {
    if (onlineUrl) openUrl(onlineUrl);
  }

  let neuerPunkt = $state("");
  let neueFrist = $state("");
  let neuerFristTitel = $state("");
  // Index des Punkts, dessen Datei gerade hochgeladen wird (-1 = keiner).
  let laedtIdx = $state(-1);

  // --- Antrags-PDF: Bereitschaft, Hinweis, Vorschau/Speichern ----------
  // Der Knopf ist aktiv, wenn ALLE benötigten Dokumente den Status
  // "abgeschlossen" haben UND alle hochgeladen sind. Eine LEERE Checkliste
  // (man darf alle Dokumente entfernen) gilt als bereit – das PDF enthält
  // dann Stammblatt, Formular und KFP, aber keine Anhänge.
  let alleAbgeschlossen = $derived(
    antrag.checkliste.every((p) => p.status === "abgeschlossen")
  );
  let alleHochgeladen = $derived(
    antrag.checkliste.every((p) => (p.datei || "").trim() !== "")
  );
  let pdfBereit = $derived(alleAbgeschlossen && alleHochgeladen);
  let pdfHinweis = $derived(
    !alleAbgeschlossen
      ? "nicht alle erforderlichen Dateien haben den Status abgeschlossen"
      : !alleHochgeladen
        ? "es wurden noch nicht alle Dateien hochgeladen"
        : antrag.checkliste.length === 0
          ? "Antrags-PDF aus Stammdaten, Formular und KFP erstellen (keine Anhänge hinterlegt)"
          : "Antrags-PDF aus Stammdaten, Formular, KFP und Anhängen erstellen"
  );

  let pdfModalOffen = $state(false);
  let pdfBeschaeftigt = $state(false);
  let pdfGespeichert = $state("");

  async function pdfErstellenKlick() {
    if (!pdfVorschau || !pdfBereit) return;
    pdfBeschaeftigt = true;
    pdfGespeichert = "";
    try {
      const ok = await pdfVorschau();
      if (ok) pdfModalOffen = true;
    } finally {
      pdfBeschaeftigt = false;
    }
  }
  async function pdfSpeichernKlick() {
    if (!pdfSpeichern) return;
    pdfBeschaeftigt = true;
    try {
      const pfad = await pdfSpeichern();
      if (pfad) {
        pdfGespeichert = pfad;
        pdfModalOffen = false;
      }
    } finally {
      pdfBeschaeftigt = false;
    }
  }

  // E-Mail-Versand: Kontaktdaten des Förderers (aus dem Antrag-Eintrag).
  let kontaktEmail = $derived((antrag.kontakt?.email ?? "").trim());
  let kontaktName = $derived((antrag.kontakt?.ansprechpartner ?? "").trim());
  let mailHinweis = $derived(
    kontaktEmail
      ? "Speichert das PDF und öffnet eine vorbereitete Mail an die Kontaktperson"
      : "noch keine E-Mailadresse der Kontaktperson eingetragen"
  );

  // Speichert das PDF und öffnet eine vorbereitete Mail an die
  // Kontaktperson. Die Datei kann mailto nicht selbst anhängen – darum
  // wird der Ordner geöffnet (von pdfSpeichern) und der Pfad im Text
  // genannt, damit der Nutzer sie hineinzieht.
  async function pdfMailKlick() {
    if (!pdfSpeichern || !kontaktEmail) return;
    pdfBeschaeftigt = true;
    try {
      const pfad = await pdfSpeichern();
      if (pfad) {
        pdfGespeichert = pfad;
        // Empfänger-Adresse erst absichern (gültige Einzeladresse, keine
        // versteckten mailto-Felder), dann Betreff/Text anhängen.
        const mailtoBasis = sichereMailUrl(kontaktEmail);
        if (mailtoBasis) {
          const betreff = "Förderantrag";
          const text =
            `Guten Tag${kontaktName ? " " + kontaktName : ""},\n\n` +
            `im Anhang sende ich Ihnen meinen Förderantrag.\n` +
            `Bitte hängen Sie diese Datei an:\n${pfad}\n\n` +
            `Mit freundlichen Grüßen`;
          const adresse =
            `${mailtoBasis}` +
            `?subject=${encodeURIComponent(betreff)}` +
            `&body=${encodeURIComponent(text)}`;
          openUrl(adresse);
        }
        pdfModalOffen = false;
      }
    } finally {
      pdfBeschaeftigt = false;
    }
  }

  function punktHinzufuegen(event) {
    event.preventDefault();
    const t = neuerPunkt.trim();
    if (!t) return;
    antrag.checkliste.push({ text: t, status: "noch_nicht", statusFrei: "", datei: "" });
    neuerPunkt = "";
    aendern();
  }

  // Dokument zu einem Checklisten-Punkt hochladen (Datei-Dialog +
  // Kopie in den Förderer-Ordner; merkt sich nur den Dateinamen).
  async function dateiHochladen(i) {
    if (!hochladen) return;
    laedtIdx = i;
    try {
      const name = await hochladen(antrag.checkliste[i].text);
      if (name) {
        antrag.checkliste[i].datei = name;
        aendern();
      }
    } finally {
      laedtIdx = -1;
    }
  }
  // Nur die Verknüpfung lösen – die Datei selbst bleibt im Ordner.
  function dateiEntfernen(i) {
    antrag.checkliste[i].datei = "";
    aendern();
  }
  function punktEntfernen(i) {
    antrag.checkliste.splice(i, 1);
    aendern();
  }

  // Offizielle Einreichfrist(en) – editierbar
  function offFristAendern(i, wert) {
    antrag.offizielleFristen[i] = wert;
    aendern();
  }
  function offFristEntfernen(i) {
    antrag.offizielleFristen.splice(i, 1);
    aendern();
  }
  function offFristNeuSlot() {
    antrag.offizielleFristen.push("");
  }

  // Eigene (benannte) Fristen
  function fristHinzufuegen(event) {
    event.preventDefault();
    if (!neueFrist) return;
    antrag.eigeneFristen.push({ datum: neueFrist, titel: neuerFristTitel.trim() });
    antrag.eigeneFristen.sort((a, b) => a.datum.localeCompare(b.datum));
    neueFrist = "";
    neuerFristTitel = "";
    aendern();
  }
  function fristEntfernen(i) {
    antrag.eigeneFristen.splice(i, 1);
    aendern();
  }
  function fristAnzeige(d) {
    return new Date(d).toLocaleDateString("de-DE", {
      day: "2-digit",
      month: "long",
      year: "numeric",
    });
  }
</script>

<section class="antrag">
  <div class="status-zeile">
    <h4>Antrag-Status</h4>
    <span class="punkt farbe-{statusFarbe(ANTRAG_STATUS, antrag.status)}"></span>
    <select bind:value={antrag.status} onchange={aendern}>
      {#each ANTRAG_STATUS as s (s.key)}
        <option value={s.key}>{s.label}</option>
      {/each}
    </select>
  </div>
  {#if antrag.status === "anderer"}
    <input
      class="frei"
      type="text"
      placeholder="Status frei beschriften …"
      bind:value={antrag.statusFrei}
      onchange={aendern}
    />
  {/if}

  <h4 class="check-titel">Kontakt zum Förderer</h4>
  <div class="kontakt">
    <div class="zwei">
      <div>
        <label for="kt-name">Ansprechpartner:in</label>
        <input id="kt-name" type="text" bind:value={antrag.kontakt.ansprechpartner} onchange={aendern} />
      </div>
      <div>
        <label for="kt-tel">Telefon</label>
        <input id="kt-tel" type="text" bind:value={antrag.kontakt.telefon} onchange={aendern} />
      </div>
    </div>
    <label for="kt-mail">E-Mail</label>
    <input id="kt-mail" type="email" bind:value={antrag.kontakt.email} onchange={aendern} />
    <label for="kt-notiz">
      Notiz
      <span class="nur-lokal">(verlässt Dein Gerät nicht und wird nicht synchronisiert, ist also nur für Dich)</span>
    </label>
    <textarea id="kt-notiz" rows="2" bind:value={antrag.kontakt.notiz} onchange={aendern}></textarea>
  </div>

  <h4 class="check-titel">Offizielle Einreichfrist</h4>
  <div class="off-fristen">
    {#each antrag.offizielleFristen ?? [] as d, i (i)}
      <span class="off-frist">
        <input
          type="date"
          value={d}
          onchange={(e) => offFristAendern(i, e.currentTarget.value)}
        />
        <button class="entfernen" title="Frist entfernen" onclick={() => offFristEntfernen(i)}>✕</button>
      </span>
    {/each}
    <button type="button" class="off-add" onclick={offFristNeuSlot}>+ Frist</button>
  </div>
  <p class="hinweis-klein">
    Aus der Datenbank vorbefüllt – falls falsch übernommen, hier korrigieren.
  </p>

  <h4 class="check-titel">Eigene Fristen</h4>
  {#if (antrag.eigeneFristen ?? []).length === 0}
    <p class="leer">Keine eigenen Fristen. Trage unten z. B. interne Abgabetermine ein.</p>
  {/if}
  <ul class="fristen">
    {#each antrag.eigeneFristen ?? [] as f, i (i)}
      <li>
        <span class="frist-datum">
          📅 {fristAnzeige(f.datum)}{#if f.titel} – {f.titel}{/if}
        </span>
        <button class="entfernen" title="Frist entfernen" onclick={() => fristEntfernen(i)}>✕</button>
      </li>
    {/each}
  </ul>
  <form class="hinzufuegen eigene-frist" onsubmit={fristHinzufuegen}>
    <input type="date" bind:value={neueFrist} />
    <input type="text" placeholder="Bezeichnung (z. B. Teamabgabe)" bind:value={neuerFristTitel} />
    <button type="submit" disabled={!neueFrist}>+ Frist</button>
  </form>

  <h4 class="check-titel">Benötigte Dokumente</h4>
  {#if antrag.checkliste.length === 0}
    <p class="leer">Noch keine Punkte. Füge unten die nötigen Dokumente hinzu.</p>
  {/if}
  <ul class="checkliste">
    {#each antrag.checkliste as punkt, i (punkt)}
      <li>
        <span class="punkt farbe-{statusFarbe(CHECK_STATUS, punkt.status)}"></span>
        <div class="punkt-inhalt">
          <span class="punkt-text" class:fertig={punkt.status === "abgeschlossen"}>
            {punkt.text}
          </span>
          <div class="punkt-status">
            <select bind:value={punkt.status} onchange={aendern}>
              {#each CHECK_STATUS as s (s.key)}
                <option value={s.key}>{s.label}</option>
              {/each}
            </select>
            {#if punkt.status === "anderer"}
              <input
                class="frei"
                type="text"
                placeholder="frei beschriften …"
                bind:value={punkt.statusFrei}
                onchange={aendern}
              />
            {/if}
          </div>
          {#if hochladen}
            <div class="datei-zeile">
              {#if punkt.datei}
                <span class="datei-name" title={punkt.datei}>📎 {punkt.datei}</span>
                <button class="datei-knopf" disabled={laedtIdx === i} onclick={() => dateiHochladen(i)}>
                  {laedtIdx === i ? "lädt …" : "ersetzen"}
                </button>
                <button class="datei-entfernen" title="Verknüpfung entfernen" onclick={() => dateiEntfernen(i)}>✕</button>
              {:else}
                <button class="datei-knopf hochladen" disabled={laedtIdx === i} onclick={() => dateiHochladen(i)}>
                  {laedtIdx === i ? "lädt …" : "⬆ Datei hochladen"}
                </button>
              {/if}
            </div>
          {/if}
        </div>
        <button class="entfernen" title="Punkt entfernen" onclick={() => punktEntfernen(i)}>
          ✕
        </button>
      </li>
    {/each}
  </ul>

  <form class="hinzufuegen" onsubmit={punktHinzufuegen}>
    <input type="text" placeholder="Weiteres Dokument …" bind:value={neuerPunkt} />
    <button type="submit" disabled={!neuerPunkt.trim()}>+ Hinzufügen</button>
  </form>

  {#if einreichOnline}
    <div class="online-bereich">
      <p class="online-hinweis">
        ℹ Dieser Förderer nimmt Anträge <strong>nur über sein eigenes
        Online-Formular</strong> entgegen – nicht per Mail. Bereite die
        Unterlagen als PDF vor und lade sie dort hoch.
      </p>
      {#if onlineUrl}
        <button class="online-knopf" onclick={onlineFormularOeffnen}>
          🌐 Zum Online-Formular
        </button>
      {:else}
        <p class="online-fehlt">Die Adresse des Online-Formulars ist noch nicht hinterlegt – siehe Webseite des Förderers.</p>
      {/if}
    </div>
  {/if}

  {#if pdfVorschau}
    <div class="pdf-bereich">
      <button
        class="pdf-knopf"
        disabled={!pdfBereit || pdfBeschaeftigt}
        title={pdfHinweis}
        onclick={pdfErstellenKlick}
      >
        {pdfBeschaeftigt && !pdfModalOffen ? "Erzeugt …" : "📄 Antrags-PDF erstellen"}
      </button>
      {#if pdfGespeichert}
        <p class="pdf-ok">✓ Gespeichert: {pdfGespeichert}</p>
      {/if}
    </div>
  {/if}
</section>

{#if pdfModalOffen}
  <div class="pdf-schleier" onclick={() => (pdfModalOffen = false)} role="presentation">
    <div class="pdf-dialog" onclick={(e) => e.stopPropagation()} role="presentation">
      <h3>Antrags-PDF – Vorschau</h3>
      <p>
        Die Vorschau wurde in deinem PDF-Programm geöffnet – prüfe sie dort.
        Passt alles, speichere das fertige PDF in den Förderer-Ordner.
      </p>
      <div class="pdf-aktionen">
        <button class="primaer" disabled={pdfBeschaeftigt} onclick={pdfSpeichernKlick}>
          {pdfBeschaeftigt ? "Speichert …" : "✓ Speichern"}
        </button>
        {#if einreichOnline}
          <button
            class="zweit"
            disabled={pdfBeschaeftigt || !onlineUrl}
            title={onlineUrl ? "Speichert das PDF und öffnet das Online-Formular des Förderers" : "Keine Online-Formular-Adresse hinterlegt"}
            onclick={async () => { await pdfSpeichernKlick(); onlineFormularOeffnen(); }}
          >
            🌐 und zum Online-Formular
          </button>
        {:else}
          <button
            class="zweit"
            disabled={pdfBeschaeftigt || !kontaktEmail}
            title={mailHinweis}
            onclick={pdfMailKlick}
          >
            ✉ und per Mail senden
          </button>
        {/if}
        <button class="leise" disabled={pdfBeschaeftigt} onclick={() => (pdfModalOffen = false)}>
          Abbrechen
        </button>
      </div>
      <p class="pdf-mail-hinweis">
        {#if einreichOnline}
          „Zum Online-Formular" speichert das PDF in den Förderer-Ordner und
          öffnet das Online-Formular – die gespeicherten Dateien lädst du dort
          hoch.
        {:else}
          „Per Mail senden" öffnet eine vorbereitete E-Mail an die Kontaktperson
          und den Ordner mit dem PDF – die Datei ziehst du noch selbst als Anhang
          hinein (das ist bei jedem Mailprogramm so).
        {/if}
      </p>
    </div>
  </div>
{/if}

<style>
  .antrag {
    border-top: 1px solid #dfe1e6;
    margin-top: 20px;
    padding-top: 20px;
  }
  h4 {
    margin: 0;
    font-size: 0.95rem;
    font-weight: 600;
  }
  .status-zeile {
    display: flex;
    align-items: center;
    gap: 10px;
    flex-wrap: wrap;
  }
  .check-titel {
    margin: 24px 0 10px;
  }

  select {
    padding: 8px 10px;
    font-size: 0.9rem;
    font-family: inherit;
    color: #172b4d;
    border: 2px solid #dfe1e6;
    border-radius: 8px;
    background: #fafbfc;
  }
  select:focus {
    outline: none;
    border-color: #4f6df5;
    background: #fff;
  }
  .frei {
    width: 100%;
    box-sizing: border-box;
    margin-top: 8px;
    padding: 8px 10px;
    font-size: 0.9rem;
    font-family: inherit;
    border: 2px solid #dfe1e6;
    border-radius: 8px;
    background: #fafbfc;
  }
  .frei:focus {
    outline: none;
    border-color: #4f6df5;
    background: #fff;
  }

  .leer {
    color: #5e6c84;
    font-size: 0.9rem;
    margin: 0 0 10px;
  }

  .kontakt label {
    display: block;
    font-size: 0.82rem;
    font-weight: 600;
    color: #5e6c84;
    margin: 10px 0 5px;
  }
  /* dezenter Hinweis: dieses Feld bleibt lokal */
  .nur-lokal {
    font-weight: 400;
    font-size: 0.76rem;
    color: #8590a2;
  }
  .kontakt input,
  .kontakt textarea {
    width: 100%;
    box-sizing: border-box;
    padding: 8px 10px;
    font-size: 0.9rem;
    font-family: inherit;
    border: 2px solid #dfe1e6;
    border-radius: 8px;
    background: #fafbfc;
  }
  .kontakt input:focus,
  .kontakt textarea:focus {
    outline: none;
    border-color: #4f6df5;
    background: #fff;
  }
  .kontakt textarea {
    resize: vertical;
    line-height: 1.5;
  }
  .kontakt .zwei {
    display: flex;
    gap: 12px;
  }
  .kontakt .zwei > div {
    flex: 1;
    min-width: 0;
  }

  .fristen {
    list-style: none;
    margin: 0 0 4px;
    padding: 0;
  }
  .fristen li {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
    padding: 6px 0;
    border-bottom: 1px solid #f1f2f4;
  }
  .frist-datum {
    font-size: 0.9rem;
  }
  /* kompaktes, editierbares Datum der offiziellen Frist */
  .off-fristen {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 8px;
  }
  .off-frist {
    display: inline-flex;
    align-items: center;
    gap: 2px;
  }
  .off-frist input[type="date"] {
    padding: 7px 9px;
    font-size: 0.9rem;
    font-family: inherit;
    border: 2px solid #dfe1e6;
    border-radius: 8px;
    background: #fafbfc;
  }
  .off-frist input[type="date"]:focus {
    outline: none;
    border-color: #4f6df5;
    background: #fff;
  }
  .off-add {
    background: none;
    border: none;
    color: #4f6df5;
    font-size: 0.85rem;
    font-family: inherit;
    cursor: pointer;
    padding: 4px 6px;
    border-radius: 6px;
  }
  .off-add:hover {
    background: #eef1ff;
  }
  .hinzufuegen input[type="date"] {
    flex: 1;
  }
  .eigene-frist input[type="date"] {
    flex: 0 0 auto;
  }
  .hinweis-klein {
    font-size: 0.8rem;
    color: #8590a2;
    margin: 4px 0 0;
  }
  .checkliste {
    list-style: none;
    margin: 0;
    padding: 0;
  }
  .checkliste li {
    display: flex;
    align-items: flex-start;
    gap: 10px;
    padding: 10px 0;
    border-bottom: 1px solid #f1f2f4;
  }
  .punkt-inhalt {
    flex: 1;
    min-width: 0;
  }
  .punkt-text {
    display: block;
    font-size: 0.92rem;
    margin-bottom: 6px;
  }
  .punkt-text.fertig {
    color: #5e6c84;
    text-decoration: line-through;
  }
  .punkt-status {
    display: flex;
    align-items: center;
    gap: 8px;
    flex-wrap: wrap;
  }
  .punkt-status .frei {
    width: auto;
    flex: 1;
    margin-top: 0;
  }

  /* Hochgeladenes Dokument je Checklisten-Punkt */
  .datei-zeile {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-top: 8px;
    flex-wrap: wrap;
  }
  .datei-name {
    font-size: 0.82rem;
    color: #216e4e;
    background: #dcfff1;
    padding: 3px 10px;
    border-radius: 99px;
    max-width: 100%;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .datei-knopf {
    background: none;
    border: 2px solid #dfe1e6;
    color: #44546f;
    font-size: 0.82rem;
    font-family: inherit;
    cursor: pointer;
    padding: 4px 12px;
    border-radius: 8px;
  }
  .datei-knopf:hover:not(:disabled) {
    border-color: #4f6df5;
    color: #172b4d;
  }
  .datei-knopf:disabled {
    opacity: 0.6;
    cursor: default;
  }
  .datei-knopf.hochladen {
    color: #3d5bf0;
    border-color: #c7d0f8;
  }
  .datei-entfernen {
    background: none;
    border: none;
    color: #8590a2;
    font-size: 0.9rem;
    cursor: pointer;
    padding: 2px 6px;
    border-radius: 6px;
  }
  .datei-entfernen:hover {
    background: #ffeceb;
    color: #ae2e24;
  }

  /* runder Farbpunkt je Status */
  .punkt {
    width: 11px;
    height: 11px;
    border-radius: 50%;
    flex-shrink: 0;
    margin-top: 4px;
  }
  .farbe-blau { background: #4f6df5; }
  .farbe-lila { background: #8270db; }
  .farbe-gruen { background: #22a06b; }
  .farbe-rot { background: #ca3521; }
  .farbe-gelb { background: #e2a400; }
  .farbe-grau { background: #b3bac5; }

  .entfernen {
    background: none;
    border: none;
    color: #8590a2;
    font-size: 0.95rem;
    cursor: pointer;
    padding: 4px 8px;
    border-radius: 6px;
    flex-shrink: 0;
  }
  .entfernen:hover {
    background: #ffeceb;
    color: #ae2e24;
  }

  .hinzufuegen {
    display: flex;
    gap: 8px;
    margin-top: 14px;
  }
  .hinzufuegen input {
    flex: 1;
    box-sizing: border-box;
    padding: 9px 12px;
    font-size: 0.9rem;
    font-family: inherit;
    border: 2px solid #dfe1e6;
    border-radius: 8px;
    background: #fafbfc;
  }
  .hinzufuegen input:focus {
    outline: none;
    border-color: #4f6df5;
    background: #fff;
  }
  .hinzufuegen button {
    padding: 9px 16px;
    font-size: 0.9rem;
    font-weight: 600;
    font-family: inherit;
    color: #fff;
    background: #4f6df5;
    border: none;
    border-radius: 8px;
    cursor: pointer;
  }
  .hinzufuegen button:hover:not(:disabled) {
    background: #3d5bf0;
  }
  .hinzufuegen button:disabled {
    background: #c1c7d0;
    cursor: default;
  }

  /* Einreichung nur über Online-Formular */
  .online-bereich {
    margin-top: 22px;
    padding: 14px 16px;
    border: 1px solid #94c0ff;
    background: #e6f0ff;
    border-radius: 8px;
  }
  .online-hinweis {
    margin: 0;
    font-size: 0.88rem;
    line-height: 1.5;
    color: #0c4a8f;
  }
  .online-knopf {
    margin-top: 12px;
    padding: 11px 16px;
    font-size: 0.95rem;
    font-weight: 600;
    font-family: inherit;
    color: #fff;
    background: #1f6feb;
    border: none;
    border-radius: 8px;
    cursor: pointer;
  }
  .online-knopf:hover {
    background: #195fc9;
  }
  .online-fehlt {
    margin: 10px 0 0;
    font-size: 0.82rem;
    color: #5e6c84;
  }

  /* Antrags-PDF erstellen */
  .pdf-bereich {
    margin-top: 22px;
    padding-top: 18px;
    border-top: 1px solid #dfe1e6;
  }
  .pdf-knopf {
    width: 100%;
    padding: 12px;
    font-size: 0.95rem;
    font-weight: 600;
    font-family: inherit;
    color: #fff;
    background: #216e4e;
    border: none;
    border-radius: 8px;
    cursor: pointer;
  }
  .pdf-knopf:hover:not(:disabled) {
    background: #1a5a40;
  }
  .pdf-knopf:disabled {
    background: #c1c7d0;
    cursor: not-allowed;
  }
  .pdf-ok {
    margin: 10px 0 0;
    font-size: 0.82rem;
    color: #216e4e;
    word-break: break-all;
  }

  .pdf-schleier {
    position: fixed;
    inset: 0;
    background: rgba(9, 30, 66, 0.45);
    display: grid;
    place-items: center;
    padding: 24px;
    z-index: 30;
  }
  .pdf-dialog {
    background: #fff;
    border-radius: 12px;
    padding: 28px;
    max-width: 440px;
    width: 100%;
    box-shadow: 0 12px 40px rgba(9, 30, 66, 0.3);
  }
  .pdf-dialog h3 {
    margin: 0 0 10px;
    font-size: 1.1rem;
  }
  .pdf-dialog p {
    margin: 0 0 18px;
    font-size: 0.92rem;
    line-height: 1.55;
    color: #44546f;
  }
  .pdf-aktionen {
    display: flex;
    align-items: center;
    gap: 10px;
    flex-wrap: wrap;
  }
  .pdf-aktionen .primaer {
    padding: 10px 18px;
    font-size: 0.93rem;
    font-weight: 600;
    font-family: inherit;
    color: #fff;
    background: #216e4e;
    border: none;
    border-radius: 8px;
    cursor: pointer;
  }
  .pdf-aktionen .primaer:hover:not(:disabled) {
    background: #1a5a40;
  }
  .pdf-aktionen .primaer:disabled {
    background: #c1c7d0;
    cursor: default;
  }
  .pdf-aktionen .zweit {
    padding: 10px 16px;
    font-size: 0.93rem;
    font-weight: 600;
    font-family: inherit;
    color: #172b4d;
    background: #fff;
    border: 2px solid #dfe1e6;
    border-radius: 8px;
    cursor: pointer;
  }
  .pdf-aktionen .zweit:hover:not(:disabled) {
    border-color: #4f6df5;
  }
  .pdf-aktionen .zweit:disabled {
    color: #b3bac5;
    background: #f4f5f7;
    border-color: #ebecf0;
    cursor: not-allowed;
  }
  .pdf-aktionen .leise {
    background: none;
    border: none;
    color: #5e6c84;
    font-size: 0.9rem;
    font-family: inherit;
    cursor: pointer;
    padding: 8px;
  }
  .pdf-aktionen .leise:hover:not(:disabled) {
    color: #172b4d;
    text-decoration: underline;
  }
  .pdf-mail-hinweis {
    margin: 14px 0 0;
    font-size: 0.78rem;
    line-height: 1.5;
    color: #8590a2;
  }
</style>
