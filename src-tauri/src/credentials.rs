//! App-owned credentials only. Never enumerates/imports/deletes another application's entries.
use anyhow::{bail, Result};
use opentoken_core::{connections::CredentialStore, model::hash, storage::Storage};

pub struct Vault {
    prefix: String,
}
impl Vault {
    pub fn new(storage: &Storage) -> Self {
        let path = std::fs::canonicalize(&storage.dir).unwrap_or_else(|_| storage.dir.clone());
        Self {
            prefix: format!(
                "OpenToken/{}/",
                hash(&opentoken_core::storage::path_key(&path))
            ),
        }
    }
    fn target(&self, id: &str) -> Result<String> {
        if id.is_empty()
            || id.len() > 80
            || !id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
        {
            bail!("Invalid credential identifier.");
        }
        Ok(format!("{}{id}", self.prefix))
    }
}
#[cfg(windows)]
mod native {
    use super::*;
    use windows::{
        core::{HRESULT, PCWSTR, PWSTR},
        Win32::{Foundation::ERROR_NOT_FOUND, Security::Credentials::*},
    };
    struct Credential(*mut CREDENTIALW);
    impl Drop for Credential {
        fn drop(&mut self) {
            unsafe {
                if !self.0.is_null() {
                    let c = &*self.0;
                    if !c.CredentialBlob.is_null() {
                        std::ptr::write_bytes(c.CredentialBlob, 0, c.CredentialBlobSize as usize);
                    }
                    CredFree(self.0.cast());
                }
            }
        }
    }
    fn missing(e: &windows::core::Error) -> bool {
        e.code() == HRESULT::from_win32(ERROR_NOT_FOUND.0)
    }
    impl CredentialStore for Vault {
        fn read(&self, id: &str) -> Result<Option<String>> {
            let name: Vec<u16> = self.target(id)?.encode_utf16().chain(Some(0)).collect();
            let mut ptr = std::ptr::null_mut();
            match unsafe { CredReadW(PCWSTR(name.as_ptr()), CRED_TYPE_GENERIC, None, &mut ptr) } {
                Ok(()) => {}
                Err(e) if missing(&e) => return Ok(None),
                Err(_) => bail!("Windows could not read the OpenToken credential."),
            }
            let entry = Credential(ptr);
            let c = unsafe { &*entry.0 };
            if c.CredentialBlobSize == 0
                || c.CredentialBlobSize > 2048
                || c.CredentialBlob.is_null()
            {
                bail!("The saved OpenToken credential is invalid. Reconnect this provider.");
            }
            let bytes = unsafe {
                std::slice::from_raw_parts(c.CredentialBlob, c.CredentialBlobSize as usize)
            };
            let value = std::str::from_utf8(bytes)
                .map_err(|_| anyhow::anyhow!("The saved OpenToken credential is invalid."))?;
            Ok(Some(value.to_owned()))
        }
        fn write(&self, id: &str, secret: &str) -> Result<()> {
            if secret.is_empty() || secret.len() > 2048 {
                bail!("Invalid credential length.");
            }
            let mut name: Vec<u16> = self.target(id)?.encode_utf16().chain(Some(0)).collect();
            let mut user: Vec<u16> = "OpenToken".encode_utf16().chain(Some(0)).collect();
            let mut bytes = secret.as_bytes().to_vec();
            let entry = CREDENTIALW {
                Type: CRED_TYPE_GENERIC,
                TargetName: PWSTR(name.as_mut_ptr()),
                CredentialBlobSize: bytes.len() as u32,
                CredentialBlob: bytes.as_mut_ptr(),
                Persist: CRED_PERSIST_LOCAL_MACHINE,
                UserName: PWSTR(user.as_mut_ptr()),
                ..Default::default()
            };
            let result = unsafe { CredWriteW(&entry, 0) };
            bytes.fill(0);
            result.map_err(|_| {
                anyhow::anyhow!("Windows could not securely save the OpenToken credential.")
            })
        }
        fn delete(&self, id: &str) -> Result<()> {
            let name: Vec<u16> = self.target(id)?.encode_utf16().chain(Some(0)).collect();
            match unsafe { CredDeleteW(PCWSTR(name.as_ptr()), CRED_TYPE_GENERIC, None) } {
                Ok(()) => Ok(()),
                Err(e) if missing(&e) => Ok(()),
                Err(_) => bail!("Windows could not remove the OpenToken credential."),
            }
        }
    }
}
#[cfg(not(windows))]
impl CredentialStore for Vault {
    fn read(&self, _: &str) -> Result<Option<String>> {
        bail!("Windows secure storage is required.")
    }
    fn write(&self, _: &str, _: &str) -> Result<()> {
        bail!("Windows secure storage is required.")
    }
    fn delete(&self, _: &str) -> Result<()> {
        bail!("Windows secure storage is required.")
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_unsafe_or_empty_credential_ids() {
        let vault = Vault {
            prefix: "OpenToken/test/".into(),
        };
        for id in ["", "../other", "other/key", "a\0b"] {
            assert!(vault.target(id).is_err());
        }
    }
    #[cfg(windows)]
    #[test]
    fn windows_vault_round_trip_is_namespaced_and_removable() {
        // A synthetic entry under a unique test-only prefix, removed even on panic.
        let prefix = format!(
            "OpenToken/test-{}-{}/",
            std::process::id(),
            chrono::Utc::now().timestamp_nanos_opt().unwrap()
        );
        let vault = Vault {
            prefix: prefix.clone(),
        };
        struct Cleanup(Vault);
        impl Drop for Cleanup {
            fn drop(&mut self) {
                let _ = self.0.delete("fixture");
            }
        }
        let _cleanup = Cleanup(Vault { prefix });
        assert!(vault.read("fixture").unwrap().is_none());
        vault.write("fixture", "synthetic-credential-one").unwrap();
        assert_eq!(
            vault.read("fixture").unwrap().as_deref(),
            Some("synthetic-credential-one")
        );
        let other = Vault {
            prefix: "OpenToken/test-other-namespace/".into(),
        };
        assert!(other.read("fixture").unwrap().is_none());
        vault.write("fixture", "synthetic-credential-two").unwrap();
        assert_eq!(
            vault.read("fixture").unwrap().as_deref(),
            Some("synthetic-credential-two")
        );
        vault.delete("fixture").unwrap();
        vault.delete("fixture").unwrap();
        assert!(vault.read("fixture").unwrap().is_none());
    }
}
