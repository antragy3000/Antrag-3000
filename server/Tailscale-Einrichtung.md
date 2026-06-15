# NAS über Tailscale anbinden (kostenlos, keine offenen Ports)

Deine NAS und deine Geräte sind in einem privaten, verschlüsselten
Tailscale-Netz. Niemand außer deinen Geräten sieht etwas; es muss **kein
Router-Port** geöffnet werden. Caddy macht weiterhin **HTTPS + mTLS mit
deiner Team-CA** – nur eben über das Tailnet.

## 1. Tailscale verbinden
- **NAS:** Synology Paket-Zentrum → **Tailscale** installieren → mit deinem
  Tailscale-Konto anmelden.
- **Jedes Gerät:** Tailscale-App installieren ([tailscale.com/download](https://tailscale.com/download))
  und mit demselben Konto anmelden.
- Im Tailscale-Admin (login.tailscale.com) erscheinen alle Geräte. Notiere
  den **Namen** (MagicDNS, z. B. `nas.dein-tailnet.ts.net`) **oder** die
  **100.x.y.z-Adresse** der NAS.

## 2. Zertifikate in der App erzeugen (Reiter „Stammdaten & Team")
Unter **„▸ Team verwalten"**:
- **1 · Für die NAS:** „Team-CA-Zertifikat speichern" → `team-ca.crt`.
- **1b · NAS-Server-Zertifikat:** die NAS-Adresse aus Schritt 1 eintragen →
  „Server-Zertifikat speichern" → erzeugt **`server.crt`** + **`server.key`**.

## 3. Dateien auf die NAS legen
In den Ordner `server/` (auf der NAS):
- `ca/team-ca.crt`
- `server.crt`
- `server.key`
- `katalog/foerderungen.json` (der zu verteilende Katalog)

## 4. Dienst starten
Im `server/`-Ordner auf der NAS:
```
docker compose -f docker-compose.tailscale.yml up -d --build
```

## 5. In der App
- Geräte-Paket(e) mit der **Tailscale-Adresse** der NAS erzeugen
  (Schritt „2 · Gerät hinzufügen" bzw. „3 · Dieses Gerät direkt einrichten").
- **Team-Sync → Verbindung testen** → sollte ✓ melden.
- **Förder-Datenbank → „Vom Team-Server holen"** lädt den Katalog.

> Hinweis: Die App vertraut der Verbindung, weil das Server-Zertifikat von
> **deiner** Team-CA signiert ist (im Geräte-Paket steckt das CA-Zertifikat
> mit). Kein Let's Encrypt, kein Cloudflare nötig.
