//! Translation lookup.
//!
//! Desktop builds use the system gettext through `gettext-rs`. Android has no
//! libintl, so the Android build reads the `.mo` files packaged in the app
//! assets with the small reader below instead of forking gettext.

#[cfg(not(target_os = "android"))]
pub(crate) use gettextrs::{gettext, pgettext};

#[cfg(not(target_os = "android"))]
pub(crate) fn setup(locale_dir: &std::path::Path) -> anyhow::Result<()> {
    // # Safety
    //
    // setlocale() is called as early as possible, before starting threads or installing UNIX signals.
    unsafe { gettextrs::setlocale(gettextrs::LocaleCategory::LcAll, "") };
    gettextrs::bindtextdomain(crate::config::GETTEXT_PACKAGE, locale_dir)?;
    gettextrs::bind_textdomain_codeset(crate::config::GETTEXT_PACKAGE, "UTF-8")?;
    gettextrs::textdomain(crate::config::GETTEXT_PACKAGE)?;
    Ok(())
}

#[cfg(any(target_os = "android", test))]
#[cfg_attr(not(target_os = "android"), allow(dead_code))]
mod catalog {
    use std::collections::HashMap;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::sync::OnceLock;

    static CATALOG: OnceLock<Option<MoCatalog>> = OnceLock::new();

    pub(crate) fn setup(locale_dir: &Path) -> anyhow::Result<()> {
        let _ = CATALOG.set(load_catalog(locale_dir));
        Ok(())
    }

    fn catalog() -> Option<&'static MoCatalog> {
        CATALOG.get().and_then(Option::as_ref)
    }

    pub(crate) fn gettext<T: Into<String>>(msgid: T) -> String {
        let msgid = msgid.into();
        catalog()
            .and_then(|catalog| catalog.lookup(None, &msgid))
            .map_or(msgid, str::to_owned)
    }

    pub(crate) fn pgettext<T: Into<String>, U: Into<String>>(msgctxt: T, msgid: U) -> String {
        let msgctxt = msgctxt.into();
        let msgid = msgid.into();
        catalog()
            .and_then(|catalog| catalog.lookup(Some(&msgctxt), &msgid))
            .map_or(msgid, str::to_owned)
    }

    fn load_catalog(locale_dir: &Path) -> Option<MoCatalog> {
        for language in language_candidates() {
            let dir = locale_dir.join(&language).join("LC_MESSAGES");
            let Some(mo) = find_mo_file(&dir) else {
                continue;
            };
            if let Ok(bytes) = fs::read(&mo)
                && let Ok(catalog) = MoCatalog::parse(&bytes)
            {
                return Some(catalog);
            }
        }
        None
    }

    fn find_mo_file(dir: &Path) -> Option<PathBuf> {
        fs::read_dir(dir)
            .ok()?
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .find(|path| path.extension().is_some_and(|ext| ext == "mo"))
    }

    /// LANGUAGE, LC_ALL, LC_MESSAGES, LANG plus GLib's list. Android does not
    /// export the Java locale to the POSIX environment, so this is best effort.
    fn language_candidates() -> Vec<String> {
        let mut languages = Vec::new();
        for name in ["LANGUAGE", "LC_ALL", "LC_MESSAGES", "LANG"] {
            if let Ok(value) = std::env::var(name) {
                push_languages(&mut languages, &value);
            }
        }
        for name in gtk4::glib::language_names() {
            push_languages(&mut languages, name.as_str());
        }
        push_language(&mut languages, "C");
        languages
    }

    fn push_languages(languages: &mut Vec<String>, value: &str) {
        for language in value.split(':') {
            // "de_DE.UTF-8@euro" -> "de_DE", then fall back to "de".
            let language = language.split(['.', '@']).next().unwrap_or(language);
            if language.is_empty() {
                continue;
            }
            push_language(languages, language);
            if let Some(base) = language.split('_').next()
                && base != language
            {
                push_language(languages, base);
            }
        }
    }

    fn push_language(languages: &mut Vec<String>, language: &str) {
        if !languages.iter().any(|known| known == language) {
            languages.push(language.to_owned());
        }
    }

    #[derive(Debug, Default)]
    struct MoCatalog {
        messages: HashMap<String, String>,
    }

    impl MoCatalog {
        fn parse(data: &[u8]) -> anyhow::Result<Self> {
            if data.len() < 28 {
                anyhow::bail!("mo file is too short");
            }
            let magic = u32::from_le_bytes(data[0..4].try_into()?);
            let little_endian = match magic {
                0x9504_12de => true,
                0xde12_0495 => false,
                _ => anyhow::bail!("mo file has an unknown magic"),
            };
            let word = |offset: usize| -> anyhow::Result<u32> {
                let bytes = data
                    .get(offset..offset + 4)
                    .ok_or_else(|| anyhow::anyhow!("mo file table is truncated"))?;
                let bytes: [u8; 4] = bytes.try_into()?;
                Ok(if little_endian {
                    u32::from_le_bytes(bytes)
                } else {
                    u32::from_be_bytes(bytes)
                })
            };

            let count = word(8)?;
            let originals_offset = word(12)? as usize;
            let translations_offset = word(16)? as usize;
            let mut messages = HashMap::new();

            for index in 0..count {
                let (original_length, original_offset) =
                    string_table_entry(&word, originals_offset, index)?;
                let (translation_length, translation_offset) =
                    string_table_entry(&word, translations_offset, index)?;
                let original = string_at(data, original_offset, original_length)?;
                let translation = string_at(data, translation_offset, translation_length)?;

                // The empty original is the header entry.
                if !original.is_empty() && !translation.is_empty() {
                    messages.insert(original.to_owned(), translation.to_owned());
                }
            }

            Ok(Self { messages })
        }

        fn lookup(&self, context: Option<&str>, msgid: &str) -> Option<&str> {
            let key = match context {
                Some(context) => format!("{context}\u{4}{msgid}"),
                None => msgid.to_owned(),
            };
            self.messages.get(&key).map(String::as_str)
        }
    }

    fn string_table_entry(
        word: &impl Fn(usize) -> anyhow::Result<u32>,
        table_offset: usize,
        index: u32,
    ) -> anyhow::Result<(usize, usize)> {
        let entry = table_offset + index as usize * 8;
        Ok((word(entry)? as usize, word(entry + 4)? as usize))
    }

    fn string_at(data: &[u8], offset: usize, length: usize) -> anyhow::Result<&str> {
        let bytes = data
            .get(offset..offset + length)
            .ok_or_else(|| anyhow::anyhow!("mo file string is out of bounds"))?;
        Ok(std::str::from_utf8(bytes)?)
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        /// Build a minimal little-endian .mo with one header and the given entries.
        fn mo(entries: &[(&str, &str)]) -> Vec<u8> {
            let mut strings = vec![("", "Content-Type: text/plain; charset=UTF-8\n")];
            strings.extend_from_slice(entries);
            strings.sort_by_key(|(original, _)| *original);

            let count = strings.len() as u32;
            let originals_offset = 28u32;
            let translations_offset = originals_offset + count * 8;
            let strings_offset = translations_offset + count * 8;

            let mut originals = Vec::new();
            let mut translations = Vec::new();
            let mut data = Vec::new();
            for (original, _translation) in &strings {
                originals.push((original.len() as u32, strings_offset + data.len() as u32));
                data.extend_from_slice(original.as_bytes());
            }
            for (_, translation) in &strings {
                translations.push((translation.len() as u32, strings_offset + data.len() as u32));
                data.extend_from_slice(translation.as_bytes());
            }

            let mut bytes = Vec::new();
            bytes.extend_from_slice(&0x9504_12deu32.to_le_bytes());
            bytes.extend_from_slice(&0u32.to_le_bytes());
            bytes.extend_from_slice(&count.to_le_bytes());
            bytes.extend_from_slice(&originals_offset.to_le_bytes());
            bytes.extend_from_slice(&translations_offset.to_le_bytes());
            bytes.extend_from_slice(&0u32.to_le_bytes());
            bytes.extend_from_slice(&0u32.to_le_bytes());
            for (length, offset) in originals {
                bytes.extend_from_slice(&length.to_le_bytes());
                bytes.extend_from_slice(&offset.to_le_bytes());
            }
            for (length, offset) in translations {
                bytes.extend_from_slice(&length.to_le_bytes());
                bytes.extend_from_slice(&offset.to_le_bytes());
            }
            bytes.extend_from_slice(&data);
            bytes
        }

        #[test]
        fn translates_messages_and_keeps_unknown_ones() {
            let catalog = MoCatalog::parse(&mo(&[
                ("Open", "Öffnen"),
                ("context\u{4}Save", "Speichern"),
            ]))
            .unwrap();

            assert_eq!(catalog.lookup(None, "Open"), Some("Öffnen"));
            assert_eq!(catalog.lookup(None, "Missing"), None);
            assert_eq!(catalog.lookup(Some("context"), "Save"), Some("Speichern"));
            assert_eq!(catalog.lookup(None, "Save"), None);
        }

        #[test]
        fn rejects_garbage() {
            assert!(MoCatalog::parse(b"not a mo file").is_err());
        }
    }
}

#[cfg(target_os = "android")]
pub(crate) use catalog::{gettext, pgettext};

#[cfg(target_os = "android")]
pub(crate) fn setup(locale_dir: &std::path::Path) -> anyhow::Result<()> {
    catalog::setup(locale_dir)
}
