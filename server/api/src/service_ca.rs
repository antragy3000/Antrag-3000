// ============================================================
// CA-Baustein (gehostetes Modell)
//
// EIN generischer CA-Typ für ZWEI Vertrauensanker des Dienstes:
//   - Service-CA  → signiert Team-Geräte-Ausweise (Enrollment).
//   - Förderer-CA → signiert Förderer-Ausweise (Förderer verbinden sich online).
//
// KOPIERSICHER: Das Gegenüber (Gerät bzw. Förderer) erzeugt sein Schlüsselpaar
// LOKAL und schickt nur seinen ÖFFENTLICHEN Schlüssel (SubjectPublicKeyInfo-PEM).
// Der private Schlüssel verlässt das Gerät nie – der Server kann einen Ausweis
// also gar nicht duplizieren. `signiere()` ist genau dieser Baustein.
//
// SICHERHEIT: Die CA-Schlüssel (…-ca.key) sind sensible Ziele. Sie liegen nur
// auf dem Server mit engen Dateirechten (0600) und werden NIE über eine
// Netzwerk-/Webview-Verbindung herausgegeben. Nur die öffentlichen …-ca.crt
// werden verteilt (Caddy-Vertrauen, später Zugangs-/Einladungs-Pakete).
// Härtung (getrennte Signier-Komponente / kurzlebige Ausweise / HSM) ist als
// späterer Schritt vorgesehen (Roadmap 10).
// ============================================================

use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

/// Eine Zertifizierungsstelle des Dienstes: öffentliches Zertifikat
/// (verteilbar) + privater Schlüssel (bleibt am Server). Der Schlüssel ist
/// bewusst NICHT `pub`, damit er nicht versehentlich serialisiert/ausgeliefert
/// wird. Wird für die Service-CA UND die Förderer-CA genutzt.
pub struct Ca {
    /// PEM des CA-Zertifikats – öffentlich (Caddy-Trust, Einladungs-Pakete).
    pub cert_pem: String,
    /// PEM des CA-Schlüssels – GEHEIM, verlässt den Server nie.
    key_pem: String,
}

impl Ca {
    /// Lädt die CA aus `dir` (Dateien `{stamm}.crt` + `{stamm}.key`), oder
    /// erzeugt sie beim ersten Start mit dem Namen `ca_cn` und legt sie dort
    /// ab. Idempotent: existiert sie schon, wird sie nur gelesen (der
    /// bestehende Schlüssel bleibt – sonst würden alle bisher ausgestellten
    /// Ausweise ungültig).
    /// Für einstufige CAs (Förderer-CA): signiert Ausweise direkt, keine Unterstufe.
    pub fn laden_oder_erzeugen(dir: &Path, stamm: &str, ca_cn: &str) -> Result<Self, String> {
        Self::laden_oder_erzeugen_mit(dir, stamm, ca_cn, 0)
    }

    /// Wie `laden_oder_erzeugen`, aber eine NEU erzeugte CA erlaubt `unterstufen`
    /// Zwischen-CAs unter sich (pathLenConstraint). Eine bestehende CA wird nur
    /// gelesen – ihr Wert bleibt, wie er beim Erzeugen war.
    fn laden_oder_erzeugen_mit(
        dir: &Path,
        stamm: &str,
        ca_cn: &str,
        unterstufen: u8,
    ) -> Result<Self, String> {
        let cert_pfad = dir.join(format!("{stamm}.crt"));
        let key_pfad = dir.join(format!("{stamm}.key"));

        if let (Ok(cert_pem), Ok(key_pem)) = (
            std::fs::read_to_string(&cert_pfad),
            std::fs::read_to_string(&key_pfad),
        ) {
            if !cert_pem.trim().is_empty() && !key_pem.trim().is_empty() {
                return Ok(Ca { cert_pem, key_pem });
            }
        }

        let ca = Self::erzeugen(ca_cn, unterstufen)?;
        std::fs::create_dir_all(dir)
            .map_err(|e| format!("CA-Ordner {dir:?} nicht anlegbar: {e}"))?;
        schreibe_geschuetzt(&key_pfad, &ca.key_pem, 0o600)
            .map_err(|e| format!("CA-Schlüssel nicht speicherbar: {e}"))?;
        schreibe_geschuetzt(&cert_pfad, &ca.cert_pem, 0o644)
            .map_err(|e| format!("CA-Zertifikat nicht speicherbar: {e}"))?;
        Ok(ca)
    }

    /// Erzeugt frisch eine selbstsignierte CA (nur im Speicher) mit CN `ca_cn`.
    /// Gleiche rcgen-0.13-Bausteine wie die Förderer-CA in der Admin-App.
    fn erzeugen(ca_cn: &str, unterstufen: u8) -> Result<Self, String> {
        let key = rcgen::KeyPair::generate().map_err(|e| format!("Schlüssel nicht erzeugbar: {e}"))?;
        let mut p = rcgen::CertificateParams::new(Vec::<String>::new())
            .map_err(|e| format!("CA-Parameter fehlerhaft: {e}"))?;
        p.distinguished_name
            .push(rcgen::DnType::OrganizationName, "Antrag 3000");
        p.distinguished_name
            .push(rcgen::DnType::CommonName, ca_cn);
        // Constrained(n): höchstens n Zwischen-CAs darunter. 0 = signiert Ausweise
        // nur direkt (Förderer-CA); die Wurzel der StufenCa braucht 1, sonst
        // lehnt jede Prüfung nach RFC 5280 (Go/Caddy, openssl) die Kette
        // Blatt → Zwischen → Wurzel ab.
        p.is_ca = rcgen::IsCa::Ca(rcgen::BasicConstraints::Constrained(unterstufen));
        p.key_usages = vec![
            rcgen::KeyUsagePurpose::KeyCertSign,
            rcgen::KeyUsagePurpose::CrlSign,
        ];
        p.not_before = rcgen::date_time_ymd(2024, 1, 1);
        p.not_after = rcgen::date_time_ymd(2035, 1, 1);
        let cert = p.self_signed(&key).map_err(|e| format!("CA nicht signierbar: {e}"))?;
        Ok(Ca {
            cert_pem: cert.pem(),
            key_pem: key.serialize_pem(),
        })
    }

    /// SHA-256-Fingerabdruck (Hex) des CA-Zertifikats – nur zum Loggen/Prüfen.
    pub fn fingerprint(&self) -> String {
        fingerprint_von_pem(&self.cert_pem).unwrap_or_else(|| {
            let mut h = Sha256::new();
            h.update(self.cert_pem.as_bytes());
            hex::encode(h.finalize())
        })
    }

    /// Signiert einen Ausweis aus dem mitgeschickten ÖFFENTLICHEN Schlüssel des
    /// Gegenübers (SubjectPublicKeyInfo-PEM). Der private Schlüssel wird NICHT
    /// gebraucht und ist dem Server nicht bekannt – das macht den Ausweis
    /// kopiersicher. `cn` = Anzeigename (Gerät/Förderer), `org` = Organisation
    /// im Zertifikat (z. B. "Antrag 3000 Team-Gerät" oder "Antrag 3000 Förderer").
    pub fn signiere(
        &self,
        pubkey_pem: &str,
        cn: &str,
        org: &str,
        gueltig_tage: i64,
    ) -> Result<String, String> {
        let ca_key = rcgen::KeyPair::from_pem(&self.key_pem)
            .map_err(|e| format!("CA-Schlüssel unlesbar: {e}"))?;
        let ca_cert = rcgen::CertificateParams::from_ca_cert_pem(&self.cert_pem)
            .map_err(|e| format!("CA-Zertifikat unlesbar: {e}"))?
            .self_signed(&ca_key)
            .map_err(|e| format!("CA nicht rekonstruierbar: {e}"))?;

        let pubkey = rcgen::SubjectPublicKeyInfo::from_pem(pubkey_pem)
            .map_err(|e| format!("Öffentlicher Schlüssel unlesbar: {e}"))?;

        let cn = cn.trim();
        if cn.is_empty() {
            return Err("Name (CN) fehlt.".into());
        }
        let mut p = rcgen::CertificateParams::new(Vec::<String>::new())
            .map_err(|e| format!("Ausweis-Parameter fehlerhaft: {e}"))?;
        p.distinguished_name
            .push(rcgen::DnType::OrganizationName, org);
        p.distinguished_name
            .push(rcgen::DnType::CommonName, cn);
        p.is_ca = rcgen::IsCa::NoCa;
        p.key_usages = vec![rcgen::KeyUsagePurpose::DigitalSignature];
        // Nur zum Anmelden (mTLS-Client) – nicht als Server-Zertifikat nutzbar.
        p.extended_key_usages = vec![rcgen::ExtendedKeyUsagePurpose::ClientAuth];
        // Kurzlebig (Roadmap 10): ein geklauter Ausweis verfällt von selbst; die
        // App erneuert rechtzeitig über die bestehende mTLS-Verbindung. Kleine
        // Rückdatierung als Uhr-Toleranz zwischen Server und Gerät.
        let jetzt = time::OffsetDateTime::now_utc();
        p.not_before = jetzt - time::Duration::hours(1);
        p.not_after = jetzt + time::Duration::days(gueltig_tage.max(1));

        let cert = p
            .signed_by(&pubkey, &ca_cert, &ca_key)
            .map_err(|e| format!("Ausweis nicht signierbar: {e}"))?;
        Ok(cert.pem())
    }

    /// Erzeugt eine von DIESER (Wurzel-)CA signierte **Zwischen-CA** (Roadmap 10):
    /// eigenes Schlüsselpaar + ein CA-Zertifikat, das die Wurzel abstempelt. Braucht
    /// den Wurzel-Schlüssel – nur bei diesem EINMALIGEN Schritt; danach darf der
    /// Wurzel-Schlüssel offline. Gibt (zwischen_cert_pem, zwischen_key_pem).
    pub fn erzeuge_zwischen_ca(&self, cn: &str) -> Result<(String, String), String> {
        let wurzel_key = rcgen::KeyPair::from_pem(&self.key_pem)
            .map_err(|e| format!("Wurzel-Schlüssel unlesbar: {e}"))?;
        let wurzel_cert = rcgen::CertificateParams::from_ca_cert_pem(&self.cert_pem)
            .map_err(|e| format!("Wurzel-Zertifikat unlesbar: {e}"))?
            .self_signed(&wurzel_key)
            .map_err(|e| format!("Wurzel nicht rekonstruierbar: {e}"))?;
        let key = rcgen::KeyPair::generate().map_err(|e| format!("Schlüssel nicht erzeugbar: {e}"))?;
        let mut p = rcgen::CertificateParams::new(Vec::<String>::new())
            .map_err(|e| format!("Zwischen-CA-Parameter fehlerhaft: {e}"))?;
        p.distinguished_name
            .push(rcgen::DnType::OrganizationName, "Antrag 3000");
        p.distinguished_name.push(rcgen::DnType::CommonName, cn);
        // Darf Blatt-Ausweise signieren, aber keine weiteren (Unter-)CAs.
        p.is_ca = rcgen::IsCa::Ca(rcgen::BasicConstraints::Constrained(0));
        p.key_usages = vec![
            rcgen::KeyUsagePurpose::KeyCertSign,
            rcgen::KeyUsagePurpose::CrlSign,
        ];
        let jetzt = time::OffsetDateTime::now_utc();
        p.not_before = jetzt - time::Duration::hours(1);
        p.not_after = jetzt + time::Duration::days(1095); // ~3 Jahre
        let cert = p
            .signed_by(&key, &wurzel_cert, &wurzel_key)
            .map_err(|e| format!("Zwischen-CA nicht signierbar: {e}"))?;
        Ok((cert.pem(), key.serialize_pem()))
    }

    /// Grace-Wiederanmeldung für einstufige CAs (Förderer-CA): prüft, dass das
    /// erste Zertifikat in `cert_pem` von DIESER CA signiert ist und höchstens
    /// `grace_tage` abgelaufen; gibt den öffentlichen Schlüssel (SEC1) für die
    /// Besitznachweis-Prüfung zurück.
    pub fn grace_pubkey(&self, cert_pem: &str, grace_tage: i64) -> Result<Vec<u8>, String> {
        use x509_parser::pem::parse_x509_pem;
        let (_, ca_pem) =
            parse_x509_pem(self.cert_pem.as_bytes()).map_err(|_| "CA unlesbar".to_string())?;
        let ca = ca_pem.parse_x509().map_err(|_| "CA unparsebar".to_string())?;
        let (_, leaf_pem) =
            parse_x509_pem(cert_pem.as_bytes()).map_err(|_| "Ausweis unlesbar".to_string())?;
        let leaf = leaf_pem.parse_x509().map_err(|_| "Ausweis unparsebar".to_string())?;
        leaf.verify_signature(Some(ca.public_key()))
            .map_err(|_| "Ausweis nicht von dieser CA".to_string())?;
        let not_after = leaf.validity().not_after.timestamp();
        let jetzt = time::OffsetDateTime::now_utc().unix_timestamp();
        if jetzt > not_after + grace_tage * 86_400 {
            return Err("Ausweis zu lange abgelaufen".into());
        }
        Ok(leaf.public_key().subject_public_key.data.to_vec())
    }
}

/// Zweistufige Service-CA (Roadmap 10): **Wurzel** (Vertrauensanker; ihr Schlüssel
/// darf offline verwahrt werden) + **Zwischen-CA** (signiert die Blatt-Ausweise am
/// Server). Jeder ausgestellte Ausweis trägt die Kette Blatt→Zwischen. Wird der
/// Server geknackt, ist nur die Zwischen-CA betroffen – aus der (offline) Wurzel
/// stellt man eine neue aus, statt „alles neu".
///
/// Caddy vertraut der Wurzel UND der Zwischen-CA direkt (Caddyfile.netcup). Grund:
/// Die 2026-07 erzeugte Produktiv-Wurzel trägt pathLenConstraint 0 und erlaubt
/// keine Zwischen-CA unter sich – Go/Caddy lehnt Blatt→Zwischen→Wurzel deshalb ab
/// („too many intermediates"). Mit der Zwischen-CA als eigenem Vertrauensanker
/// endet die Kette dort. Neu erzeugte Wurzeln erlauben eine Zwischen-CA.
pub struct StufenCa {
    /// Wurzel-Zertifikat (öffentlich; Caddy-Trust-Anker + `service_ca_pem` in Antworten).
    pub wurzel_cert_pem: String,
    /// Zwischen-Zertifikat – wird an jeden Blatt-Ausweis angehängt (Kette).
    pub zwischen_cert_pem: String,
    /// Zwischen-Schlüssel – signiert die Blatt-Ausweise (bleibt am Server).
    zwischen_key_pem: String,
}

impl StufenCa {
    /// Lädt/erzeugt Wurzel (`service-ca.crt/.key`, unverändert – bestehende bleibt)
    /// und Zwischen-CA (`service-int.crt/.key`). Fehlt die Zwischen-CA, wird sie
    /// EINMALIG aus der Wurzel erzeugt (braucht dafür den Wurzel-Schlüssel; danach
    /// darf er offline – die Zwischen-CA signiert im Alltag allein).
    pub fn laden_oder_erzeugen(dir: &Path) -> Result<Self, String> {
        let wurzel_crt = dir.join("service-ca.crt");
        let int_crt = dir.join("service-int.crt");
        let int_key = dir.join("service-int.key");

        // Ist die Zwischen-CA schon da? Dann reicht das Wurzel-ZERTIFIKAT als
        // Vertrauensanker – der Wurzel-SCHLÜSSEL wird im Alltag NICHT gebraucht
        // (er darf offline liegen). WICHTIG: hier die Wurzel NICHT neu erzeugen,
        // sonst würde ein offline genommener Schlüssel eine neue Wurzel auslösen
        // und alle bestehenden Ausweise entwerten.
        if let (Ok(zc), Ok(zk), Ok(wc)) = (
            std::fs::read_to_string(&int_crt),
            std::fs::read_to_string(&int_key),
            std::fs::read_to_string(&wurzel_crt),
        ) {
            if !zc.trim().is_empty() && !zk.trim().is_empty() && !wc.trim().is_empty() {
                return Ok(StufenCa {
                    wurzel_cert_pem: wc,
                    zwischen_cert_pem: zc,
                    zwischen_key_pem: zk,
                });
            }
        }

        // Erststart (noch keine Zwischen-CA): Wurzel laden/erzeugen (braucht den
        // Wurzel-Schlüssel) und die Zwischen-CA einmalig daraus ableiten.
        let wurzel = Ca::laden_oder_erzeugen_mit(dir, "service-ca", "Antrag 3000 Service-CA", 1)?;
        let (zc, zk) = wurzel.erzeuge_zwischen_ca("Antrag 3000 Service-CA (Zwischen)")?;
        schreibe_geschuetzt(&int_key, &zk, 0o600)
            .map_err(|e| format!("Zwischen-Schlüssel nicht speicherbar: {e}"))?;
        schreibe_geschuetzt(&int_crt, &zc, 0o644)
            .map_err(|e| format!("Zwischen-Zertifikat nicht speicherbar: {e}"))?;
        Ok(StufenCa {
            wurzel_cert_pem: wurzel.cert_pem,
            zwischen_cert_pem: zc,
            zwischen_key_pem: zk,
        })
    }

    /// Fingerabdruck der Zwischen-CA (nur zum Loggen/Prüfen).
    pub fn fingerprint(&self) -> String {
        fingerprint_von_pem(&self.zwischen_cert_pem).unwrap_or_default()
    }

    /// Signiert einen Blatt-Ausweis mit dem ZWISCHEN-Schlüssel (kopiersicher aus
    /// dem öffentlichen Schlüssel des Gegenübers) und hängt das Zwischen-Zertifikat
    /// an → fertige Kette (Blatt + Zwischen) für den Client. Der Fingerabdruck des
    /// Ausweises ist der des BLATTS (erstes Zertifikat).
    pub fn ausweis_kette(
        &self,
        pubkey_pem: &str,
        cn: &str,
        org: &str,
        gueltig_tage: i64,
    ) -> Result<String, String> {
        let int_key = rcgen::KeyPair::from_pem(&self.zwischen_key_pem)
            .map_err(|e| format!("Zwischen-Schlüssel unlesbar: {e}"))?;
        let int_cert = rcgen::CertificateParams::from_ca_cert_pem(&self.zwischen_cert_pem)
            .map_err(|e| format!("Zwischen-Zertifikat unlesbar: {e}"))?
            .self_signed(&int_key)
            .map_err(|e| format!("Zwischen-CA nicht rekonstruierbar: {e}"))?;
        let pubkey = rcgen::SubjectPublicKeyInfo::from_pem(pubkey_pem)
            .map_err(|e| format!("Öffentlicher Schlüssel unlesbar: {e}"))?;
        let cn = cn.trim();
        if cn.is_empty() {
            return Err("Name (CN) fehlt.".into());
        }
        let mut p = rcgen::CertificateParams::new(Vec::<String>::new())
            .map_err(|e| format!("Ausweis-Parameter fehlerhaft: {e}"))?;
        p.distinguished_name
            .push(rcgen::DnType::OrganizationName, org);
        p.distinguished_name.push(rcgen::DnType::CommonName, cn);
        p.is_ca = rcgen::IsCa::NoCa;
        p.key_usages = vec![rcgen::KeyUsagePurpose::DigitalSignature];
        // Nur zum Anmelden (mTLS-Client) – nicht als Server-Zertifikat nutzbar.
        p.extended_key_usages = vec![rcgen::ExtendedKeyUsagePurpose::ClientAuth];
        let jetzt = time::OffsetDateTime::now_utc();
        p.not_before = jetzt - time::Duration::hours(1);
        p.not_after = jetzt + time::Duration::days(gueltig_tage.max(1));
        let blatt = p
            .signed_by(&pubkey, &int_cert, &int_key)
            .map_err(|e| format!("Ausweis nicht signierbar: {e}"))?;
        Ok(format!("{}{}", blatt.pem(), self.zwischen_cert_pem))
    }

    /// Grace-Wiederanmeldung (Roadmap 10): das Blatt (erstes Zertifikat in
    /// `cert_pem`) muss von der Zwischen-CA ODER – für Alt-Ausweise – direkt von der
    /// Wurzel signiert sein und höchstens `grace_tage` abgelaufen. Gibt bei Erfolg
    /// den öffentlichen Blatt-Schlüssel (SEC1) für die Besitznachweis-Prüfung zurück.
    pub fn grace_pubkey(&self, cert_pem: &str, grace_tage: i64) -> Result<Vec<u8>, String> {
        use x509_parser::pem::parse_x509_pem;
        let (_, blatt_pem) =
            parse_x509_pem(cert_pem.as_bytes()).map_err(|_| "Ausweis unlesbar".to_string())?;
        let blatt = blatt_pem
            .parse_x509()
            .map_err(|_| "Ausweis unparsebar".to_string())?;
        let von_uns = [&self.zwischen_cert_pem, &self.wurzel_cert_pem]
            .iter()
            .any(|ca_pem| {
                parse_x509_pem(ca_pem.as_bytes())
                    .ok()
                    .and_then(|(_, p)| {
                        p.parse_x509()
                            .ok()
                            .map(|c| blatt.verify_signature(Some(c.public_key())).is_ok())
                    })
                    .unwrap_or(false)
            });
        if !von_uns {
            return Err("Ausweis nicht von dieser CA".into());
        }
        let not_after = blatt.validity().not_after.timestamp();
        let jetzt = time::OffsetDateTime::now_utc().unix_timestamp();
        if jetzt > not_after + grace_tage * 86_400 {
            return Err("Ausweis zu lange abgelaufen".into());
        }
        Ok(blatt.public_key().subject_public_key.data.to_vec())
    }
}

/// SHA-256-Fingerabdruck (Hex) EINES Zertifikat-PEMs. Genau der Wert, den der
/// Server aus dem von Caddy durchgereichten Zertifikat (DER) bildet – so
/// stimmt der bei der Ausstellung eingetragene Fingerabdruck später mit dem
/// überein, den das verbundene Gegenüber vorweist.
pub fn fingerprint_von_pem(cert_pem: &str) -> Option<String> {
    let der = pem_zu_der(cert_pem)?;
    let mut h = Sha256::new();
    h.update(&der);
    Some(hex::encode(h.finalize()))
}

/// Wandelt ein einzelnes PEM-Objekt in seine DER-Bytes. Ohne Extra-Crate:
/// Base64 zwischen den BEGIN/END-Zeilen dekodieren.
fn pem_zu_der(pem: &str) -> Option<Vec<u8>> {
    use base64::Engine;
    // Nur das ERSTE Zertifikat: ein Ausweis kann eine Kette sein (Blatt +
    // Zwischen-CA); der Fingerabdruck ist immer der des BLATTS (erstes Zert),
    // genau wie ihn Caddy aus dem vorgelegten Client-Zertifikat bildet.
    const B: &str = "-----BEGIN CERTIFICATE-----";
    const E: &str = "-----END CERTIFICATE-----";
    let start = pem.find(B)? + B.len();
    let laenge = pem[start..].find(E)?;
    let b64: String = pem[start..start + laenge]
        .lines()
        .map(|l| l.trim())
        .collect::<String>();
    base64::engine::general_purpose::STANDARD.decode(b64).ok()
}

/// Schreibt eine Datei und setzt (auf Unix) enge Rechte. Auf Windows – dem
/// Entwicklungs-OS – gibt es kein chmod; dort greifen die NTFS-Standardrechte.
fn schreibe_geschuetzt(pfad: &PathBuf, inhalt: &str, _modus: u32) -> std::io::Result<()> {
    std::fs::write(pfad, inhalt)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(pfad, std::fs::Permissions::from_mode(_modus))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Erzeugt eine CA, simuliert ein Gegenüber (lokales Schlüsselpaar, nur der
    /// öffentliche Teil geht an die CA) und prüft: der ausgestellte Ausweis ist
    /// echt von der CA signiert (kryptografische Kettenprüfung).
    #[test]
    fn ausweis_ist_von_ca_signiert() {
        let dir = std::env::temp_dir().join(format!("a3k-ca-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let ca = Ca::laden_oder_erzeugen(&dir, "service-ca", "Antrag 3000 Service-CA").expect("CA erzeugen");

        // Idempotenz: zweiter Aufruf liefert dieselbe CA.
        let ca2 = Ca::laden_oder_erzeugen(&dir, "service-ca", "Antrag 3000 Service-CA").expect("CA laden");
        assert_eq!(ca.cert_pem, ca2.cert_pem, "CA darf nicht neu erzeugt werden");

        // Gegenüber: Schlüsselpaar lokal, nur der öffentliche Teil "gesendet".
        let key = rcgen::KeyPair::generate().unwrap();
        let pubkey_pem = key.public_key_pem();

        let ausweis_pem = ca
            .signiere(&pubkey_pem, "Test-Gerät", "Antrag 3000 Team-Gerät", 90)
            .expect("Ausweis signieren");
        assert!(ausweis_pem.contains("BEGIN CERTIFICATE"));

        use x509_parser::pem::parse_x509_pem;
        let (_, ca_pem) = parse_x509_pem(ca.cert_pem.as_bytes()).unwrap();
        let ca_x509 = ca_pem.parse_x509().unwrap();
        let (_, leaf_pem) = parse_x509_pem(ausweis_pem.as_bytes()).unwrap();
        let leaf_x509 = leaf_pem.parse_x509().unwrap();
        leaf_x509
            .verify_signature(Some(ca_x509.public_key()))
            .expect("Ausweis muss von der CA signiert sein");

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Zwei CAs im selben Ordner (Service + Förderer) sind getrennt und
    /// unterschiedlich.
    #[test]
    fn zwei_getrennte_cas() {
        let dir = std::env::temp_dir().join(format!("a3k-ca2-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let service = Ca::laden_oder_erzeugen(&dir, "service-ca", "Antrag 3000 Service-CA").unwrap();
        let foerderer = Ca::laden_oder_erzeugen(&dir, "foerderer-ca", "Antrag 3000 Förderer-CA").unwrap();
        assert_ne!(service.cert_pem, foerderer.cert_pem);
        assert_ne!(service.fingerprint(), foerderer.fingerprint());
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// D-12: Blatt-Ausweise taugen nur zum Anmelden (EKU clientAuth, kein
    /// serverAuth); die CAs selbst tragen keine EKU-Einschränkung. Ist
    /// `A3K_EKU_DIR` gesetzt, landen die Zertifikate (ohne Schlüssel) als PEM dort –
    /// für die Gegenprobe mit `openssl verify -purpose sslclient|sslserver`.
    #[test]
    fn ausweise_nur_zum_anmelden() {
        use x509_parser::pem::parse_x509_pem;
        // (clientAuth, serverAuth, any) des ERSTEN Zertifikats; None = keine EKU.
        fn eku(pem: &str) -> Option<(bool, bool, bool)> {
            let (_, p) = parse_x509_pem(pem.as_bytes()).unwrap();
            let c = p.parse_x509().unwrap();
            c.extended_key_usage()
                .unwrap()
                .map(|e| (e.value.client_auth, e.value.server_auth, e.value.any))
        }
        let dir = std::env::temp_dir().join(format!("a3k-eku-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let key = rcgen::KeyPair::generate().unwrap();

        // Zweistufig (Team-Geräte): Blatt aus der Zwischen-CA.
        let stufen = StufenCa::laden_oder_erzeugen(&dir).unwrap();
        let kette = stufen
            .ausweis_kette(&key.public_key_pem(), "Test-Gerät", "Antrag 3000 Team-Gerät", 90)
            .unwrap();
        assert_eq!(eku(&kette), Some((true, false, false)), "Blatt: nur clientAuth");
        assert_eq!(eku(&stufen.zwischen_cert_pem), None, "Zwischen-CA ohne EKU");
        assert_eq!(eku(&stufen.wurzel_cert_pem), None, "Wurzel ohne EKU");

        // Einstufig (Förderer-CA).
        let foerderer = Ca::laden_oder_erzeugen(&dir, "foerderer-ca", "Antrag 3000 Förderer-CA").unwrap();
        let ausweis = foerderer
            .signiere(&key.public_key_pem(), "Test-Förderer", "Antrag 3000 Förderer", 90)
            .unwrap();
        assert_eq!(eku(&ausweis), Some((true, false, false)), "Förderer: nur clientAuth");
        assert_eq!(eku(&foerderer.cert_pem), None, "Förderer-CA ohne EKU");

        if let Some(aus) = std::env::var_os("A3K_EKU_DIR") {
            let aus = PathBuf::from(aus);
            std::fs::create_dir_all(&aus).unwrap();
            for (name, inhalt) in [
                ("wurzel.pem", &stufen.wurzel_cert_pem),
                ("zwischen.pem", &stufen.zwischen_cert_pem),
                ("blatt-team.pem", &kette),
                ("foerderer-ca.pem", &foerderer.cert_pem),
                ("blatt-foerderer.pem", &ausweis),
            ] {
                std::fs::write(aus.join(name), inhalt).unwrap();
            }
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Pfadlängen-Regel wie Go/Caddy (crypto/x509/verify.go, `isValid`): jede CA
    /// der Kette darf höchstens so viele Zwischen-CAs unter sich haben, wie ihr
    /// pathLenConstraint erlaubt – auch der Vertrauensanker. `kette` = Blatt
    /// zuerst, Anker zuletzt; je Eintrag zählt das erste Zertifikat im PEM.
    fn pfadlaenge_ok(kette: &[&str]) -> bool {
        use x509_parser::pem::parse_x509_pem;
        kette.iter().enumerate().skip(1).all(|(i, pem)| {
            let (_, p) = parse_x509_pem(pem.as_bytes()).unwrap();
            let c = p.parse_x509().unwrap();
            let darunter = (i - 1) as u32; // Zwischen-CAs zwischen Blatt und dieser CA
            match c.basic_constraints().unwrap() {
                Some(bc) => {
                    bc.value.ca && bc.value.path_len_constraint.map_or(true, |n| darunter <= n)
                }
                None => false,
            }
        })
    }

    /// Regression zur Störung seit 2026-07-17: Ausweise aus der Zwischen-CA müssen
    /// die Kettenprüfung von Caddy bestehen. Neue Wurzel: erlaubt eine Zwischen-CA.
    /// Bestehende Wurzel mit pathlen 0 (Produktion): nur über die Zwischen-CA als
    /// eigenen Anker (Caddyfile.netcup) – genau das bildet der zweite Teil nach.
    #[test]
    fn kette_besteht_pfadlaengen_pruefung() {
        let key = rcgen::KeyPair::generate().unwrap();

        let dir = std::env::temp_dir().join(format!("a3k-pfad-neu-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let neu = StufenCa::laden_oder_erzeugen(&dir).unwrap();
        let kette = neu
            .ausweis_kette(&key.public_key_pem(), "Laptop", "Antrag 3000 Team-Gerät", 90)
            .unwrap();
        assert!(
            pfadlaenge_ok(&[&kette, &neu.zwischen_cert_pem, &neu.wurzel_cert_pem]),
            "neue Wurzel muss eine Zwischen-CA erlauben"
        );
        let _ = std::fs::remove_dir_all(&dir);

        // Wie in Produktion: Wurzel existiert schon (pathlen 0), Zwischen-CA kommt dazu.
        let dir = std::env::temp_dir().join(format!("a3k-pfad-alt-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        Ca::laden_oder_erzeugen(&dir, "service-ca", "Antrag 3000 Service-CA").unwrap();
        let alt = StufenCa::laden_oder_erzeugen(&dir).unwrap();
        let kette = alt
            .ausweis_kette(&key.public_key_pem(), "Laptop", "Antrag 3000 Team-Gerät", 90)
            .unwrap();
        assert!(
            !pfadlaenge_ok(&[&kette, &alt.zwischen_cert_pem, &alt.wurzel_cert_pem]),
            "pathlen-0-Wurzel lehnt die Kette ab (so wie Caddy)"
        );
        assert!(
            pfadlaenge_ok(&[&kette, &alt.zwischen_cert_pem]),
            "mit der Zwischen-CA als Anker besteht die Kette"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Offline-Wurzel: ist die Zwischen-CA erst einmal da, lädt der Server auch
    /// OHNE Wurzel-Schlüssel weiter – und erzeugt die Wurzel NICHT neu (sonst
    /// würden alle bestehenden Ausweise entwertet).
    #[test]
    fn stufen_ca_laedt_ohne_wurzel_schluessel() {
        let dir = std::env::temp_dir().join(format!("a3k-stufen-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);

        // Erststart: Wurzel + Zwischen-CA entstehen.
        let ca1 = StufenCa::laden_oder_erzeugen(&dir).unwrap();
        assert!(dir.join("service-ca.key").exists());
        assert!(dir.join("service-int.crt").exists());

        // Wurzel-Schlüssel „offline nehmen" (löschen).
        std::fs::remove_file(dir.join("service-ca.key")).unwrap();

        // Neu laden: gleiche Wurzel + gleiche Zwischen-CA, Wurzel-Schlüssel bleibt weg.
        let ca2 = StufenCa::laden_oder_erzeugen(&dir).unwrap();
        assert_eq!(ca1.wurzel_cert_pem, ca2.wurzel_cert_pem, "Wurzel darf nicht neu erzeugt werden");
        assert_eq!(ca1.zwischen_cert_pem, ca2.zwischen_cert_pem);
        assert!(
            !dir.join("service-ca.key").exists(),
            "Wurzel-Schlüssel darf nicht wieder auftauchen"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }
}
