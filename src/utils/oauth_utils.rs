use rand::{TryRng, rngs::SysRng};
use sha2::{Digest, Sha256};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};

/// A PKCE code verifier and its derived code challenge (S256 method).
pub struct PkceChallenge {
    /// The secret random string — keep this; send it to /token.
    pub code_verifier: String,
    /// The SHA-256 derived challenge — send this to /authorize.
    pub code_challenge: String,
    /// Always "S256" — the only method you should use.
    pub code_challenge_method: &'static str,
}

impl PkceChallenge {
    /// Generate a fresh PKCE pair.
    pub fn new() -> Self {
        let verifier = generate_verifier();
        let challenge = derive_challenge(&verifier);
        Self {
            code_verifier: verifier,
            code_challenge: challenge,
            code_challenge_method: "S256",
        }
    }
    /// Verify that a given verifier matches this challenge.
    ///
    /// for testing
    pub fn verify(&self, verifier: &str) -> bool {
        derive_challenge(verifier) == self.code_challenge
    }
}

/// Generate a 32-byte random verifier, base64url-encoded (no padding).
///
/// 32 bytes → 256 bits of entropy, well above the RFC minimum.
fn generate_verifier() -> String {
    let mut bytes = [0u8; 32];
    SysRng.try_fill_bytes(&mut bytes).unwrap();
    URL_SAFE_NO_PAD.encode(&bytes)
}

/// Derive the S256 code challenge from a verifier.
///
/// challenge = BASE64URL(SHA256(ASCII(verifier)))
fn derive_challenge(verifier: &str) -> String {
    let digest = Sha256::digest(verifier.as_bytes());
    URL_SAFE_NO_PAD.encode(digest)    
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verifier_length_is_valid() {
        // RFC 7636 §4.1: 43–128 unreserved ASCII chars.
        let pkce = PkceChallenge::new();
        let len = pkce.code_verifier.len();
        assert!(len >= 43 && len <= 128, "verifier length {len} out of range");
    }

    #[test]
    fn verifier_charset_is_unreserved() {
        // Only A-Z a-z 0-9 - _ . ~ are unreserved. We use - and _.
        let pkce = PkceChallenge::new();
        assert!(
            pkce.code_verifier.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_'),
            "verifier contains reserved characters"
        );
    }

    #[test]
    fn challenge_is_derived_from_verifier() {
        let pkce = PkceChallenge::new();
        assert!(pkce.verify(&pkce.code_verifier));
    }

    #[test]
    fn wrong_verifier_fails_verification() {
        let pkce = PkceChallenge::new();
        assert!(!pkce.verify("not-the-right-verifier"));
    }

    #[test]
    fn method_is_s256() {
        let pkce = PkceChallenge::new();
        assert_eq!(pkce.code_challenge_method, "S256");
    }

    /// RFC 7636 Appendix B — known-answer test.
    /// verifier = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk"
    /// expected challenge = "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM"
    #[test]
    fn rfc_known_answer() {
        let verifier = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk";
        let challenge = derive_challenge(verifier);
        assert_eq!(challenge, "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM");
    }
}