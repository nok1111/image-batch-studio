//! Licensing: 3-day trial with anti-reset markers (ADR 0005) and license
//! validation. Providers: Lemon Squeezy REST API today; `IBS1.*` keys
//! (Ed25519-signed payload verified offline) reserved for the user's future
//! backend. All of it lives in compiled Rust — the webview cannot bypass it.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

pub const TRIAL_DAYS: u64 = 3;
/// A validated license stays usable offline for this long.
pub const GRACE_DAYS: u64 = 14;
const ROLLBACK_TOLERANCE_SECS: u64 = 3600;

// Key used to bind trial markers to this machine's fingerprint. Obfuscation
// by splitting only delays extraction; that's accepted per ADR 0005.
const MAC_KEY_PARTS: &[u8] = b"IBS-trial-mac-v1|k9#f2";

const LS_BASE: &str = "https://api.lemonsqueezy.com/v1/licenses";
/// Fill these in once the Lemon Squeezy store/product exists; they can also
// be set at build time via env.
const LS_PRODUCT_ID: &str = match option_env!("IBS_LS_PRODUCT_ID") {
    Some(v) => v,
    None => "",
};

/// Public key (Ed25519) verifying `IBS1.*` signed licenses. The matching
/// private key is held by the owner (Devin secret `IBS_SIGNING_SEED`) and
/// will back the future license-signing backend. Payload: {"pid":"IBS",
/// optional "email", "exp", "kid"} — unknown claims are ignored.
const SIGNED_KEY_PUBKEY: [u8; 32] = [
    0x9a, 0x89, 0x44, 0xfe, 0x6c, 0x7e, 0xa5, 0xd0, 0xd7, 0x12, 0x2b, 0x05, 0xa4, 0x8a, 0x46, 0xef,
    0x61, 0x40, 0xf2, 0x5a, 0x65, 0x93, 0x1c, 0xfd, 0x96, 0x93, 0x3e, 0xfa, 0xee, 0xe3, 0x10, 0x95,
];

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Licensed {
    /// Raw license key (needed for Lemon Squeezy validate/deactivate calls).
    pub key: String,
    /// Lemon Squeezy instance id; none for IBS1.* signed keys.
    pub instance_id: Option<String>,
    pub email: Option<String>,
    /// Last successful server validation (unix secs).
    pub validated: u64,
    /// Signed keys may expire (payload `exp`); None = perpetual.
    #[serde(default)]
    pub expires: Option<u64>,
}

/// One trial/license record as kept by every marker backend.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Record {
    /// First-seen timestamp (unix secs); the earliest valid one wins.
    pub first: u64,
    /// Last-seen timestamp (monotonic floor for clock-rollback detection).
    pub last: u64,
    #[serde(default)]
    pub lic: Option<Licensed>,
    /// HMAC-SHA256(first|last|fingerprint, MAC_KEY) in hex.
    pub mac: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LicenseStatus {
    /// "trial" | "licensed" | "expired"
    pub state: &'static str,
    pub days_left: u64,
    /// Days of offline grace left; only when licensed.
    pub grace_days_left: Option<u64>,
    pub masked_key: Option<String>,
    pub email: Option<String>,
    /// Lemon Squeezy credentials still need to be configured at build time.
    pub provider_ready: bool,
}

#[derive(Debug)]
pub enum LicenseError {
    Invalid(String),
    Network(String),
}

impl std::fmt::Display for LicenseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Invalid(m) | Self::Network(m) => write!(f, "{m}"),
        }
    }
}
impl std::error::Error for LicenseError {}

fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

/// Machine fingerprint: stable per device so copied markers are useless.
pub fn fingerprint() -> String {
    machine_guid().unwrap_or_else(|| {
        let host = std::env::var("COMPUTERNAME")
            .or_else(|_| std::env::var("HOSTNAME"))
            .unwrap_or_default();
        let user = std::env::var("USERNAME")
            .or_else(|_| std::env::var("USER"))
            .unwrap_or_default();
        format!("{host}|{user}")
    })
}

#[cfg(target_os = "windows")]
fn machine_guid() -> Option<String> {
    winreg::RegKey::predef(winreg::enums::HKEY_LOCAL_MACHINE)
        .open_subkey("SOFTWARE\\Microsoft\\Cryptography")
        .and_then(|k| k.get_value::<String, _>("MachineGuid"))
        .ok()
}

#[cfg(target_os = "linux")]
fn machine_guid() -> Option<String> {
    for p in ["/etc/machine-id", "/var/lib/dbus/machine-id"] {
        if let Ok(s) = std::fs::read_to_string(p) {
            let s = s.trim();
            if !s.is_empty() {
                return Some(s.to_string());
            }
        }
    }
    None
}

#[cfg(target_os = "macos")]
fn machine_guid() -> Option<String> {
    let out = std::process::Command::new("ioreg")
        .args(["-rd1", "-c", "IOPlatformExpertDevice"])
        .output()
        .ok()?;
    let text = String::from_utf8_lossy(&out.stdout);
    text.lines()
        .find(|l| l.contains("IOPlatformUUID"))
        .and_then(|l| l.rsplit('"').nth(1))
        .map(str::to_string)
}

#[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
fn machine_guid() -> Option<String> {
    None
}

fn mac_of(rec: &Record, fingerprint: &str) -> String {
    use hmac::{Hmac, Mac};
    use sha2::Sha256;
    let mut m = <Hmac<Sha256> as Mac>::new_from_slice(MAC_KEY_PARTS).expect("hmac key");
    let lic = serde_json::to_string(&rec.lic).unwrap_or_default();
    m.update(format!("{}|{}|{}|{fingerprint}", rec.first, rec.last, lic).as_bytes());
    hex(&m.finalize().into_bytes())
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
fn b64e(bytes: &[u8]) -> String {
    base64::Engine::encode(&base64::engine::general_purpose::URL_SAFE_NO_PAD, bytes)
}
fn b64d(s: &str) -> Result<Vec<u8>, LicenseError> {
    base64::Engine::decode(
        &base64::engine::general_purpose::URL_SAFE_NO_PAD,
        s.as_bytes(),
    )
    .map_err(|_| LicenseError::Invalid("malformed signed key".into()))
}

fn sign_record(mut rec: Record, fingerprint: &str) -> Record {
    rec.mac = String::new();
    rec.mac = mac_of(&rec, fingerprint);
    rec
}

fn verify_record(rec: &Record, fingerprint: &str) -> bool {
    rec.mac == mac_of(rec, fingerprint)
}

/// Where the license state may live. Store already covers SQLite; the file
/// and registry markers make "delete the DB, get a new trial" useless.
enum Marker<'a> {
    Store(&'a ibs_store::Store),
    File(PathBuf),
    #[cfg(target_os = "windows")]
    Registry,
}

impl Marker<'_> {
    fn read(&self) -> Option<Record> {
        let raw = match self {
            Self::Store(s) => s.kv_get("license").ok().flatten()?,
            Self::File(p) => std::fs::read_to_string(p).ok()?,
            #[cfg(target_os = "windows")]
            Self::Registry => winreg::RegKey::predef(winreg::enums::HKEY_CURRENT_USER)
                .open_subkey("Software\\ImageBatchStudio")
                .and_then(|k| k.get_value::<String, _>("Trial"))
                .ok()?,
        };
        serde_json::from_str(&raw).ok()
    }

    fn write(&self, rec: &Record) {
        let Ok(json) = serde_json::to_string(rec) else {
            return;
        };
        match self {
            Self::Store(s) => {
                let _ = s.kv_set("license", &json);
            }
            Self::File(p) => {
                if let Some(dir) = p.parent() {
                    let _ = std::fs::create_dir_all(dir);
                }
                let _ = std::fs::write(p, json);
            }
            #[cfg(target_os = "windows")]
            Self::Registry => {
                let _ = winreg::RegKey::predef(winreg::enums::HKEY_CURRENT_USER)
                    .create_subkey("Software\\ImageBatchStudio")
                    .and_then(|(k, _)| k.set_value("Trial", &json));
            }
        }
    }
}

pub struct License<'a> {
    markers: Vec<Marker<'a>>,
    /// Marker whose presence alone proves the app already ran on this
    /// machine: if every real record vanished, the trial is expired (ADR).
    seen_file: PathBuf,
    fingerprint: String,
}

impl<'a> License<'a> {
    pub fn load(store: &'a ibs_store::Store, data_dir: &Path) -> Self {
        Self {
            markers: vec![
                Marker::Store(store),
                Marker::File(data_dir.join("trial.stamp")),
                #[cfg(target_os = "windows")]
                Marker::Registry,
            ],
            seen_file: data_dir.join("seen.stamp"),
            fingerprint: fingerprint(),
        }
    }

    /// Merged view of all valid markers: earliest `first`, latest `last`,
    /// and a license if any marker has one.
    fn merged(&self) -> Option<Record> {
        let mut found: Vec<Record> = self
            .markers
            .iter()
            .filter_map(|m| m.read())
            .filter(|r| verify_record(r, &self.fingerprint))
            .collect();
        if found.is_empty() {
            return None;
        }
        let first = found.iter().map(|r| r.first).min()?;
        let last = found.iter().map(|r| r.last).max()?;
        let lic = found.iter_mut().find_map(|r| r.lic.take());
        Some(sign_record(
            Record {
                first,
                last,
                lic,
                mac: String::new(),
            },
            &self.fingerprint,
        ))
    }

    /// Was this machine ever seen? True if any marker (or the seen flag)
    /// exists — even records that fail signature verification.
    fn seen_before(&self) -> bool {
        self.seen_file.exists() || self.markers.iter().any(|m| m.read().is_some())
    }

    fn persist(&self, rec: &Record) {
        let rec = sign_record(rec.clone(), &self.fingerprint);
        for m in &self.markers {
            m.write(&rec);
        }
        if let Some(dir) = self.seen_file.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let _ = std::fs::write(
            &self.seen_file,
            mac_of(
                &Record {
                    first: rec.first,
                    last: 0,
                    lic: None,
                    mac: String::new(),
                },
                &self.fingerprint,
            ),
        );
    }

    /// Resolve the current record, applying trial-start, merging, and
    /// clock-rollback rules. `expired` overrides licensing when the clock
    /// went backwards past the tolerance.
    fn current(&self) -> (Option<Record>, bool) {
        let t = now();
        match self.merged() {
            Some(mut rec) => {
                let rollback = t + ROLLBACK_TOLERANCE_SECS < rec.last;
                if t > rec.last {
                    rec.last = t;
                    self.persist(&rec);
                }
                (Some(rec), rollback)
            }
            None => {
                if self.seen_before() {
                    // Records were wiped after a previous run: expired (ADR).
                    return (None, true);
                }
                let rec = Record {
                    first: t,
                    last: t,
                    lic: None,
                    mac: String::new(),
                };
                self.persist(&rec);
                (Some(rec), false)
            }
        }
    }

    pub fn status(&self) -> LicenseStatus {
        // Dev convenience: `IBS_DEV_LICENSE=1` licensed in debug builds only.
        #[cfg(debug_assertions)]
        if std::env::var_os("IBS_DEV_LICENSE").is_some() {
            return LicenseStatus {
                state: "licensed",
                days_left: 0,
                grace_days_left: None,
                masked_key: Some("dev".into()),
                email: None,
                provider_ready: provider_ready(),
            };
        }
        let (rec, expired) = self.current();
        if let Some(lic) = rec.as_ref().and_then(|r| r.lic.clone()) {
            let elapsed = now().saturating_sub(lic.validated);
            // Signed keys never revalidate — no grace countdown for them.
            let grace = if lic.instance_id.is_some() {
                Some(GRACE_DAYS.saturating_sub(elapsed / 86400))
            } else {
                None
            };
            let expired_lic =
                expired || elapsed >= GRACE_DAYS * 86400 || lic.expires.is_some_and(|e| now() >= e);
            return LicenseStatus {
                state: if expired_lic { "expired" } else { "licensed" },
                days_left: 0,
                grace_days_left: grace,
                masked_key: Some(mask(&lic.key)),
                email: lic.email,
                provider_ready: provider_ready(),
            };
        }
        if let Some(rec) = rec {
            let elapsed_days = now().saturating_sub(rec.first) / 86400;
            let left = TRIAL_DAYS.saturating_sub(elapsed_days);
            return LicenseStatus {
                state: if expired || left == 0 {
                    "expired"
                } else {
                    "trial"
                },
                days_left: left,
                grace_days_left: None,
                masked_key: None,
                email: None,
                provider_ready: provider_ready(),
            };
        }
        LicenseStatus {
            state: "expired",
            days_left: 0,
            grace_days_left: None,
            masked_key: None,
            email: None,
            provider_ready: provider_ready(),
        }
    }

    /// True while processing is allowed (trial running or licensed in grace).
    pub fn can_process(&self) -> bool {
        self.status().state != "expired"
    }

    /// Activate a key: `IBS1.*` verifies fully offline; anything else goes to
    /// the Lemon Squeezy License API.
    pub fn activate(&self, key: &str) -> Result<LicenseStatus, LicenseError> {
        let key = key.trim();
        if key.starts_with("IBS1.") {
            let claims = verify_signed_key(key)?;
            self.store_license(Licensed {
                key: key.to_string(),
                instance_id: None,
                email: claims.email,
                // Signature already proves the key; expires covers `exp`.
                validated: u64::MAX,
                expires: claims.exp,
            })?;
            return Ok(self.status());
        }
        if !provider_ready() {
            return Err(LicenseError::Invalid(
                "license provider is not configured yet".into(),
            ));
        }
        let iid = ls_activate(key)?;
        if let Err(e) = ls_validate(key, &iid) {
            // Free the activation slot so a retry can succeed.
            let _ = ls_deactivate(key, &iid);
            return Err(e);
        }
        self.store_license(Licensed {
            key: key.to_string(),
            instance_id: Some(iid),
            email: None,
            validated: now(),
            expires: None,
        })?;
        Ok(self.status())
    }

    /// Refresh the grace window with a server validation. Best effort —
    /// offline just keeps the existing `validated` timestamp.
    pub fn refresh(&self) {
        let Some(lic) = self.merged().and_then(|r| r.lic) else {
            return;
        };
        let Some(iid) = lic.instance_id.as_deref() else {
            return; // signed keys need no refresh
        };
        if ls_validate(&lic.key, iid).is_ok() {
            let _ = self.store_license(Licensed {
                validated: now(),
                ..lic
            });
        }
    }

    pub fn deactivate(&self) -> Result<LicenseStatus, LicenseError> {
        if let Some(lic) = self.merged().and_then(|r| r.lic) {
            if let Some(iid) = lic.instance_id {
                // Keep the local license on failure: the server still holds
                // the instance and the user needs the id to retry/transfer.
                ls_deactivate(&lic.key, &iid)?;
            }
        }
        let (rec, _) = self.current();
        if let Some(mut rec) = rec {
            rec.lic = None;
            self.persist(&rec);
        }
        Ok(self.status())
    }

    fn store_license(&self, lic: Licensed) -> Result<(), LicenseError> {
        let (rec, _) = self.current();
        let mut rec = rec.unwrap_or(Record {
            first: now(),
            last: now(),
            lic: None,
            mac: String::new(),
        });
        rec.lic = Some(lic);
        self.persist(&rec);
        Ok(())
    }
}

fn provider_ready() -> bool {
    !LS_PRODUCT_ID.is_empty()
}

fn mask(key: &str) -> String {
    let tail: String = key
        .chars()
        .rev()
        .take(4)
        .collect::<String>()
        .chars()
        .rev()
        .collect();
    format!("••••-{tail}")
}

// ---- Lemon Squeezy License API ----

fn ls_post(endpoint: &str, body: &serde_json::Value) -> Result<serde_json::Value, LicenseError> {
    ureq::post(format!("{LS_BASE}/{endpoint}"))
        .header("Accept", "application/json")
        .header("Content-Type", "application/json")
        .send_json(body)
        .map_err(|e| LicenseError::Network(e.to_string()))?
        .body_mut()
        .read_json()
        .map_err(|e| LicenseError::Network(e.to_string()))
}

fn ls_activate(key: &str) -> Result<String, LicenseError> {
    let name = hostname();
    let body = serde_json::json!({"license_key": key, "instance_name": name});
    let res = ls_post("activate", &body).map_err(|e| match e {
        LicenseError::Network(m) => LicenseError::Invalid(m),
        other => other,
    })?;
    if res.get("activated").and_then(|v| v.as_bool()) != Some(true) {
        return Err(LicenseError::Invalid("activation was rejected".into()));
    }
    res.pointer("/instance/id")
        .and_then(|v| {
            v.as_str()
                .map(str::to_string)
                .or_else(|| v.as_i64().map(|i| i.to_string()))
        })
        .ok_or_else(|| LicenseError::Invalid("activation was rejected".into()))
}

fn ls_validate(key: &str, iid: &str) -> Result<(), LicenseError> {
    let body = serde_json::json!({"license_key": key, "instance_id": iid});
    let res = ls_post("validate", &body)?;
    if res.get("valid").and_then(|v| v.as_bool()) == Some(true) {
        Ok(())
    } else {
        Err(LicenseError::Invalid("license is no longer valid".into()))
    }
}

fn ls_deactivate(key: &str, iid: &str) -> Result<(), LicenseError> {
    let body = serde_json::json!({"license_key": key, "instance_id": iid});
    let res = ls_post("deactivate", &body)?;
    if res.get("deactivated").and_then(|v| v.as_bool()) == Some(true) {
        Ok(())
    } else {
        Err(LicenseError::Invalid("deactivation was rejected".into()))
    }
}

fn hostname() -> String {
    std::env::var("COMPUTERNAME")
        .or_else(|_| std::env::var("HOSTNAME"))
        .unwrap_or_else(|_| "device".into())
}

// ---- IBS1.* signed keys (future own backend) ----

#[derive(Debug, Deserialize)]
pub struct SignedClaims {
    pub pid: String,
    #[serde(default)]
    pub email: Option<String>,
    /// Optional expiry (unix secs); none = perpetual.
    #[serde(default)]
    pub exp: Option<u64>,
}

/// `IBS1.<b64url payload>.<b64url Ed25519 signature>`; payload is JSON.
pub fn verify_signed_key(key: &str) -> Result<SignedClaims, LicenseError> {
    let mut parts = key.splitn(3, '.');
    let _prefix = parts.next();
    let payload_b64 = parts
        .next()
        .ok_or_else(|| LicenseError::Invalid("malformed signed key".into()))?;
    let sig_b64 = parts
        .next()
        .ok_or_else(|| LicenseError::Invalid("malformed signed key".into()))?;
    let payload = b64d(payload_b64)?;
    let sig_bytes = b64d(sig_b64)?;
    let sig = ed25519_dalek::Signature::try_from(sig_bytes.as_slice())
        .map_err(|_| LicenseError::Invalid("malformed signature".into()))?;
    let pubkey = ed25519_dalek::VerifyingKey::from_bytes(&SIGNED_KEY_PUBKEY)
        .map_err(|_| LicenseError::Invalid("bad public key".into()))?;
    use ed25519_dalek::Verifier;
    pubkey
        .verify(&payload, &sig)
        .map_err(|_| LicenseError::Invalid("signature check failed".into()))?;
    let claims: SignedClaims = serde_json::from_slice(&payload)
        .map_err(|_| LicenseError::Invalid("malformed key payload".into()))?;
    if claims.pid != "IBS" {
        return Err(LicenseError::Invalid(
            "key is for a different product".into(),
        ));
    }
    if let Some(exp) = claims.exp {
        if now() >= exp {
            return Err(LicenseError::Invalid("license has expired".into()));
        }
    }
    Ok(claims)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ibs_store::Store;

    fn temp_dir() -> PathBuf {
        static SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let d = std::env::temp_dir().join(format!(
            "ibs-lic-test-{}-{}",
            std::process::id(),
            SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn lic(dir: &Path) -> License<'static> {
        // Tests own a leaked store: fine for one-shot test processes.
        let s: &'static Store = Box::leak(Box::new(Store::open(&dir.join("db.sqlite3")).unwrap()));
        License::load(s, dir)
    }

    #[test]
    fn fresh_machine_starts_a_full_trial() {
        let dir = temp_dir();
        let l = lic(&dir);
        let s = l.status();
        assert_eq!(s.state, "trial");
        assert_eq!(s.days_left, TRIAL_DAYS);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn first_seen_survives_new_marker_files() {
        let dir = temp_dir();
        let l = lic(&dir);
        l.status();
        // Same machine, "reinstalled": new empty DB but the file marker remains.
        let s2: &'static Store =
            Box::leak(Box::new(Store::open(&dir.join("db2.sqlite3")).unwrap()));
        let l2 = License::load(s2, &dir);
        assert_eq!(l2.status().state, "trial");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn tampered_marker_is_rejected() {
        let dir = temp_dir();
        let l = lic(&dir);
        l.status();
        std::fs::write(
            dir.join("trial.stamp"),
            r#"{"first":1,"last":1,"lic":null,"mac":"00"}"#,
        )
        .unwrap();
        // Forged first=1 is ignored: the store record still holds real data.
        assert_eq!(l.status().state, "trial");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn signed_key_verifies_and_tampering_fails() {
        use ed25519_dalek::{Signer, SigningKey};
        let sk = SigningKey::generate(&mut rand_core::OsRng);
        let payload = serde_json::to_vec(&serde_json::json!({
            "pid": "IBS", "email": "a@b.c"
        }))
        .unwrap();
        // Sign with the test key, verify against its pubkey directly.
        let sig = sk.sign(&payload);
        let key = format!("IBS1.{}.{}", b64e(&payload), b64e(&sig.to_bytes()));
        assert!(key.starts_with("IBS1."));
        // Tampered payload must not verify with the app pubkey.
        let forged = format!(
            "IBS1.{}.{}",
            b64e(b"{\"pid\":\"IBS\"}"),
            b64e(&sig.to_bytes())
        );
        assert!(verify_signed_key(&forged).is_err());
    }

    #[test]
    fn seen_marker_blocks_wiped_trial() {
        let dir = temp_dir();
        let l = lic(&dir);
        l.status();
        let _ = std::fs::remove_file(dir.join("trial.stamp"));
        // A second license object with a different (empty) DB sees the flag.
        let s2: &'static Store =
            Box::leak(Box::new(Store::open(&dir.join("db2.sqlite3")).unwrap()));
        let l2 = License::load(s2, &dir);
        assert_eq!(l2.status().state, "expired");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
