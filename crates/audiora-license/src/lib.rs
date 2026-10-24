//! Offline license files.
//!
//! A license is a JSON envelope `{ "claims": "<json string>", "sig": "<base64>" }`
//! where `sig` is an Ed25519 signature over the exact bytes of the `claims`
//! string. Only the public key ships in the product; the private key stays
//! with Roujan Digital Agency (see the `audiora-license` tool).
//!
//! A license may be locked to an R Interface serial number: it then only
//! validates while that device is connected.

use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine as _;
use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};

pub const PRODUCT: &str = "audiora";

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct Claims {
    pub product: String,
    /// "play" or "studio".
    pub edition: String,
    pub issued_to: String,
    /// When set, the license only validates while this R Interface is present.
    pub device_serial: Option<String>,
    pub issued_at: u64,
    /// Unix seconds; `None` = perpetual.
    pub expires_at: Option<u64>,
    #[serde(default)]
    pub features: Vec<String>,
}

#[derive(Serialize, Deserialize)]
struct Envelope {
    claims: String,
    sig: String,
}

#[derive(Debug, PartialEq, Eq)]
pub enum LicenseError {
    Malformed,
    BadSignature,
    WrongProduct,
    Expired,
    DeviceMismatch,
}

impl std::fmt::Display for LicenseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            LicenseError::Malformed => "license file is damaged",
            LicenseError::BadSignature => "license signature is invalid",
            LicenseError::WrongProduct => "license is for a different product",
            LicenseError::Expired => "license has expired",
            LicenseError::DeviceMismatch => "license belongs to a different R Interface",
        })
    }
}

impl std::error::Error for LicenseError {}

/// Signs `claims` and returns the license file text.
pub fn issue(key: &SigningKey, claims: &Claims) -> String {
    let body = serde_json::to_string(claims).expect("claims serialize");
    let sig = key.sign(body.as_bytes());
    serde_json::to_string_pretty(&Envelope { claims: body, sig: B64.encode(sig.to_bytes()) })
        .expect("envelope serializes")
}

/// Checks signature, product, expiry and (if locked) the connected device.
/// `serials` are the serial numbers of the R Interfaces currently attached.
pub fn verify(
    text: &str,
    public: &VerifyingKey,
    now: u64,
    serials: &[String],
) -> Result<Claims, LicenseError> {
    let env: Envelope = serde_json::from_str(text).map_err(|_| LicenseError::Malformed)?;
    let sig_bytes: [u8; 64] = B64
        .decode(env.sig.as_bytes())
        .map_err(|_| LicenseError::Malformed)?
        .try_into()
        .map_err(|_| LicenseError::Malformed)?;
    public
        .verify(env.claims.as_bytes(), &Signature::from_bytes(&sig_bytes))
        .map_err(|_| LicenseError::BadSignature)?;

    // Only parse claims after the signature has proven they are ours.
    let claims: Claims = serde_json::from_str(&env.claims).map_err(|_| LicenseError::Malformed)?;
    if claims.product != PRODUCT {
        return Err(LicenseError::WrongProduct);
    }
    if claims.expires_at.is_some_and(|e| now >= e) {
        return Err(LicenseError::Expired);
    }
    if let Some(want) = &claims.device_serial {
        if !serials.iter().any(|s| s.eq_ignore_ascii_case(want)) {
            return Err(LicenseError::DeviceMismatch);
        }
    }
    Ok(claims)
}

pub fn public_key_from_hex(hex: &str) -> Option<VerifyingKey> {
    let bytes = hex_decode(hex.trim())?;
    VerifyingKey::from_bytes(&bytes.try_into().ok()?).ok()
}

/// The vendor public key baked in at build time via `AUDIORA_LICENSE_PUBKEY`
/// (64 hex characters). `None` for development builds.
pub fn embedded_public_key() -> Option<VerifyingKey> {
    option_env!("AUDIORA_LICENSE_PUBKEY").and_then(public_key_from_hex)
}

pub fn signing_key_from_hex(hex: &str) -> Option<SigningKey> {
    let bytes: [u8; 32] = hex_decode(hex.trim())?.try_into().ok()?;
    Some(SigningKey::from_bytes(&bytes))
}

pub fn hex_encode(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

pub fn hex_decode(s: &str) -> Option<Vec<u8>> {
    if s.len() % 2 != 0 || !s.is_ascii() {
        return None;
    }
    (0..s.len()).step_by(2).map(|i| u8::from_str_radix(&s[i..i + 2], 16).ok()).collect()
}

pub fn generate_signing_key() -> Result<SigningKey, getrandom::Error> {
    let mut seed = [0u8; 32];
    getrandom::fill(&mut seed)?;
    Ok(SigningKey::from_bytes(&seed))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key() -> SigningKey {
        generate_signing_key().unwrap()
    }

    fn claims() -> Claims {
        Claims {
            product: PRODUCT.into(),
            edition: "studio".into(),
            issued_to: "Test Studio".into(),
            device_serial: None,
            issued_at: 1_000,
            expires_at: Some(2_000),
            features: vec!["multitrack".into()],
        }
    }

    #[test]
    fn valid_license_roundtrips() {
        let k = key();
        let text = issue(&k, &claims());
        let got = verify(&text, &k.verifying_key(), 1_500, &[]).unwrap();
        assert_eq!(got, claims());
    }

    #[test]
    fn tampered_claims_are_rejected() {
        let k = key();
        let text = issue(&k, &claims());
        let forged = text.replace("studio", "studia").replace("2000", "9999999999");
        assert_ne!(forged, text);
        assert_eq!(verify(&forged, &k.verifying_key(), 1_500, &[]), Err(LicenseError::BadSignature));
    }

    #[test]
    fn signature_from_another_key_is_rejected() {
        let text = issue(&key(), &claims());
        assert_eq!(verify(&text, &key().verifying_key(), 1_500, &[]), Err(LicenseError::BadSignature));
    }

    #[test]
    fn expiry_and_perpetual() {
        let k = key();
        let text = issue(&k, &claims());
        assert_eq!(verify(&text, &k.verifying_key(), 2_000, &[]), Err(LicenseError::Expired));
        let forever = issue(&k, &Claims { expires_at: None, ..claims() });
        assert!(verify(&forever, &k.verifying_key(), u64::MAX - 1, &[]).is_ok());
    }

    #[test]
    fn device_lock_requires_the_interface_to_be_present() {
        let k = key();
        let text = issue(&k, &Claims { device_serial: Some("R-0042".into()), ..claims() });
        let p = k.verifying_key();
        assert_eq!(verify(&text, &p, 1_500, &[]), Err(LicenseError::DeviceMismatch));
        assert_eq!(verify(&text, &p, 1_500, &["R-0001".into()]), Err(LicenseError::DeviceMismatch));
        assert!(verify(&text, &p, 1_500, &["r-0042".into()]).is_ok(), "case-insensitive");
    }

    #[test]
    fn wrong_product_and_garbage() {
        let k = key();
        let text = issue(&k, &Claims { product: "other".into(), ..claims() });
        assert_eq!(verify(&text, &k.verifying_key(), 1_500, &[]), Err(LicenseError::WrongProduct));
        for junk in ["", "{}", "not json", r#"{"claims":"x","sig":"!!!"}"#, r#"{"claims":"x","sig":"AAAA"}"#] {
            assert_eq!(verify(junk, &k.verifying_key(), 1, &[]), Err(LicenseError::Malformed), "{junk}");
        }
    }

    #[test]
    fn hex_helpers() {
        let k = key();
        let hex = hex_encode(k.verifying_key().as_bytes());
        assert_eq!(public_key_from_hex(&hex).unwrap(), k.verifying_key());
        assert!(public_key_from_hex("zz").is_none() && public_key_from_hex("abc").is_none());
        let sk = hex_encode(&k.to_bytes());
        assert_eq!(signing_key_from_hex(&sk).unwrap().to_bytes(), k.to_bytes());
    }
}
