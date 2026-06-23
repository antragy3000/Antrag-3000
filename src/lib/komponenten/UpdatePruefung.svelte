<script>
  // Phase 3 / Etappe 5: signiertes App-Selbstupdate (Bedienoberfläche).
  //
  // Prüft über das Tauri-Updater-Plugin, ob auf dem Team-Server eine neuere,
  // GÜLTIG SIGNIERTE App-Version liegt. Gefunden wird nur, was mit dem
  // privaten Schlüssel des Admins signiert wurde (öffentlicher Schlüssel ist
  // fest in der App). Heruntergeladen/installiert wird ausschließlich nach
  // ausdrücklicher Bestätigung durch die Nutzer:in – nie automatisch.
  import { check } from "@tauri-apps/plugin-updater";
  import { relaunch } from "@tauri-apps/plugin-process";
  import { getVersion } from "@tauri-apps/api/app";

  let { schliessen } = $props();

  // 'pruefen' | 'aktuell' | 'verfuegbar' | 'laedt' | 'fertig' | 'fehler'
  let zustand = $state("pruefen");
  let fehler = $state("");
  let info = $state(null); // { version, body, date }
  let laufendeVersion = $state("");
  let geladen = $state(0);
  let gesamt = $state(0);
  let aktualisierung = null; // das Update-Objekt (nicht reaktiv nötig)

  let prozent = $derived(gesamt > 0 ? Math.min(100, Math.round((geladen / gesamt) * 100)) : 0);
  const mb = (b) => (b / 1048576).toFixed(1);

  function datumText(iso) {
    if (!iso) return "";
    // Das Updater-Datum kommt z. B. als "2026-06-23 10:00:00.000 +00:00:00".
    const d = new Date(String(iso).replace(" ", "T").slice(0, 19));
    return isNaN(d) ? "" : d.toLocaleDateString("de-CH");
  }

  async function pruefen() {
    zustand = "pruefen";
    fehler = "";
    try {
      laufendeVersion = await getVersion();
    } catch { /* unkritisch */ }
    try {
      const u = await check();
      if (!u) { zustand = "aktuell"; return; }
      aktualisierung = u;
      info = { version: u.version, body: u.body ?? "", date: u.date ?? "" };
      zustand = "verfuegbar";
    } catch (e) {
      fehler = String(e);
      zustand = "fehler";
    }
  }

  async function jetztAktualisieren() {
    if (!aktualisierung) return;
    zustand = "laedt";
    geladen = 0;
    gesamt = 0;
    try {
      await aktualisierung.downloadAndInstall((ev) => {
        if (ev.event === "Started") gesamt = ev.data?.contentLength ?? 0;
        else if (ev.event === "Progress") geladen += ev.data?.chunkLength ?? 0;
        else if (ev.event === "Finished") zustand = "fertig";
      });
      zustand = "fertig";
      // Kurz den Hinweis zeigen, dann mit der neuen Version neu starten.
      await relaunch();
    } catch (e) {
      fehler = String(e);
      zustand = "fehler";
    }
  }

  // Beim Öffnen sofort prüfen.
  $effect(() => {
    pruefen();
  });
</script>

<div class="schleier" onclick={zustand === "laedt" ? null : schliessen} role="presentation">
  <div class="dialog" onclick={(e) => e.stopPropagation()} role="presentation">
    <h2>App-Update</h2>

    {#if laufendeVersion}
      <p class="dezent">Installierte Version: <strong>{laufendeVersion}</strong></p>
    {/if}

    {#if zustand === "pruefen"}
      <p class="status">⏳ Suche nach Updates …</p>
    {:else if zustand === "aktuell"}
      <div class="box ok-box"><strong>✓ Alles aktuell.</strong> Du nutzt bereits die neueste Version.</div>
    {:else if zustand === "verfuegbar"}
      <div class="box info-box">
        <strong>Neue Version verfügbar: {info.version}</strong>
        {#if datumText(info.date)}<div class="dezent klein">vom {datumText(info.date)}</div>{/if}
        {#if info.body}<div class="notizen">{info.body}</div>{/if}
      </div>
      <p class="dezent klein">
        Das Update ist digital signiert und wird vor der Installation auf Echtheit
        geprüft. Die App startet danach neu.
      </p>
      <div class="knoepfe">
        <button class="primaer" onclick={jetztAktualisieren}>Jetzt aktualisieren &amp; neu starten</button>
        <button class="leise" onclick={schliessen}>Später</button>
      </div>
    {:else if zustand === "laedt"}
      <p class="status">⬇ Update wird geladen und installiert …</p>
      <div class="balken"><div class="balken-fuell" style="width:{prozent}%"></div></div>
      <p class="dezent klein">
        {#if gesamt > 0}{mb(geladen)} / {mb(gesamt)} MB ({prozent} %){:else}Wird vorbereitet …{/if}
      </p>
      <p class="dezent klein">Bitte das Fenster nicht schließen.</p>
    {:else if zustand === "fertig"}
      <div class="box ok-box"><strong>✓ Installiert.</strong> Die App startet jetzt neu …</div>
    {:else if zustand === "fehler"}
      <div class="box fehler-box">
        <strong>⚠ Update nicht möglich.</strong>
        <div class="dezent klein">{fehler}</div>
      </div>
      <p class="dezent klein">
        Häufigste Ursache: keine Verbindung zum Team-Server. Prüfe die Verbindung
        (Status-Punkt) und versuche es erneut.
      </p>
      <div class="knoepfe">
        <button class="zweit" onclick={pruefen}>Erneut suchen</button>
        <button class="leise" onclick={schliessen}>Schließen</button>
      </div>
    {/if}

    {#if zustand !== "verfuegbar" && zustand !== "laedt" && zustand !== "fehler"}
      <div class="fuss"><button class="leise" onclick={schliessen}>Schließen</button></div>
    {/if}
  </div>
</div>

<style>
  .schleier {
    position: fixed; inset: 0; background: rgba(9, 30, 66, 0.45);
    display: grid; place-items: center; padding: 24px; z-index: 50;
  }
  .dialog {
    background: #fff; border-radius: 12px; padding: 32px;
    max-width: 460px; width: 100%; box-shadow: 0 12px 40px rgba(9, 30, 66, 0.3);
  }
  h2 { margin: 0 0 12px; font-size: 1.2rem; }
  p { margin: 0 0 10px; }
  .dezent { color: #5e6c84; font-size: 0.9rem; line-height: 1.5; }
  .klein { font-size: 0.82rem; }
  .status { font-size: 1rem; font-weight: 600; margin: 14px 0; }
  .box { border-radius: 8px; padding: 12px 14px; margin: 12px 0; font-size: 0.95rem; }
  .ok-box { background: #e3fcef; color: #143d2b; }
  .info-box { background: #deebff; color: #0747a6; }
  .fehler-box { background: #ffebe6; color: #8a1c0a; }
  .notizen { margin-top: 8px; white-space: pre-wrap; font-size: 0.9rem; }
  .balken { height: 10px; background: #ebecf0; border-radius: 6px; overflow: hidden; margin: 6px 0; }
  .balken-fuell { height: 100%; background: #4f6df5; transition: width 0.2s; }
  .knoepfe { display: flex; gap: 10px; align-items: center; flex-wrap: wrap; margin-top: 12px; }
  .primaer {
    padding: 9px 16px; font-size: 0.92rem; font-weight: 600; font-family: inherit;
    color: #fff; background: #4f6df5; border: none; border-radius: 8px; cursor: pointer;
  }
  .primaer:hover { background: #3d5bf0; }
  .zweit {
    padding: 8px 14px; font-size: 0.9rem; font-weight: 600; font-family: inherit;
    color: #172b4d; background: #fff; border: 2px solid #dfe1e6; border-radius: 8px; cursor: pointer;
  }
  .zweit:hover { border-color: #4f6df5; }
  .leise {
    background: none; border: none; color: #5e6c84; font-size: 0.9rem;
    font-family: inherit; cursor: pointer; padding: 8px;
  }
  .leise:hover { color: #172b4d; text-decoration: underline; }
  .fuss { display: flex; justify-content: flex-end; margin-top: 14px; }
</style>
