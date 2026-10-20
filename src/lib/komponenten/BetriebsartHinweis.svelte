<script>
  // Hinweis vor dem Umstellen der Betriebsart (Einzelplatz ↔ Team): erklärt,
  // was die gewählte Betriebsart bedeutet. Mit „nicht mehr anzeigen" stellt die
  // App künftig ohne Rückfrage um (Merker im Tresor, siehe +page.svelte).
  let { ziel, teamEingerichtet = false, abbrechen, bestaetigen } = $props();

  let nichtMehrZeigen = $state(false);
</script>

<div class="schleier" onclick={abbrechen} role="presentation">
  <div class="dialog" onclick={(e) => e.stopPropagation()} role="presentation">
    {#if ziel === "einzel"}
      <h2>Auf „Einzelplatz" umstellen?</h2>
      <p>Du arbeitest allein mit Antrag 3000.</p>
      <ul>
        <li>Die <strong>Förder-Datenbank</strong> kommt vom Antrag-3000-Server – ganz
          ohne Zugangs-Paket. Die App holt sie bei jedem Start still auf den neuesten Stand.</li>
        <li><strong>Nichts wird geteilt:</strong> Status, Fristen, Merkliste und eigene
          Förderer bleiben nur auf diesem Gerät.</li>
        <li>Eine laufende Team-Synchronisation wird angehalten.
          {#if teamEingerichtet}Dein Team-Zugang bleibt gespeichert.{/if}</li>
      </ul>
    {:else}
      <h2>Auf „Team" umstellen?</h2>
      <p>Ihr arbeitet zu mehreren an Anträgen.</p>
      <ul>
        <li>Mit eurem Team geteilt werden <strong>Projektnamen, gemerkte Förderungen
          mit Status, Fristen, Förderer-Kontakten und Dokument-Checklisten</strong>.</li>
        <li><strong>Nie geteilt</strong> werden Stammdaten, Formular-Texte, Kostenplan,
          Fragebogen, Notizen und die Dateien selbst – sie bleiben verschlüsselt auf diesem Gerät.</li>
        {#if teamEingerichtet}
          <li>Den Abgleich startest du im Bereich „Team-Synchronisation".</li>
        {:else}
          <li class="warn">Du hast noch <strong>kein Team eingerichtet</strong>. Erstelle ein
            Team oder folge einer Einladung. Bis dahin holt die App <strong>keine
            Aktualisierungen der Förder-Datenbank</strong> – arbeitest du allein,
            bleib besser bei „Einzelplatz".</li>
        {/if}
      </ul>
    {/if}
    <p class="klein">Du kannst die Betriebsart jederzeit wieder umstellen. An deinen
      Projekten und Anträgen ändert sich dadurch nichts.</p>

    <label class="nichtmehr">
      <input type="checkbox" bind:checked={nichtMehrZeigen} />
      Diesen Hinweis nicht mehr anzeigen
    </label>

    <div class="dialog-knoepfe">
      <button class="leise" onclick={abbrechen}>Abbrechen</button>
      <button class="primaer" onclick={() => bestaetigen(nichtMehrZeigen)}>
        Auf „{ziel === "einzel" ? "Einzelplatz" : "Team"}" umstellen
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
    padding: 32px;
    max-width: 500px;
    width: 100%;
    max-height: 85vh;
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
    margin: 0 0 12px;
    padding-left: 20px;
  }
  li {
    margin-bottom: 6px;
  }
  .warn {
    background: var(--warnung-bg);
    color: var(--warnung-text2);
    border-radius: 8px;
    padding: 8px 12px;
    list-style: none;
    margin-left: -20px;
  }
  .klein {
    color: var(--text-muted);
    font-size: 0.84rem;
  }
  .nichtmehr {
    display: flex;
    align-items: center;
    gap: 8px;
    margin: 18px 0 8px;
    font-size: 0.9rem;
    color: var(--text-2);
    cursor: pointer;
  }
  .nichtmehr input {
    width: auto;
  }
  .dialog-knoepfe {
    display: flex;
    justify-content: flex-end;
    gap: 10px;
    margin-top: 16px;
  }
  .dialog-knoepfe button {
    width: auto;
  }
  button.primaer {
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
  button.primaer:hover {
    background: var(--akzent-d);
  }
  button.leise {
    background: none;
    border: none;
    color: var(--text-muted);
    font-size: 0.875rem;
    cursor: pointer;
    padding: 6px;
    font-family: inherit;
  }
  button.leise:hover {
    color: var(--text);
    text-decoration: underline;
  }
</style>
