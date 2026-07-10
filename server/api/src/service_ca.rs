// ============================================================
// Service-CA (gehostetes Modell, Schritt 2)
//
// WARUM: Im gehosteten Modell ist der DIENST der Vertrauensanker. Statt dass
// jedes Team eine eigene CA in einer App hält, besitzt der Server EINE
// Service-CA. Sie signiert Geräte-Ausweise – später automatisch beim
// Enrollment (Schritt 3), wenn Einladung + Abo gültig sind.
//
// KOPIERSICHER: Das Gerät erzeugt sein Schlüsselpaar LOKAL und schickt nur
// seinen ÖFFENTLICHEN Schlüssel (SubjectPublicKeyInfo-PEM). Der private
// Schlüssel verlässt das Gerät nie – der Server kann einen Ausweis also gar
// nicht "mitnehmen" oder duplizieren. `signiere_geraet()` ist genau dieser
// Baustein (die Enrollment-Endpunkte in Schritt 3 rufen ihn auf).
//
// SICHERHEIT: Der CA-Schlüssel (service-ca.key) ist ein sensibles Ziel. Er
// liegt nur auf dem Server, mit engen Dateirechten (0600), und wird NIE über
// eine Netzwerk-/Webview-Verbindung herausgegeben. Nur das öffentliche
// service-ca.crt wird verteilt (Caddy-Vertrauen, später Zugangs-Pakete).
// Härtung (getrennte Signier-Komponente / HSM) ist als späterer Schritt
// vorgesehen (Roadmap 10).
// ============================================================

use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

/// Die Service-CA: öffentliches Zertifikat (verteilbar) + privater Schlüssel
/// (bleibt am Server). Der Schlüssel ist bewusst NICHT `pub`, damit er nicht
/// versehentlich serialisiert oder ausgeliefert wird.
pub struct ServiceCa {
    /// PEM des CA-Zertifikats – öffentlich, kommt in Caddys Trust-Pool und
    /// später als `ca_pem` in die Zugangs-Pakete.
    pub cert_pem: String,
    /// PEM des CA-Schlüssels – GEHEIM, verlässt den Server nie.
    key_pem: String,
}

/// Dateiname des öffentlichen CA-Zertifikats im CA-Ordner.
const CERT_DATEI: &str = "service-ca.crt";
/// Dateiname des privaten CA-Schlüssels im CA-Ordner.
const KEY_DATEI: &str = "service-ca.key";

impl ServiceCa {
    /// Lädt die Service-CA aus `dir`, oder erzeugt sie beim ersten Start und
    /// legt sie dort ab. Idempotent: existiert sie schon, wird sie nur gelesen
    /// (der bestehende Schlüssel bleibt unverändert – wichtig, sonst würden
    /// alle bisher ausgestellten Ausweise ungültig).
    pub fn laden_oder_erzeugen(dir: &Path) -> Result<Self, String> {
        let cert_pfad = dir.join(CERT_DATEI);
        let key_pfad = dir.join(KEY_DATEI);

        // Bereits vorhanden? Dann nur lesen.
        if let (Ok(cert_pem), Ok(key_pem)) = (
            std::fs::read_to_string(&cert_pfad),
            std::fs::read_to_string(&key_pfad),
        ) {
            if !cert_pem.trim().is_empty() && !key_pem.trim().is_empty() {
                return Ok(ServiceCa { cert_pem, key_pem });
            }
        }

        // Sonst neu erzeugen.
        let ca = Self::erzeugen()?;
        std::fs::create_dir_all(dir)
            .map_err(|e| format!("CA-Ordner {dir:?} nicht anlegbar: {e}"))?;
        schreibe_geschuetzt(&key_pfad, &ca.key_pem, 0o600)
            .map_err(|e| format!("CA-Schlüssel nicht speicherbar: {e}"))?;
        schreibe_geschuetzt(&cert_pfad, &ca.cert_pem, 0o644)
            .map_err(|e| format!("CA-Zertifikat nicht speicherbar: {e}"))?;
        Ok(ca)
    }

    /// Erzeugt frisch eine selbstsignierte Service-CA (nur im Speicher).
    /// Gleiche rcgen-0.13-Bausteine wie die Förderer-CA in der Admin-App.
    fn erzeugen() -> Result<Self, String> {
        let key = rcgen::KeyPair::generate().map_err(|e| format!("Schlüssel nicht erzeugbar: {e}"))?;
        let mut p = rcgen::CertificateParams::new(Vec::<String>::new())
            .map_err(|e| format!("CA-Parameter fehlerhaft: {e}"))?;
        p.distinguished_name
            .push(rcgen::DnType::OrganizationName, "Antrag 3000");
        p.distinguished_name
            .push(rcgen::DnType::CommonName, "Antrag 3000 Service-CA");
        // Constrained(0): darf Geräte-Ausweise signieren, aber keine weiteren
        // (Unter-)CAs – ein Ausweis kann selbst nichts signieren.
        p.is_ca = rcgen::IsCa::Ca(rcgen::BasicConstraints::Constrained(0));
        p.key_usages = vec![
            rcgen::KeyUsagePurpose::KeyCertSign,
            rcgen::KeyUsagePurpose::CrlSign,
        ];
        p.not_before = rcgen::date_time_ymd(2024, 1, 1);
        p.not_after = rcgen::date_time_ymd(2035, 1, 1);
        let cert = p.self_signed(&key).map_err(|e| format!("CA nicht signierbar: {e}"))?;
        Ok(ServiceCa {
            cert_pem: cert.pem(),
            key_pem: key.serialize_pem(),
        })
    }

    /// SHA-256-Fingerabdruck (Hex) des CA-Zertifikats – nur zum Loggen/Prüfen,
    /// damit man beim Start sieht, welche CA aktiv ist.
    pub fn fingerprint(&self) -> String {
        // Aus dem PEM den DER-Rumpf lösen und hashen. Bei Fehlern (sollte nicht
        // vorkommen) den PEM-Text hashen, damit die Funktion nie paniced.
        match pem_zu_der(&self.cert_pem) {
            Some(der) => {
                let mut h = Sha256::new();
                h.update(&der);
                hex::encode(h.finalize())
            }
            None => {
                let mut h = Sha256::new();
                h.update(self.cert_pem.as_bytes());
                hex::encode(h.finalize())
            }
        }
    }

    /// Signiert einen GERÄTE-Ausweis aus dem mitgeschickten öffentlichen
    /// Schlüssel des Geräts (SubjectPublicKeyInfo-PEM). Der private Schlüssel
    /// des Geräts wird NICHT gebraucht und ist dem Server nicht bekannt –
    /// genau das macht den Ausweis kopiersicher.
    ///
    /// `cn` ist der Anzeigename/Common-Name des Geräts. Rückgabe: das
    /// Ausweis-Zertifikat als PEM. (Aufrufer: Enrollment-Endpunkte, Schritt 3.)
    #[allow(dead_code)] // wird in Schritt 3 (Enrollment-Endpunkte) verdrahtet
    pub fn signiere_geraet(&self, geraete_pubkey_pem: &str, cn: &str) -> Result<String, String> {
        // CA aus dem gespeicherten PEM rekonstruieren (Schlüssel + Zertifikat).
        let ca_key = rcgen::KeyPair::from_pem(&self.key_pem)
            .map_err(|e| format!("CA-Schlüssel unlesbar: {e}"))?;
        let ca_cert = rcgen::CertificateParams::from_ca_cert_pem(&self.cert_pem)
            .map_err(|e| format!("CA-Zertifikat unlesbar: {e}"))?
            .self_signed(&ca_key)
            .map_err(|e| format!("CA nicht rekonstruierbar: {e}"))?;

        // Nur den ÖFFENTLICHEN Schlüssel des Geräts einlesen.
        let geraete_pubkey = rcgen::SubjectPublicKeyInfo::from_pem(geraete_pubkey_pem)
            .map_err(|e| format!("Geräte-Schlüssel unlesbar: {e}"))?;

        let cn = cn.trim();
        if cn.is_empty() {
            return Err("Geräte-Name (CN) fehlt.".into());
        }
        let mut p = rcgen::CertificateParams::new(Vec::<String>::new())
            .map_err(|e| format!("Ausweis-Parameter fehlerhaft: {e}"))?;
        p.distinguished_name
            .push(rcgen::DnType::OrganizationName, "Antrag 3000 Team-Gerät");
        p.distinguished_name
            .push(rcgen::DnType::CommonName, cn);
        p.is_ca = rcgen::IsCa::NoCa;
        p.key_usages = vec![rcgen::KeyUsagePurpose::DigitalSignature];
        p.not_before = rcgen::date_time_ymd(2024, 1, 1);
        p.not_after = rcgen::date_time_ymd(2035, 1, 1);

        // Mit dem GERÄTE-Public-Key als Subjekt und der CA als Aussteller
        // signieren. Der private Geräte-Schlüssel kommt hier nicht vor.
        let cert = p
            .signed_by(&geraete_pubkey, &ca_cert, &ca_key)
            .map_err(|e| format!("Ausweis nicht signierbar: {e}"))?;
        Ok(cert.pem())
    }
}

/// Wandelt ein einzelnes PEM-Objekt in seine DER-Bytes. Ohne Extra-Crate:
/// Base64 zwischen den BEGIN/END-Zeilen dekodieren.
fn pem_zu_der(pem: &str) -> Option<Vec<u8>> {
    use base64::Engine;
    let b64: String = pem
        .lines()
        .filter(|l| !l.starts_with("-----"))
        .collect::<String>();
    base64::engine::general_purpose::STANDARD
        .decode(b64.trim())
        .ok()
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

    /// Erzeugt eine CA, simuliert ein Gerät (lokales Schlüsselpaar, nur der
    /// öffentliche Teil geht an die CA) und prüft: der ausgestellte Ausweis
    /// ist echt von der Service-CA signiert (kryptografische Kettenprüfung).
    #[test]
    fn ausweis_ist_von_service_ca_signiert() {
        let dir = std::env::temp_dir().join(format!("a3k-serviceca-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let ca = ServiceCa::laden_oder_erzeugen(&dir).expect("CA erzeugen");

        // Idempotenz: zweiter Aufruf liefert dieselbe CA (kein Neu-Erzeugen).
        let ca2 = ServiceCa::laden_oder_erzeugen(&dir).expect("CA laden");
        assert_eq!(ca.cert_pem, ca2.cert_pem, "CA darf nicht neu erzeugt werden");

        // Geräteseite: Schlüsselpaar lokal, nur der öffentliche Teil (SPKI-PEM)
        // wird "gesendet".
        let geraet_key = rcgen::KeyPair::generate().unwrap();
        let geraet_pubkey_pem = geraet_key.public_key_pem();

        let ausweis_pem = ca
            .signiere_geraet(&geraet_pubkey_pem, "Test-Gerät")
            .expect("Ausweis signieren");
        assert!(ausweis_pem.contains("BEGIN CERTIFICATE"));

        // Kettenprüfung: Signatur des Ausweises gegen den öffentlichen
        // Schlüssel der CA verifizieren.
        use x509_parser::pem::parse_x509_pem;
        let (_, ca_pem) = parse_x509_pem(ca.cert_pem.as_bytes()).unwrap();
        let ca_x509 = ca_pem.parse_x509().unwrap();
        let (_, leaf_pem) = parse_x509_pem(ausweis_pem.as_bytes()).unwrap();
        let leaf_x509 = leaf_pem.parse_x509().unwrap();
        leaf_x509
            .verify_signature(Some(ca_x509.public_key()))
            .expect("Ausweis muss von der Service-CA signiert sein");

        let _ = std::fs::remove_dir_all(&dir);
    }
}
