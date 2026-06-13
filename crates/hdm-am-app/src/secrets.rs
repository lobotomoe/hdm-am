//! Keychain-backed secret storage for the HDM password and cashier PIN.
//!
//! Secrets are keyed by `(profile_id, field)` so the draft and each saved favorite keep their own
//! credentials. On Apple platforms they are stored in the data-protection Keychain with the
//! `AfterFirstUnlockThisDeviceOnly` accessibility class — readable while the app runs, but excluded
//! from iCloud Keychain and device-to-device transfer. On non-Apple platforms secret storage is
//! unavailable and these calls degrade to no-ops (the user re-enters the secret each session)
//! rather than persisting anything insecurely.

/// Keychain field name for the HDM device password.
pub const PASSWORD: &str = "password";
/// Keychain field name for the cashier PIN.
pub const PIN: &str = "pin";

/// Store (or replace) a secret for `(profile_id, field)`. Failures are logged, not propagated: a
/// Keychain write failure must not break the user's operation — it just means the secret is not
/// remembered.
pub fn set(profile_id: &str, field: &str, value: &str) {
    platform::set(profile_id, field, value);
}

/// Retrieve a secret. Returns `None` when absent (or on an unexpected Keychain error, which is
/// logged) so the caller simply treats it as "not remembered".
#[must_use]
pub fn get(profile_id: &str, field: &str) -> Option<String> {
    platform::get(profile_id, field)
}

/// Delete a secret. Idempotent: a missing item is success.
pub fn delete(profile_id: &str, field: &str) {
    platform::delete(profile_id, field);
}

#[cfg(target_vendor = "apple")]
mod platform {
    use core_foundation::base::TCFType;
    use core_foundation::boolean::CFBoolean;
    use core_foundation::data::CFData;
    use core_foundation::dictionary::CFDictionary;
    use core_foundation::string::CFString;
    use core_foundation_sys::base::CFTypeRef;
    use core_foundation_sys::string::CFStringRef;
    use security_framework::base::Error;
    use security_framework::passwords::{
        PasswordOptions, delete_generic_password, generic_password,
    };
    use security_framework_sys::access_control::kSecAttrAccessibleAfterFirstUnlockThisDeviceOnly;
    use security_framework_sys::base::{errSecDuplicateItem, errSecItemNotFound, errSecSuccess};
    use security_framework_sys::item::{
        kSecAttrAccount, kSecAttrService, kSecClass, kSecClassGenericPassword,
        kSecUseDataProtectionKeychain, kSecValueData,
    };
    use security_framework_sys::keychain_item::{SecItemAdd, SecItemUpdate};

    const SERVICE: &str = "am.lobotomoe.hdm.credentials";

    // The `kSecAttrAccessible` *key* is not re-exported by security-framework-sys (only the *value*
    // constants are), so declare it here. It links from the same Security.framework.
    unsafe extern "C" {
        static kSecAttrAccessible: CFStringRef;
    }

    fn account(profile_id: &str, field: &str) -> String {
        format!("{profile_id}/{field}")
    }

    pub fn set(profile_id: &str, field: &str, value: &str) {
        if let Err(err) = set_inner(&account(profile_id, field), value) {
            log::warn!("keychain set failed for {field}: {err}");
        }
    }

    // The safe `set_generic_password` cannot set `kSecAttrAccessible`, so it would default to the
    // iCloud-eligible `WhenUnlocked` class. We need device-only, so add/update via SecItem directly.
    fn set_inner(account: &str, value: &str) -> Result<(), Error> {
        unsafe {
            let k_class = CFString::wrap_under_get_rule(kSecClass);
            let v_class = CFString::wrap_under_get_rule(kSecClassGenericPassword);
            let k_service = CFString::wrap_under_get_rule(kSecAttrService);
            let k_account = CFString::wrap_under_get_rule(kSecAttrAccount);
            let k_value = CFString::wrap_under_get_rule(kSecValueData);
            let k_accessible = CFString::wrap_under_get_rule(kSecAttrAccessible);
            let v_accessible =
                CFString::wrap_under_get_rule(kSecAttrAccessibleAfterFirstUnlockThisDeviceOnly);
            let k_dpk = CFString::wrap_under_get_rule(kSecUseDataProtectionKeychain);

            let service = CFString::new(SERVICE);
            let account = CFString::new(account);
            let data = CFData::from_buffer(value.as_bytes());

            let add = CFDictionary::from_CFType_pairs(&[
                (k_class.as_CFType(), v_class.as_CFType()),
                (k_service.as_CFType(), service.as_CFType()),
                (k_account.as_CFType(), account.as_CFType()),
                (k_value.as_CFType(), data.as_CFType()),
                (k_accessible.as_CFType(), v_accessible.as_CFType()),
                (k_dpk.as_CFType(), CFBoolean::true_value().as_CFType()),
            ]);

            let mut out: CFTypeRef = std::ptr::null();
            let status = SecItemAdd(add.as_concrete_TypeRef(), &raw mut out);
            if status == errSecSuccess {
                return Ok(());
            }
            if status != errSecDuplicateItem {
                return Err(Error::from_code(status));
            }

            // Item already exists: update only its value (identity keys go in the query).
            let query = CFDictionary::from_CFType_pairs(&[
                (k_class.as_CFType(), v_class.as_CFType()),
                (k_service.as_CFType(), service.as_CFType()),
                (k_account.as_CFType(), account.as_CFType()),
            ]);
            let update =
                CFDictionary::from_CFType_pairs(&[(k_value.as_CFType(), data.as_CFType())]);
            let status = SecItemUpdate(query.as_concrete_TypeRef(), update.as_concrete_TypeRef());
            if status == errSecSuccess {
                Ok(())
            } else {
                Err(Error::from_code(status))
            }
        }
    }

    pub fn get(profile_id: &str, field: &str) -> Option<String> {
        let account = account(profile_id, field);
        match generic_password(PasswordOptions::new_generic_password(SERVICE, &account)) {
            Ok(bytes) => match String::from_utf8(bytes) {
                Ok(secret) => Some(secret),
                Err(err) => {
                    log::warn!("keychain value for {field} is not valid UTF-8: {err}");
                    None
                }
            },
            Err(err) if err.code() == errSecItemNotFound => None,
            Err(err) => {
                log::warn!("keychain get failed for {field}: {err}");
                None
            }
        }
    }

    pub fn delete(profile_id: &str, field: &str) {
        let account = account(profile_id, field);
        match delete_generic_password(SERVICE, &account) {
            Ok(()) => {}
            Err(err) if err.code() == errSecItemNotFound => {}
            Err(err) => log::warn!("keychain delete failed for {field}: {err}"),
        }
    }
}

#[cfg(not(target_vendor = "apple"))]
mod platform {
    // No secure credential store on these targets, so every call is a logged no-op (the logging
    // also keeps these non-`const`, which is correct — a real store would not be const either).
    pub fn set(_profile_id: &str, _field: &str, _value: &str) {
        log::debug!("secret storage is unavailable on this platform; not persisting");
    }

    pub fn get(_profile_id: &str, _field: &str) -> Option<String> {
        log::debug!("secret storage is unavailable on this platform; nothing to retrieve");
        None
    }

    pub fn delete(_profile_id: &str, _field: &str) {
        log::debug!("secret storage is unavailable on this platform; nothing to delete");
    }
}
