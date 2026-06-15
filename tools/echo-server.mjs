// ============================================================
// Mitschnitt-Server fuer den Trockenlauf (Etappe 5).
//
// ZWECK: Unabhaengig von der App und ohne NAS nachpruefen, WAS die
// Anwendung beim Synchronisieren ins Netz geben wuerde. Der Server
// nimmt jede Anfrage an, gibt Methode, Pfad und den vollstaendigen
// Body (huebsch formatiert) im Terminal aus und antwortet so, dass die
// App zufrieden ist. Es wird NICHTS gespeichert und NICHTS weitergeleitet.
//
// START:  node tools/echo-server.mjs
// In der App: Team-Sync -> "Pruefen & Protokoll" ->
//   "An lokalen Mitschnitt senden" (Adresse http://127.0.0.1:8099).
//
// Pruefe in der Ausgabe: Es duerfen NUR unkritische Board-Felder
// erscheinen (id, name, foerderungId, status, Fristen, oeffentlicher
// Kontakt). KEINE Stammdaten, IBAN, Steuer, Budget/KFP, Formular-Texte
// oder Projektbeschriebe.
// ============================================================

import http from "node:http";

const PORT = Number(process.env.PORT ?? 8099);

const server = http.createServer((req, res) => {
  let roh = "";
  req.on("data", (stueck) => (roh += stueck));
  req.on("end", () => {
    const zeit = new Date().toLocaleTimeString();
    console.log("\n=== " + zeit + "  " + req.method + " " + req.url + " ===");
    if (roh.length === 0) {
      console.log("(kein Body)");
    } else {
      try {
        console.log(JSON.stringify(JSON.parse(roh), null, 2));
      } catch {
        console.log(roh);
      }
    }

    // Antworten so, wie es der echte Server tun wuerde, damit die App
    // den Trockenlauf sauber abschliesst.
    res.setHeader("content-type", "application/json");
    if (req.method === "GET" && req.url.startsWith("/api/board")) {
      res.end("[]");
    } else if (req.method === "GET" && req.url === "/api/health") {
      res.end('"ok"');
    } else {
      res.end(JSON.stringify({ version: 1, konflikt: false, aktuell: null }));
    }
  });
});

server.listen(PORT, "127.0.0.1", () => {
  console.log("Mitschnitt-Server laeuft auf http://127.0.0.1:" + PORT);
  console.log("Jede empfangene Anfrage wird hier vollstaendig angezeigt.");
  console.log("Beenden mit Strg+C.\n");
});
