//! Language selection and a test that keeps the bundled translations complete and in sync.
//!
//! The UI uses symbolic translation keys (`@tr("op.probe.title")`), so — unlike gettext's
//! English-as-msgid default — there is no automatic fallback: a key missing from the active
//! language renders as the raw key. The `bundled_translations_*` tests below fail the build if any
//! language drifts, which is what makes the symbolic-key model safe.

/// UI languages the app bundles translations for.
pub const SUPPORTED: [&str; 3] = ["en", "ru", "hy"];

/// Pick the initial language from the system locale (e.g. `ru-RU` -> `ru`), falling back to
/// English when the locale is unknown or unsupported.
#[must_use]
pub fn initial_language() -> String {
    let locale = sys_locale::get_locale().unwrap_or_default();
    let lang = locale
        .split(['-', '_'])
        .next()
        .unwrap_or("en")
        .to_ascii_lowercase();
    if SUPPORTED.contains(&lang.as_str()) {
        lang
    } else {
        "en".to_owned()
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    const EN: &str = include_str!("../lang/en/LC_MESSAGES/hdm-am-app.po");
    const RU: &str = include_str!("../lang/ru/LC_MESSAGES/hdm-am-app.po");
    const HY: &str = include_str!("../lang/hy/LC_MESSAGES/hdm-am-app.po");

    fn unescape(value: &str) -> String {
        value.replace("\\\"", "\"").replace("\\\\", "\\")
    }

    /// Parse the single-line `msgid`/`msgstr` pairs our generator emits, skipping the empty-id
    /// header entry.
    fn parse(po: &str) -> BTreeMap<String, String> {
        let mut map = BTreeMap::new();
        let mut lines = po.lines();
        while let Some(line) = lines.next() {
            let Some(id) = line
                .strip_prefix("msgid \"")
                .and_then(|rest| rest.strip_suffix('"'))
            else {
                continue;
            };
            if id.is_empty() {
                continue;
            }
            let Some(value) = lines
                .next()
                .and_then(|next| next.strip_prefix("msgstr \""))
                .and_then(|rest| rest.strip_suffix('"'))
            else {
                panic!("msgid {id} is not followed by a msgstr line");
            };
            map.insert(unescape(id), unescape(value));
        }
        map
    }

    #[test]
    fn bundled_translations_are_complete_and_in_sync() {
        let en = parse(EN);
        let ru = parse(RU);
        let hy = parse(HY);

        assert!(!en.is_empty(), "no translations parsed from en .po");

        let en_keys: Vec<&String> = en.keys().collect();
        assert_eq!(en_keys, ru.keys().collect::<Vec<_>>(), "ru keys differ from en");
        assert_eq!(en_keys, hy.keys().collect::<Vec<_>>(), "hy keys differ from en");

        for (lang, map) in [("en", &en), ("ru", &ru), ("hy", &hy)] {
            for (key, value) in map {
                assert!(
                    !value.trim().is_empty(),
                    "{lang}: empty translation for key {key}"
                );
            }
        }
    }
}
