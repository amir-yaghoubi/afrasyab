//! AES-256-GCM helpers for opaque domain tokens (nonce prepended to ciphertext).

use aes_gcm::aead::rand_core::RngCore;
use aes_gcm::aead::{Aead, KeyInit, OsRng};
use aes_gcm::{Aes256Gcm, Nonce};

const NONCE_LEN: usize = 12;

/// Encrypts and decrypts token payloads using AES-256-GCM.
///
/// Wire format: `[12-byte nonce][ciphertext || 128-bit tag]`.
pub struct TokenCipher {
    cipher: Aes256Gcm,
}

impl TokenCipher {
    pub fn new(key: &[u8; 32]) -> Self {
        Self {
            cipher: Aes256Gcm::new_from_slice(key.as_slice())
                .expect("AES-256-GCM key length is 32 bytes"),
        }
    }

    pub fn encrypt(&self, plaintext: &[u8]) -> Vec<u8> {
        let mut nonce = [0u8; NONCE_LEN];
        OsRng.fill_bytes(&mut nonce);

        let nonce_ga = Nonce::from_slice(&nonce);
        let mut ciphertext = self
            .cipher
            .encrypt(nonce_ga, plaintext)
            .expect("AES-GCM encryption must succeed for valid inputs");

        let mut out = Vec::with_capacity(NONCE_LEN + ciphertext.len());
        out.extend_from_slice(&nonce);
        out.append(&mut ciphertext);
        out
    }

    pub fn decrypt(&self, blob: &[u8]) -> Result<Vec<u8>, crate::error::DomainError> {
        if blob.len() <= NONCE_LEN {
            return Err(crate::error::DomainError::Crypto(
                "encrypted blob too short".into(),
            ));
        }
        let (nonce, ciphertext) = blob.split_at(NONCE_LEN);
        let nonce_ga = Nonce::from_slice(nonce);
        self.cipher
            .decrypt(nonce_ga, ciphertext)
            .map_err(|e| crate::error::DomainError::Crypto(e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip() {
        let key = [42u8; 32];
        let cipher = TokenCipher::new(&key);
        let plaintext = b"session-token-or-similar";

        let blob = cipher.encrypt(plaintext);
        assert!(blob.len() > NONCE_LEN);
        assert_ne!(&blob[..NONCE_LEN], &[0u8; NONCE_LEN]);

        let recovered = cipher.decrypt(&blob).unwrap();
        assert_eq!(recovered, plaintext);
    }
}
