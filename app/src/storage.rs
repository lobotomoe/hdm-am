//! On-device persistence of connection settings and saved favorites.
//!
//! Non-secret fields (host, port, operator id, …) are written to a small versioned JSON file in the
//! app's data directory. Secrets (HDM password, cashier PIN) are never written here — they live in
//! the Keychain (see [`crate::secrets`]), keyed by the profile id.
//!
//! Two kinds of records are kept: a single always-restored `draft` (the working set the user last
//! had on screen) and a list of named `favorites` the user can switch between. The split means
//! editing the form never silently mutates a saved favorite.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

const SCHEMA_VERSION: u32 = 1;

/// Reserved Keychain profile id for the working draft's secrets.
pub const DRAFT_ID: &str = "__draft__";

/// Connection-specific fields shared by the draft and every saved profile.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Connection {
    pub host: String,
    pub port: String,
    pub timeout_seconds: String,
    pub cashier: String,
    pub department: String,
}

/// A named, saved connection. Secrets live in the Keychain keyed by `id`.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Profile {
    pub id: String,
    pub name: String,
    #[serde(flatten)]
    pub connection: Connection,
}

/// Persisted application state. `#[serde(default)]` keeps an older or partially written store
/// loadable when new fields are added later, rather than discarding the user's saved data.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Store {
    pub version: u32,
    pub language: String,
    pub advanced: bool,
    pub draft: Connection,
    pub favorites: Vec<Profile>,
    /// Id of the currently selected favorite, or "" when the draft is unsaved/standalone.
    pub selected: String,
}

impl Default for Store {
    fn default() -> Self {
        Self {
            version: SCHEMA_VERSION,
            language: String::new(),
            advanced: false,
            draft: Connection::default(),
            favorites: Vec::new(),
            selected: String::new(),
        }
    }
}

impl Store {
    /// Parse a store from `path`. A missing file yields the default; a corrupt file is preserved as
    /// `<path>.corrupt` (rather than silently discarded) and the default is returned so the app
    /// still starts.
    fn load_from(path: &Path) -> Self {
        let bytes = match fs::read(path) {
            Ok(bytes) => bytes,
            Err(err) if err.kind() == io::ErrorKind::NotFound => return Self::default(),
            Err(err) => {
                log::warn!("could not read settings store {}: {err}", path.display());
                return Self::default();
            }
        };
        match serde_json::from_slice::<Self>(&bytes) {
            Ok(store) => store,
            Err(err) => {
                log::warn!(
                    "settings store {} is corrupt ({err}); starting fresh",
                    path.display()
                );
                let backup = path.with_extension("corrupt");
                if let Err(err) = fs::rename(path, &backup) {
                    log::warn!("could not preserve corrupt store: {err}");
                }
                Self::default()
            }
        }
    }

    /// Atomically write the store to `path` (temp file + rename) so a crash mid-write cannot leave a
    /// half-written, unparseable file.
    fn save_to(&self, path: &Path) -> io::Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_vec_pretty(self).map_err(io::Error::other)?;
        let tmp = path.with_extension("tmp");
        fs::write(&tmp, &json)?;
        fs::rename(&tmp, path)
    }
}

/// Location of the settings store, or `None` when no per-user data directory is available
/// (persistence is then disabled rather than guessed).
fn store_path() -> Option<PathBuf> {
    data_dir().map(|dir| dir.join("store.json"))
}

#[cfg(target_vendor = "apple")]
fn data_dir() -> Option<PathBuf> {
    // iOS: HOME is the app sandbox container. macOS: the user's home. Both follow the Apple
    // convention of ~/Library/Application Support/<bundle>.
    let home = std::env::var_os("HOME")?;
    Some(PathBuf::from(home).join("Library/Application Support/am.lobotomoe.hdm"))
}

#[cfg(not(target_vendor = "apple"))]
fn data_dir() -> Option<PathBuf> {
    // Android and other targets: best-effort via HOME; persistence is disabled if it is unset.
    let home = std::env::var_os("HOME")?;
    Some(PathBuf::from(home).join(".local/share/hdm-am"))
}

/// Load the persisted store, or the default if none exists / persistence is unavailable.
pub fn load() -> Store {
    store_path().map_or_else(Store::default, |path| Store::load_from(&path))
}

/// Persist the store. Failures are logged, not propagated: a settings-save failure must never break
/// the user's actual fiscal operation.
pub fn save(store: &Store) {
    let Some(path) = store_path() else {
        log::debug!("no data directory; settings are not persisted on this platform");
        return;
    };
    if let Err(err) = store.save_to(&path) {
        log::warn!("could not save settings store: {err}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_path(tag: &str) -> PathBuf {
        std::env::temp_dir()
            .join(format!("hdm-store-test-{}-{tag}", std::process::id()))
            .join("store.json")
    }

    #[test]
    fn roundtrips_through_disk() {
        let path = temp_path("roundtrip");
        let draft = Connection {
            host: "192.168.1.5".to_owned(),
            port: "1025".to_owned(),
            timeout_seconds: "50".to_owned(),
            cashier: "3".to_owned(),
            department: "1".to_owned(),
        };
        let store = Store {
            language: "ru".to_owned(),
            advanced: true,
            draft: draft.clone(),
            favorites: vec![Profile {
                id: "p1".to_owned(),
                name: "Shop".to_owned(),
                connection: draft,
            }],
            selected: "p1".to_owned(),
            ..Store::default()
        };

        assert!(store.save_to(&path).is_ok());
        let loaded = Store::load_from(&path);
        assert_eq!(loaded, store);

        if let Some(dir) = path.parent() {
            let _ = fs::remove_dir_all(dir);
        }
    }

    #[test]
    fn flattened_profile_json_is_flat() {
        let profile = Profile {
            id: "p1".to_owned(),
            name: "Shop".to_owned(),
            connection: Connection {
                host: "10.0.0.1".to_owned(),
                ..Connection::default()
            },
        };
        let json = serde_json::to_string(&profile).unwrap_or_default();
        // `connection` is flattened: host sits at the top level, not nested.
        assert!(json.contains("\"host\":\"10.0.0.1\""), "got {json}");
        assert!(!json.contains("\"connection\""), "got {json}");
    }

    #[test]
    fn missing_file_is_default() {
        let path = std::env::temp_dir().join("hdm-definitely-absent-9f3/store.json");
        assert_eq!(Store::load_from(&path), Store::default());
    }

    #[test]
    fn partial_json_loads_with_defaults() {
        // A store written by a future/older version that omits fields must still load (serde
        // defaults fill the gaps) rather than being discarded.
        let store: Store = serde_json::from_str(r#"{"language":"hy"}"#).unwrap_or_default();
        assert_eq!(store.language, "hy");
        assert!(!store.advanced);
        assert!(store.favorites.is_empty());
    }
}
