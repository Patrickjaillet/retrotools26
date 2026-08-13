use crate::error::{AppError, AppResult};

/// Encrypts `plaintext` for storage on disk, returning a base64 string safe
/// to embed in a TOML config file. On Windows this uses DPAPI
/// (`CryptProtectData`, user scope): the OS derives the encryption key from
/// the current Windows login, so the ciphertext is unreadable by another
/// Windows account or if the config file is copied to another machine —
/// real OS-backed protection, not a locally-stored key sitting next to the
/// data it protects. There is no cross-platform equivalent implemented
/// here (the project targets Windows only, per `ROADMAP.md`).
pub fn encrypt_to_base64(plaintext: &str) -> AppResult<String> {
    let bytes = platform::protect(plaintext.as_bytes())?;
    Ok(base64::Engine::encode(&base64::engine::general_purpose::STANDARD, bytes))
}

/// Reverses [`encrypt_to_base64`]. Fails if the blob was encrypted under a
/// different Windows account (or on a different machine) — that's DPAPI
/// working as intended, not a bug.
pub fn decrypt_from_base64(encoded: &str) -> AppResult<String> {
    let bytes = base64::Engine::decode(&base64::engine::general_purpose::STANDARD, encoded)
        .map_err(|e| AppError::Config(format!("malformed encrypted value: {e}")))?;
    let plain = platform::unprotect(&bytes)?;
    String::from_utf8(plain).map_err(|e| AppError::Config(format!("decrypted value is not valid UTF-8: {e}")))
}

#[cfg(windows)]
mod platform {
    use super::AppResult;
    use crate::error::AppError;
    use windows_sys::Win32::Foundation::LocalFree;
    use windows_sys::Win32::Security::Cryptography::{CryptProtectData, CryptUnprotectData, CRYPT_INTEGER_BLOB};

    fn blob(data: &[u8]) -> CRYPT_INTEGER_BLOB {
        CRYPT_INTEGER_BLOB {
            cbData: data.len() as u32,
            pbData: data.as_ptr() as *mut u8,
        }
    }

    pub fn protect(data: &[u8]) -> AppResult<Vec<u8>> {
        unsafe {
            let input = blob(data);
            let mut output: CRYPT_INTEGER_BLOB = std::mem::zeroed();
            let ok = CryptProtectData(
                &input,
                std::ptr::null(),
                std::ptr::null(),
                std::ptr::null_mut(),
                std::ptr::null(),
                0,
                &mut output,
            );
            if ok == 0 {
                return Err(AppError::Config("CryptProtectData failed".into()));
            }
            let result = std::slice::from_raw_parts(output.pbData, output.cbData as usize).to_vec();
            LocalFree(output.pbData as *mut core::ffi::c_void);
            Ok(result)
        }
    }

    pub fn unprotect(data: &[u8]) -> AppResult<Vec<u8>> {
        unsafe {
            let input = blob(data);
            let mut output: CRYPT_INTEGER_BLOB = std::mem::zeroed();
            let ok = CryptUnprotectData(
                &input,
                std::ptr::null_mut(),
                std::ptr::null(),
                std::ptr::null_mut(),
                std::ptr::null(),
                0,
                &mut output,
            );
            if ok == 0 {
                return Err(AppError::Config(
                    "CryptUnprotectData failed (wrong Windows account, or the value wasn't encrypted by this app)".into(),
                ));
            }
            let result = std::slice::from_raw_parts(output.pbData, output.cbData as usize).to_vec();
            LocalFree(output.pbData as *mut core::ffi::c_void);
            Ok(result)
        }
    }
}

#[cfg(not(windows))]
mod platform {
    use super::AppResult;
    use crate::error::AppError;

    pub fn protect(_data: &[u8]) -> AppResult<Vec<u8>> {
        Err(AppError::Config("encrypted credential storage is only implemented on Windows".into()))
    }

    pub fn unprotect(_data: &[u8]) -> AppResult<Vec<u8>> {
        Err(AppError::Config("encrypted credential storage is only implemented on Windows".into()))
    }
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;

    #[test]
    fn round_trips_a_secret_through_dpapi() {
        let secret = "hunter2-très-sécurisé";
        let encrypted = encrypt_to_base64(secret).unwrap();
        assert_ne!(encrypted, secret, "ciphertext must not equal the plaintext");
        assert!(!encrypted.contains("hunter2"), "plaintext must not leak into the stored blob");
        let decrypted = decrypt_from_base64(&encrypted).unwrap();
        assert_eq!(decrypted, secret);
    }

    #[test]
    fn rejects_a_corrupted_blob() {
        let secret = "another-secret";
        let mut encrypted = encrypt_to_base64(secret).unwrap();
        encrypted.push_str("not-valid-base64!!!");
        assert!(decrypt_from_base64(&encrypted).is_err());
    }

    #[test]
    fn empty_string_round_trips() {
        let encrypted = encrypt_to_base64("").unwrap();
        assert_eq!(decrypt_from_base64(&encrypted).unwrap(), "");
    }
}
