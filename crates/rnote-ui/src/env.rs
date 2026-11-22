// Imports
use crate::config;
use anyhow::Context;
use path_absolutize::Absolutize;
use std::ffi::OsStr;
use std::path::{Component, Path, PathBuf};
use std::{env, fs};
use tracing::debug;

pub(crate) fn lib_dir() -> anyhow::Result<PathBuf> {
    if cfg!(target_os = "windows") {
        let exec_dir = exec_parent_dir()?;

        Ok(exec_dir.join("..\\lib"))
    } else if cfg!(target_os = "macos") {
        let canonicalized_exec_dir = exec_parent_dir()?.canonicalize()?;

        if macos_is_in_app_bundle(&canonicalized_exec_dir) {
            Ok(canonicalized_exec_dir.join("../Frameworks"))
        } else {
            Ok(PathBuf::from(config::LIBDIR))
        }
    } else {
        Ok(PathBuf::from(config::LIBDIR))
    }
}

pub(crate) fn data_dir() -> anyhow::Result<PathBuf> {
    if cfg!(target_os = "windows") {
        let exec_dir = exec_parent_dir()?;

        Ok(exec_dir.join("..\\share"))
    } else if cfg!(target_os = "macos") {
        let canonicalized_exec_dir = exec_parent_dir()?.canonicalize()?;

        if macos_is_in_app_bundle(&canonicalized_exec_dir) {
            Ok(canonicalized_exec_dir.join("../Resources/share"))
        } else {
            Ok(PathBuf::from(config::DATADIR))
        }
    } else {
        Ok(PathBuf::from(config::DATADIR))
    }
}

pub(crate) fn pkg_data_dir() -> anyhow::Result<PathBuf> {
    #[cfg(target_os = "android")]
    {
        return Ok(android_dirs()?.data);
    }
    #[cfg(not(target_os = "android"))]
    {
        Ok(data_dir()?.join(config::APP_NAME))
    }
}

pub(crate) fn locale_dir() -> anyhow::Result<PathBuf> {
    #[cfg(target_os = "android")]
    {
        // GLib points XDG_DATA_DIRS at the asset copy (files/share) and
        // pixiewood installs the .mo files below share/locale there.
        if let Some(locale_dir) = gtk4::glib::system_data_dirs()
            .into_iter()
            .map(|dir| dir.join("locale"))
            .find(|dir| dir.is_dir())
        {
            return Ok(locale_dir);
        }
        // Fall back to the app-private data dir when assets were not packaged.
        return Ok(android_dirs()?.locale);
    }
    #[cfg(not(target_os = "android"))]
    {
        if cfg!(target_os = "windows") {
            let exec_dir = exec_parent_dir()?;

            Ok(exec_dir.join("..\\share\\locale"))
        } else if cfg!(target_os = "macos") {
            let canonicalized_exec_dir = exec_parent_dir()?.canonicalize()?;

            if macos_is_in_app_bundle(&canonicalized_exec_dir) {
                Ok(canonicalized_exec_dir.join("../Resources/share/locale"))
            } else {
                Ok(PathBuf::from(config::LOCALEDIR))
            }
        } else {
            Ok(PathBuf::from(config::LOCALEDIR))
        }
    }
}

#[cfg_attr(target_os = "android", allow(dead_code))]
pub(crate) fn cache_dir() -> anyhow::Result<PathBuf> {
    let cache_dir = match env::var("XDG_CACHE_HOME") {
        Ok(p) => PathBuf::from(p),
        Err(_) => {
            let home_dir = env::home_dir().context("Unable to retrieve home dir")?;
            home_dir.join(".cache")
        }
    };
    let cache_dir = cache_dir.absolutize()?.to_path_buf();
    if !cache_dir.exists() {
        debug!(
            "Cache directory '{}' not yet present, creating..",
            cache_dir.display()
        );
        fs::create_dir(&cache_dir)?;
        debug!("Created cache directory '{}'", cache_dir.display());
    }
    Ok(cache_dir)
}

/// App directories on Android, where there is no writable system data or locale dir.
/// Tests resolve this with fake base dirs; only the Android build reads the real env.
#[cfg(any(target_os = "android", test))]
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AndroidDirs {
    pub(crate) data: PathBuf,
    pub(crate) cache: PathBuf,
    pub(crate) locale: PathBuf,
}

#[cfg(any(target_os = "android", test))]
impl AndroidDirs {
    /// The Android activity glue points the XDG base dirs at the app's private
    /// storage; fall back to the usual HOME layout when it doesn't.
    fn resolve(data_home: Option<&Path>, cache_home: Option<&Path>, home: &Path) -> Self {
        let data_base = data_home.map_or_else(|| home.join(".local/share"), Path::to_path_buf);
        let cache_base = cache_home.map_or_else(|| home.join(".cache"), Path::to_path_buf);
        let data = data_base.join(config::APP_NAME);

        Self {
            cache: cache_base.join(config::APP_NAME),
            locale: data.join("locale"),
            data,
        }
    }
}

/// GTK's Android runtime puts the app's internal `files/share` in
/// `XDG_DATA_DIRS` but sets no `XDG_CACHE_HOME`, and `HOME` is not usable.
/// Android keeps the cache dir as a sibling of `files`, so derive it from there.
#[cfg_attr(not(any(target_os = "android", test)), allow(dead_code))]
fn cache_home_from_data_dirs(data_dirs: &[PathBuf]) -> Option<PathBuf> {
    for dir in data_dirs {
        let Some(files_dir) = dir.parent() else {
            continue;
        };
        if files_dir.file_name() != Some(OsStr::new("files")) {
            continue;
        }
        return files_dir.parent().map(|base| base.join("cache"));
    }
    None
}

#[cfg(target_os = "android")]
fn android_dirs() -> anyhow::Result<AndroidDirs> {
    let home = env::home_dir().context("Unable to retrieve home dir")?;
    let data_home = env::var_os("XDG_DATA_HOME").map(PathBuf::from);
    let cache_home = env::var_os("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .or_else(|| cache_home_from_data_dirs(&gtk4::glib::system_data_dirs()));

    Ok(AndroidDirs::resolve(
        data_home.as_deref(),
        cache_home.as_deref(),
        &home,
    ))
}

/// Android locale tags are BCP-47 (`de-DE`); the translation reader and
/// libintl look for POSIX names (`de_DE`).
#[cfg(any(target_os = "android", test))]
pub(crate) fn android_locale_tag(raw: &str) -> Option<String> {
    let tag = raw.trim().replace('-', "_");
    (!tag.is_empty()).then_some(tag)
}

/// Android does not export the Java locale to the POSIX environment. Read the
/// system locale property so the catalog reader can select a translation.
#[cfg(target_os = "android")]
fn android_system_locale() -> Option<String> {
    const PROPS: [&std::ffi::CStr; 2] = [c"persist.sys.locale", c"ro.product.locale"];

    unsafe extern "C" {
        fn __system_property_get(
            name: *const std::ffi::c_char,
            value: *mut std::ffi::c_char,
        ) -> std::ffi::c_int;
    }

    for prop in PROPS {
        // Android's PROP_VALUE_MAX is 92 bytes; 128 leaves room for the NUL.
        let mut value = [0 as std::ffi::c_char; 128];
        // SAFETY: both pointers are valid for the duration of the call and the
        // property API NUL-terminates the value.
        let len = unsafe { __system_property_get(prop.as_ptr(), value.as_mut_ptr()) };
        if len <= 0 {
            continue;
        }
        // SAFETY: a positive length means the buffer holds a NUL-terminated value.
        if let Ok(raw) = unsafe { std::ffi::CStr::from_ptr(value.as_ptr()) }.to_str()
            && let Some(tag) = android_locale_tag(raw)
        {
            return Some(tag);
        }
    }
    None
}

pub(crate) fn app_cache_dir() -> anyhow::Result<PathBuf> {
    #[cfg(target_os = "android")]
    let app_cache_dir = android_dirs()?.cache;
    #[cfg(not(target_os = "android"))]
    let app_cache_dir = cache_dir()?.join(config::APP_NAME);
    if !app_cache_dir.exists() {
        debug!(
            "Application cache directory '{}' not yet present, creating..",
            app_cache_dir.display()
        );
        fs::create_dir(&app_cache_dir)?;
        debug!(
            "Created application cache directory '{}'",
            app_cache_dir.display()
        );
    }
    Ok(app_cache_dir)
}

pub(crate) fn drafts_dir() -> anyhow::Result<PathBuf> {
    let app_cache_dir = crate::env::app_cache_dir()?;
    let drafts_dir = app_cache_dir.join("drafts");
    if !drafts_dir.exists() {
        debug!(
            "Drafts directory '{}' not yet present, creating..",
            drafts_dir.display()
        );
        fs::create_dir(&drafts_dir)?;
        debug!("Created drafts directory '{}'", drafts_dir.display());
    }
    Ok(drafts_dir)
}

/// depending on the target platform we need to set some env vars on startup
pub(crate) fn setup_env() -> anyhow::Result<()> {
    if cfg!(target_os = "windows") {
        let data_dir = data_dir()?;
        let lib_dir = lib_dir()?;

        // SAFETY: this setup only happens while still being single-threaded
        unsafe {
            std::env::set_var("XDG_DATA_DIRS", data_dir);
            std::env::set_var(
                "GDK_PIXBUF_MODULEDIR",
                lib_dir.join("gdk-pixbuf-2.0\\2.10.0\\loaders"),
            );
        }

        // The OpenGL renderer has proven to be buggy and can even produce crashes on windows.
        // Stick to vulkan by default.
        // If it's initialization fails, Gtk4 falls back to OpenGL anyway.
        if std::env::var_os("GSK_RENDERER").is_none() {
            unsafe { std::env::set_var("GSK_RENDERER", "vulkan") };
        }

        // Without DirectComposition GSK falls back to the cairo software
        // renderer, where strokes and images render as flat coloured (pink) boxes.
        if std::env::var_os("GDK_DEBUG").is_none() {
            unsafe { std::env::set_var("GDK_DEBUG", "dcomp") };
        }
        // On Windows, pangocairo renders glyphs through cairo's win32 font backend, which goes to DirectWrite
        // and Direct2D.
        // On Windows that path is broken: pango logs "All font fallbacks failed" for every layout and rendering
        // text eventually dies with an access violation (0xc0000005) in d2d1.dll.
        // It reproducibly takes down the app when a document containing a text stroke is opened, or when the
        // typewriter is used.
        //
        // The fontconfig/freetype backend works correctly here, so select it explicitly.
        // Fontconfig is present and configured in the MSYS2 environment the app is built and shipped with.
        //
        // This deliberately does not use `std::env::set_var`: on Windows that only calls SetEnvironmentVariableW,
        // which updates the Win32 environment block but not the copy the C runtime builds at startup.
        // pangocairo reads the variable with plain `getenv` (unlike GDK, which uses `g_getenv` and therefore does
        // see `set_var`), so it would never observe the value. `_putenv_s` updates the CRT table that `getenv`
        // reads, and pangocairo resolves to the same UCRT as the app.
        if std::env::var_os("PANGOCAIRO_BACKEND").is_none() {
            unsafe extern "C" {
                fn _putenv_s(
                    name: *const std::ffi::c_char,
                    value: *const std::ffi::c_char,
                ) -> std::ffi::c_int;
            }

            unsafe { _putenv_s(c"PANGOCAIRO_BACKEND".as_ptr(), c"fc".as_ptr()) };
        }

        //unsafe { std::env::set_var("RUST_LOG", "rnote=debug,rnote-cli=debug,rnote-engine=debug,rnote-compose=debug") };
    } else if cfg!(target_os = "android") {
        // The spike gates on GL: if GL cannot initialize that is a failure with
        // logs, not a reason to fall back to the cairo renderer.
        if std::env::var_os("GSK_RENDERER").is_none() {
            // SAFETY: this setup only happens while still being single-threaded
            unsafe { std::env::set_var("GSK_RENDERER", "gl") };
        }

        // Android has no dconf module. The keyfile backend writes under
        // XDG_CONFIG_HOME (GTK's Android runtime points it at the app's files
        // dir), so app settings survive a restart without a private store.
        if std::env::var_os("GSETTINGS_BACKEND").is_none() {
            // SAFETY: this setup only happens while still being single-threaded
            unsafe { std::env::set_var("GSETTINGS_BACKEND", "keyfile") };
        }

        // Android does not export the Java locale, so pick it up from the
        // system property before the catalog reader looks at the environment.
        #[cfg(target_os = "android")]
        {
            if std::env::var_os("LANGUAGE").is_none()
                && let Some(locale) = android_system_locale()
            {
                // SAFETY: this setup only happens while still being single-threaded
                unsafe { std::env::set_var("LANGUAGE", locale) };
            }
        }
    } else if cfg!(target_os = "macos") {
        let canonicalized_exec_dir = exec_parent_dir()?.canonicalize()?;

        if macos_is_in_app_bundle(canonicalized_exec_dir) {
            let data_dir = data_dir()?;
            let resources_dir = exec_parent_dir()?.canonicalize()?.join("../Resources");

            // SAFETY: this setup only happens while still being single-threaded
            unsafe {
                std::env::set_var("XDG_DATA_DIRS", data_dir);
                std::env::set_var(
                    "GDK_PIXBUF_MODULE_FILE",
                    resources_dir.join("etc/loaders.cache"),
                );
            }
        }
    }
    Ok(())
}

fn exec_parent_dir() -> anyhow::Result<PathBuf> {
    Ok(std::env::current_exe()?
        .parent()
        .ok_or(anyhow::anyhow!(
            "could not get parent dir of executable path"
        ))?
        .to_path_buf())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn android_locale_tags_become_posix_language_names() {
        assert_eq!(android_locale_tag("de-DE").as_deref(), Some("de_DE"));
        assert_eq!(android_locale_tag("pt-BR").as_deref(), Some("pt_BR"));
        assert_eq!(
            android_locale_tag("zh-Hans-CN").as_deref(),
            Some("zh_Hans_CN")
        );
        assert_eq!(android_locale_tag(""), None);
        assert_eq!(android_locale_tag("  "), None);
    }

    #[test]
    fn android_dirs_prefer_the_xdg_base_dirs() {
        let app_home = Path::new("/data/user/0/com.github.flxzt.rnote");

        let dirs = AndroidDirs::resolve(
            Some(&app_home.join("files")),
            Some(&app_home.join("cache")),
            app_home,
        );

        assert_eq!(dirs.data, app_home.join("files").join(config::APP_NAME));
        assert_eq!(dirs.cache, app_home.join("cache").join(config::APP_NAME));
        assert_eq!(
            dirs.locale,
            app_home.join("files").join(config::APP_NAME).join("locale")
        );
    }

    #[test]
    fn android_dirs_fall_back_to_the_home_layout() {
        let app_home = Path::new("/data/user/0/com.github.flxzt.rnote");

        let dirs = AndroidDirs::resolve(None, None, app_home);

        assert_eq!(
            dirs.data,
            app_home.join(".local/share").join(config::APP_NAME)
        );
        assert_eq!(dirs.cache, app_home.join(".cache").join(config::APP_NAME));
    }

    #[test]
    fn android_cache_home_is_the_sibling_of_the_files_dir() {
        let data_dirs = [
            PathBuf::from("/usr/share"),
            PathBuf::from("/data/user/0/com.github.flxzt.rnote/files/share"),
        ];

        assert_eq!(
            cache_home_from_data_dirs(&data_dirs),
            Some(PathBuf::from("/data/user/0/com.github.flxzt.rnote/cache"))
        );
    }

    #[test]
    fn android_cache_home_ignores_non_android_data_dirs() {
        assert_eq!(
            cache_home_from_data_dirs(&[PathBuf::from("/usr/share")]),
            None
        );
        assert_eq!(cache_home_from_data_dirs(&[]), None);
    }
}

// this returns true when the app is packaged as a relocatable application bundle
fn macos_is_in_app_bundle(canonicalized_exec_dir: impl AsRef<Path>) -> bool {
    canonicalized_exec_dir
        .as_ref()
        .components()
        .zip(canonicalized_exec_dir.as_ref().components().skip(1))
        .any(|(a, b)| {
            if let Component::Normal(a) = a
                && let Component::Normal(b) = b
            {
                a == OsStr::new("Contents") && b == OsStr::new("MacOS")
            } else {
                false
            }
        })
}
