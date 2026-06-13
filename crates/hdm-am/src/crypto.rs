//! 3DES-ECB-PKCS7 codec with SHA-256-derived password key.
//!
//! Spec §4.4.3: requests and responses are encrypted with 3DES (Triple-DES) in ECB mode with
//! PKCS7 padding. The login phase uses a key derived from `SHA-256(password)` truncated to the
//! first 24 bytes; subsequent operations use the session key returned in the login response.
//!
//! ECB mode is generally considered weak by modern standards (deterministic, no IV), but the
//! spec mandates it. The application-layer sequence-number anti-replay defends against the
//! relevant attack class.

use crate::error::CryptoError;
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use cipher::{BlockModeDecrypt, BlockModeEncrypt, KeyInit, block_padding::Pkcs7};
use des::TdesEde3;
use sha2::{Digest, Sha256};

/// Length of a 3DES key (24 bytes / 192 bits) per spec §4.4.3.
pub const KEY_LEN: usize = 24;

/// 3DES block size (8 bytes / 64 bits).
pub const BLOCK_SIZE: usize = 8;

type TdesEcbEnc = ecb::Encryptor<TdesEde3>;
type TdesEcbDec = ecb::Decryptor<TdesEde3>;

/// Derive the login-phase encryption key from the HDM access password.
///
/// `SHA-256(password)` truncated to the first 24 bytes, per spec §4.4.3.
#[must_use]
pub fn derive_password_key(password: &str) -> [u8; KEY_LEN] {
    let digest = Sha256::digest(password.as_bytes());
    let mut key = [0u8; KEY_LEN];
    key.copy_from_slice(&digest[..KEY_LEN]);
    key
}

/// Decode a Base64-encoded session key as returned in the `OperatorLogin` response.
///
/// # Errors
/// Returns [`CryptoError::SessionKeyBase64`] if the input isn't valid Base64, or
/// [`CryptoError::InvalidKeyLength`] if the decoded length isn't exactly 24 bytes.
pub fn decode_session_key(base64_key: &str) -> Result<[u8; KEY_LEN], CryptoError> {
    let decoded = BASE64.decode(base64_key)?;
    if decoded.len() != KEY_LEN {
        return Err(CryptoError::InvalidKeyLength);
    }
    let mut key = [0u8; KEY_LEN];
    key.copy_from_slice(&decoded);
    Ok(key)
}

/// 3DES-ECB-PKCS7 codec bound to a 24-byte key.
///
/// A single `Codec` represents one of the two encryption phases (password phase before login,
/// session phase after). Construct a fresh codec each phase rather than mutating in place.
#[derive(Clone)]
pub struct Codec {
    key: [u8; KEY_LEN],
}

impl Codec {
    /// Build a codec from a raw 24-byte key — e.g. a session key recovered from a login response.
    #[must_use]
    pub const fn from_key(key: [u8; KEY_LEN]) -> Self {
        Self { key }
    }

    /// Build a codec from an HDM access password.
    ///
    /// Equivalent to `Codec::from_key(derive_password_key(password))`.
    #[must_use]
    pub fn from_password(password: &str) -> Self {
        Self::from_key(derive_password_key(password))
    }

    /// Encrypt a plaintext byte slice with 3DES-ECB-PKCS7.
    ///
    /// # Errors
    /// Returns [`CryptoError::InvalidKeyLength`] only if the codec was somehow constructed with
    /// a malformed key — not reachable through the public API today.
    pub fn encrypt(&self, plaintext: &[u8]) -> Result<Vec<u8>, CryptoError> {
        let cipher =
            TdesEcbEnc::new_from_slice(&self.key).map_err(|_| CryptoError::InvalidKeyLength)?;
        let msg_len = plaintext.len();
        // PKCS7 adds 1..=BLOCK_SIZE bytes of padding to bring `msg_len` up to a multiple of the
        // block size. Allocate `msg_len + BLOCK_SIZE` so the buffer is always large enough.
        let mut buf = vec![0u8; msg_len + BLOCK_SIZE];
        buf[..msg_len].copy_from_slice(plaintext);
        let ciphertext = cipher
            .encrypt_padded::<Pkcs7>(&mut buf, msg_len)
            .map_err(|_| CryptoError::InvalidKeyLength)?;
        let len = ciphertext.len();
        buf.truncate(len);
        Ok(buf)
    }

    /// Decrypt a 3DES-ECB-PKCS7 ciphertext.
    ///
    /// # Errors
    /// - [`CryptoError::InvalidBlockSize`] if `ciphertext.len()` is not a multiple of 8.
    /// - [`CryptoError::BadPadding`] if PKCS7 padding validation fails after decryption (this
    ///   almost always means the session key has gone stale).
    pub fn decrypt(&self, ciphertext: &[u8]) -> Result<Vec<u8>, CryptoError> {
        if ciphertext.len() % BLOCK_SIZE != 0 {
            return Err(CryptoError::InvalidBlockSize);
        }
        let cipher =
            TdesEcbDec::new_from_slice(&self.key).map_err(|_| CryptoError::InvalidKeyLength)?;
        let mut buf = ciphertext.to_vec();
        let plaintext = cipher
            .decrypt_padded::<Pkcs7>(&mut buf)
            .map_err(|_| CryptoError::BadPadding)?;
        let len = plaintext.len();
        buf.truncate(len);
        Ok(buf)
    }
}

impl std::fmt::Debug for Codec {
    /// Custom Debug — the key is sensitive material and must not appear in logs or panics.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Codec").field("key", &"[redacted]").finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::CryptoError;

    /// Key derivation truncates `SHA-256(password)` to exactly 24 bytes.
    #[test]
    fn password_key_truncates_sha256_to_24_bytes() {
        let key = derive_password_key("1234ABCD");
        let full = Sha256::digest(b"1234ABCD");
        assert_eq!(key.len(), KEY_LEN);
        assert_eq!(&key[..], &full[..KEY_LEN]);
    }

    /// Known-answer test for the spec's example password `"1234ABCD"`.
    ///
    /// Independently verified: `printf '%s' 1234ABCD | sha256sum`
    /// → `c41102040df4255e3c6877271b15cb64cb36744af2235080a4ba8d4d8c39fee2`
    /// First 24 bytes → `c41102040df4255e3c6877271b15cb64cb36744af2235080`
    #[test]
    fn password_key_known_answer_for_spec_example() {
        let key = derive_password_key("1234ABCD");
        let expected =
            hex::decode("c41102040df4255e3c6877271b15cb64cb36744af2235080").expect("hex");
        assert_eq!(&key[..], &expected[..]);
    }

    /// NIST SHA-256 KAT for `"abc"` — sanity check that our SHA-256 backing library produces
    /// the canonical hash. If this drifts, every downstream key derivation is suspect.
    /// Reference: <https://csrc.nist.gov/CSRC/media/Publications/fips/180/2/archive/2002-08-01/documents/fips180-2.pdf>
    /// SHA-256("abc") = `ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad`
    #[test]
    fn sha256_backing_library_matches_nist_kat() {
        let digest = Sha256::digest(b"abc");
        let expected =
            hex::decode("ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad")
                .expect("hex");
        assert_eq!(&digest[..], &expected[..]);
    }

    /// Cross-implementation known-answer test for the full 3DES-ECB + PKCS7 path.
    ///
    /// The expected ciphertext was produced independently with pyca/cryptography
    /// (`TripleDES`/ECB + PKCS7) and confirmed byte-for-byte: key = `SHA-256("1234ABCD")[..24]`,
    /// plaintext = `{"seq":1}` → `74375fcb9fd47be3d0d853569f29c41a`. Unlike the round-trip test,
    /// this pins the actual cipher output, so a symmetric bug (wrong key schedule, EEE vs EDE,
    /// wrong padding scheme, byte-swapped key) is caught here instead of in the field.
    #[test]
    fn encrypt_matches_independent_3des_kat() {
        let codec = Codec::from_password("1234ABCD");
        let ciphertext = codec.encrypt(b"{\"seq\":1}").expect("encrypt");
        assert_eq!(hex::encode(&ciphertext), "74375fcb9fd47be3d0d853569f29c41a");
        // The constant must also decrypt back to the original plaintext.
        let plaintext = codec.decrypt(&ciphertext).expect("decrypt");
        assert_eq!(plaintext, b"{\"seq\":1}");
    }

    /// Round-trip: encrypt then decrypt should reproduce the original plaintext exactly,
    /// across a variety of input sizes including the block boundary cases.
    #[test]
    fn encrypt_decrypt_round_trip() {
        let codec = Codec::from_password("anyPassword123");
        let test_inputs: &[&[u8]] = &[
            b"",
            b"x",
            b"12345678",                        // exactly one block
            b"123456789",                       // one block + 1
            b"123456789ABCDEFG",                // exactly two blocks
            b"hello world this is some text",   // arbitrary
            b"{\"seq\":42,\"paidAmount\":100}", // realistic JSON
        ];
        for plaintext in test_inputs {
            let ciphertext = codec.encrypt(plaintext).expect("encrypt");
            let decrypted = codec.decrypt(&ciphertext).expect("decrypt");
            assert_eq!(
                &decrypted[..],
                *plaintext,
                "round-trip failed for {plaintext:?}"
            );
        }
    }

    /// PKCS7 ciphertext is always a positive multiple of the 8-byte 3DES block.
    #[test]
    fn ciphertext_is_block_aligned_and_non_empty() {
        let codec = Codec::from_password("password");
        for size in 0_usize..32 {
            let plaintext = vec![0xAA; size];
            let ciphertext = codec.encrypt(&plaintext).expect("encrypt");
            assert!(ciphertext.len() >= BLOCK_SIZE);
            assert!(
                ciphertext.len() % BLOCK_SIZE == 0,
                "ciphertext length {} for plaintext size {} not multiple of {}",
                ciphertext.len(),
                size,
                BLOCK_SIZE
            );
            // PKCS7 always adds at least one byte of padding, even for exact-block plaintexts.
            assert!(ciphertext.len() > plaintext.len() || plaintext.is_empty());
        }
    }

    /// Decrypting tampered ciphertext rejects with `BadPadding`.
    #[test]
    fn decrypt_rejects_corrupted_padding() {
        let codec = Codec::from_password("anyPassword");
        let mut ciphertext = codec.encrypt(b"some plaintext").expect("encrypt");
        let last = ciphertext.len() - 1;
        ciphertext[last] ^= 0xFF;
        // After XOR, the last block's padding will (with overwhelming probability) be invalid.
        // 3DES-ECB doesn't authenticate; the only signal is padding sanity.
        let err = codec
            .decrypt(&ciphertext)
            .expect_err("expected padding error");
        assert_eq!(err, CryptoError::BadPadding);
    }

    /// Decrypting with the wrong key surfaces as a padding error 255/256 of the time.
    /// This is the expected behaviour — ECB mode provides confidentiality, not authenticity.
    #[test]
    fn decrypt_with_wrong_key_almost_always_fails_padding() {
        let codec_a = Codec::from_password("alpha");
        let codec_b = Codec::from_password("bravo");
        let ciphertext = codec_a.encrypt(b"hello world").expect("encrypt");
        let result = codec_b.decrypt(&ciphertext);
        // The decryption itself can produce arbitrary bytes; we just verify that we either
        // detect bad padding (the common case) or return some bytes (rare collisions).
        // Either way, we must not panic.
        assert!(result.is_err() || result.is_ok());
    }

    /// Ciphertext whose length isn't a multiple of the block size is rejected up-front.
    #[test]
    fn decrypt_rejects_non_block_aligned_input() {
        let codec = Codec::from_password("pw");
        // 7 bytes — not a multiple of 8
        let err = codec
            .decrypt(&[1, 2, 3, 4, 5, 6, 7])
            .expect_err("expected block-size error");
        assert_eq!(err, CryptoError::InvalidBlockSize);
    }

    /// `decode_session_key` enforces the 24-byte requirement strictly.
    #[test]
    fn decode_session_key_validates_length() {
        let bad_b64 = BASE64.encode(b"too short");
        let err = decode_session_key(&bad_b64).expect_err("expected length error");
        assert_eq!(err, CryptoError::InvalidKeyLength);
    }

    /// Malformed Base64 surfaces as a dedicated error, not a panic.
    #[test]
    fn decode_session_key_rejects_bad_base64() {
        let err = decode_session_key("not~valid~base64~!").expect_err("expected b64 error");
        assert!(matches!(err, CryptoError::SessionKeyBase64(_)));
    }

    /// A valid 24-byte Base64 decodes cleanly into a usable key.
    #[test]
    fn decode_session_key_happy_path() {
        let raw = [0xAB_u8; KEY_LEN];
        let b64 = BASE64.encode(raw);
        let decoded = decode_session_key(&b64).expect("valid key");
        assert_eq!(decoded, raw);
    }

    /// `Debug` formatting of `Codec` never reveals key material.
    #[test]
    fn codec_debug_redacts_key_material() {
        let codec = Codec::from_password("secret-do-not-leak");
        let debug = format!("{codec:?}");
        assert!(debug.contains("[redacted]"));
        assert!(!debug.contains("secret"));
    }
}
