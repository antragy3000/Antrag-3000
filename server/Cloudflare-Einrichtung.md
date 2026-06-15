# NAS über Cloudflare anbinden (Tunnel + Access-mTLS)

Dieser Weg braucht **keine offenen Router-Ports** und versteckt deine
Heim-IP. Cloudflare prüft das Geräte-Zertifikat (deine Team-CA) und lässt
nur gültige Geräte durch. **Wichtig:** Cloudflare sieht die übertragenen
Daten – das ist hier in Ordnung, weil über diesen Kanal **nur unkritische**
Daten laufen (Förder-Status, Katalog, Meldungen). Sensible Daten verlassen
dein Gerät nie.

Voraussetzung: eine **Domain in deinem Cloudflare-Konto** (z. B. eine
günstig registrierte Domain), und die kostenlose **Cloudflare Zero Trust**
(Access) ist aktiviert.

## 1. Tunnel anlegen (kein offener Port)
1. Cloudflare-Dashboard → **Zero Trust** → **Networks → Tunnels** →
   **Create a tunnel** (Typ „Cloudflared").
2. Tunnel benennen (z. B. `antrag3000`). Cloudflare zeigt einen
   **Tunnel-Token** – kopiere ihn in die Datei `server/.env` als
   `TUNNEL_TOKEN=…`.
3. Im Tunnel eine **Public Hostname**-Regel anlegen:
   - Subdomain/Domain: z. B. `team.deine-domain.tld`
   - Service: **HTTP** → `api:8080`
   (Das ist der interne Dienst aus der Docker-Compose-Datei.)

## 2. Access-Anwendung mit mTLS
1. **Zero Trust → Access → Applications → Add an application →
   Self-hosted**. Domain: derselbe Hostname `team.deine-domain.tld`.
2. **Deine Team-CA hochladen:** Zero Trust → **Settings → Authentication →
   Mutual TLS** → **Add mTLS certificate** → den Inhalt von
   `ca/team-ca.crt` einfügen (das **öffentliche** CA-Zertifikat, das die
   App unter „Team-CA-Zertifikat speichern" erzeugt).
3. In der Access-Anwendung eine **Policy** anlegen:
   - Action: **Service Auth** (kein interaktives Login)
   - Include / Require: **Valid Certificate** (das mTLS-Zertifikat von oben)
4. **Client-Zertifikat an den Ursprung weiterreichen:** In den
   mTLS-Einstellungen die Weitergabe des Zertifikats aktivieren, damit der
   Header **`Cf-Client-Cert-Der-Base64`** beim Dienst ankommt. (Der
   Antrag-3000-Server liest genau diesen Header.)

## 3. Dienst auf der NAS starten
Im Ordner `server/` (vorher `.env` mit `TUNNEL_TOKEN` füllen):

```
docker compose -f docker-compose.cloudflare.yml up -d --build
```

Der Förder-Katalog, den der Server verteilt, liegt unter
`server/katalog/foerderungen.json` (lege ihn dort ab; ein
Upload-Werkzeug folgt in Etappe 4).

## 4. In der App
Adresse im Zugangs-Paket = `https://team.deine-domain.tld`. Danach:
- **Team-Sync**: „Verbindung testen" / „Synchronisieren starten".
- **Förder-Datenbank**: „🔄 Vom Team-Server holen" lädt den Katalog.

> Hinweis: Der direkte Weg über DDNS + Port 443 (Caddy) bleibt als
> Variante B in `docker-compose.yml` erhalten – falls du Cloudflare doch
> nicht nutzen willst.
