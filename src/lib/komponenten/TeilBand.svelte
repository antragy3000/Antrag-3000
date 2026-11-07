<script>
  // Hinweisband über einem Bereich, wenn im Projektmenü ein Teil gewählt ist.
  // Sagt, worauf man gerade blickt, und führt zurück zur ganzen Ausgabe.
  // Die Wahl ist nur ein Blickwinkel: Gespeichert wird immer in der Ausgabe.
  let { teil, ausgabeName, ganzeAusgabe } = $props();
</script>

<div class="teil-band" title="Nur ein Blickwinkel: Gespeichert wird immer in der ganzen Ausgabe.">
  <span class="zeichen" aria-hidden="true">◎</span>
  <span class="text">
    <strong>Blick auf den Teil „{teil.name}“</strong>
    <span class="leise">· aus {ausgabeName}</span>
  </span>
  <button type="button" class="zurueck" onclick={ganzeAusgabe}>Ganze Ausgabe zeigen</button>
</div>

<style>
  .teil-band {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: 8px 12px;
    margin-bottom: 20px;
    padding: 10px 14px;
    font-size: 0.9rem;
    color: var(--text);
    background: var(--akzent-bg);
    border: 1px solid var(--akzent-rand);
    border-radius: var(--radius);
  }
  .zeichen {
    color: var(--akzent-text);
  }
  .text {
    flex: 1 1 260px;
  }
  .leise {
    color: var(--text-muted);
  }
  .zurueck {
    width: auto;
    margin: 0;
    padding: 5px 12px;
    font-size: 0.85rem;
    white-space: nowrap;
    color: var(--akzent-text);
    background: var(--weiss);
    border: 1px solid var(--akzent-rand);
  }
  .zurueck:hover:not(:disabled) {
    background: var(--weiss);
    border-color: var(--akzent);
  }
</style>
