//! YouCraft's local bridge credential. Reuse the owner's existing private helper secret;
//! domain separation keeps the control credential distinct from the HTTP API credential.
#[cfg(any(windows, test))]
use sha2::{Digest, Sha256};

pub fn token() -> Option<String> {
    if let Ok(token) = std::env::var("FILMCRAFT_CONTROL_TOKEN") {
        return (token.len() == 64 && token.bytes().all(|b| b.is_ascii_hexdigit())).then_some(token);
    }
    #[cfg(windows)]
    {
        let root = std::env::var_os("BETUN_DATA_DIR")
            .map(std::path::PathBuf::from)
            .or_else(|| std::env::var_os("LOCALAPPDATA").map(|r| std::path::PathBuf::from(r).join("Betun")))?;
        let secret = std::fs::read_to_string(root.join("api-token")).ok()?;
        let secret = secret.trim();
        if secret.len() < 32 {
            return None;
        }
        Some(derive(secret))
    }
    #[cfg(not(windows))]
    {
        None
    }
}
#[cfg(any(windows, test))]
fn derive(secret: &str) -> String {
    let mut digest = Sha256::new();
    digest.update(b"youcraft-control-v1\0");
    digest.update(secret.as_bytes());
    format!("{:x}", digest.finalize())
}
#[cfg(test)]
mod tests {
    #[test]
    fn credential_is_separate_and_stable() {
        let secret = "synthetic-local-secret-that-is-long-enough";
        let token = super::derive(secret);
        assert_eq!(token.len(), 64);
        assert!(token.bytes().all(|b| b.is_ascii_hexdigit()));
        assert_eq!(token, super::derive(secret));
        assert_ne!(token, super::derive("a-different-synthetic-local-secret-that-is-long-enough"));
        assert!(!token.contains(secret));
    }
}
