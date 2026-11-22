#![warn(missing_debug_implementations)]
#![allow(clippy::single_match)]
// Turns off console window on Windows, but not when building with dev profile.
#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

// Modules
pub(crate) mod app;
pub(crate) mod appmenu;
pub(crate) mod appwindow;
pub(crate) mod boxed;
pub(crate) mod canvas;
pub(crate) mod canvasmenu;
pub(crate) mod canvaswrapper;
#[cfg(any(target_os = "android", test))]
pub(crate) mod clipboard;
pub(crate) mod colorpicker;
pub(crate) mod config;
pub(crate) mod contextmenu;
pub(crate) mod dialogs;
pub(crate) mod env;
pub(crate) mod filetype;
pub(crate) mod globals;
pub(crate) mod groupediconpicker;
pub(crate) mod i18n;
pub(crate) mod iconpicker;
pub(crate) mod mainheader;
pub(crate) mod overlays;
pub(crate) mod penpicker;
pub(crate) mod penssidebar;
pub(crate) mod perf;
pub(crate) mod platform;
pub(crate) mod settingspanel;
pub(crate) mod sidebar;
pub(crate) mod strokecontentpaintable;
pub(crate) mod strokecontentpreview;
pub(crate) mod strokewidthpicker;
pub(crate) mod unitentry;
pub(crate) mod utils;
pub(crate) mod workspacebrowser;

// Re-exports
pub(crate) use app::RnApp;
pub(crate) use appmenu::RnAppMenu;
pub(crate) use appwindow::RnAppWindow;
pub(crate) use canvas::RnCanvas;
pub(crate) use canvasmenu::RnCanvasMenu;
pub(crate) use canvaswrapper::RnCanvasWrapper;
pub(crate) use colorpicker::RnColorPicker;
pub(crate) use contextmenu::RnContextMenu;
pub(crate) use filetype::FileType;
pub(crate) use groupediconpicker::RnGroupedIconPicker;
pub(crate) use iconpicker::RnIconPicker;
pub(crate) use mainheader::RnMainHeader;
pub(crate) use overlays::RnOverlays;
pub(crate) use penpicker::RnPenPicker;
pub(crate) use penssidebar::RnPensSideBar;
pub(crate) use settingspanel::RnSettingsPanel;
pub(crate) use sidebar::RnSidebar;
pub(crate) use strokecontentpaintable::StrokeContentPaintable;
pub(crate) use strokecontentpreview::RnStrokeContentPreview;
pub(crate) use strokewidthpicker::RnStrokeWidthPicker;
pub(crate) use unitentry::RnUnitEntry;
pub(crate) use workspacebrowser::RnWorkspaceBrowser;

// Renames
extern crate parry2d_f64 as p2d;

// Imports
use anyhow::Context;
use gtk4::{gio, glib, prelude::*};
use tracing::debug;

fn main() -> glib::ExitCode {
    if let Err(e) = setup_tracing() {
        eprintln!("failed to setup tracing, Err: {e:?}");
    }
    if let Err(e) = env::setup_env() {
        eprintln!("failed to setup env, Err: {e:?}");
    }
    if let Err(e) = setup_i18n() {
        eprintln!("failed to setup i18n, Err: {e:?}");
    }
    if let Err(e) = setup_gresources() {
        eprintln!("failed to setup gresources, Err: {e:?}");
    }
    perf::start();

    let app = RnApp::new();
    app.run()
}

fn setup_tracing() -> anyhow::Result<()> {
    let timer = tracing_subscriber::fmt::time::Uptime::default();

    let mut env_filter = tracing_subscriber::EnvFilter::from_default_env();
    // Debug Android builds leave the perf target visible even without RUST_LOG.
    // The recorder itself stays off unless opted in, so this only unhides logs.
    if cfg!(all(target_os = "android", debug_assertions)) {
        let directive = "rnote::perf=info"
            .parse()
            .expect("static tracing directive is valid");
        env_filter = env_filter.add_directive(directive);
    }

    let builder = tracing_subscriber::fmt()
        .compact()
        .with_env_filter(env_filter)
        .with_timer(timer);
    // Android app processes have stdout/stderr at /dev/null, so tracing would be
    // invisible in logcat (and to device-check.sh) without an explicit writer.
    #[cfg(target_os = "android")]
    let builder = builder.with_writer(AndroidLogWriter);
    builder.try_init().map_err(|e| anyhow::anyhow!(e))?;
    debug!(".. tracing subscriber initialized.");
    Ok(())
}

/// Writes tracing output to logcat under the tag `rnote`, which is what
/// `logcat.sh` and `device-check.sh` filter for.
#[cfg(target_os = "android")]
#[derive(Clone, Copy, Debug)]
struct AndroidLogWriter;

#[cfg(target_os = "android")]
impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for AndroidLogWriter {
    type Writer = Self;

    fn make_writer(&'a self) -> Self::Writer {
        *self
    }
}

#[cfg(target_os = "android")]
impl std::io::Write for AndroidLogWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let text: Vec<u8> = buf
            .iter()
            .map(|byte| if *byte == 0 { b' ' } else { *byte })
            .collect();
        let Ok(text) = std::ffi::CString::new(text) else {
            return Ok(buf.len());
        };
        // SAFETY: both pointers are valid NUL-terminated strings for the call.
        unsafe { __android_log_write(4, c"rnote".as_ptr(), text.as_ptr()) };
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

#[cfg(target_os = "android")]
#[link(name = "log")]
unsafe extern "C" {
    fn __android_log_write(
        priority: std::ffi::c_int,
        tag: *const std::ffi::c_char,
        text: *const std::ffi::c_char,
    ) -> std::ffi::c_int;
}

fn setup_i18n() -> anyhow::Result<()> {
    let locale_dir = env::locale_dir()?;
    i18n::setup(&locale_dir)
}

fn setup_gresources() -> anyhow::Result<()> {
    gio::resources_register_include!("compiled.gresource")
        .context("Failed to register and include compiled gresource.")
}
