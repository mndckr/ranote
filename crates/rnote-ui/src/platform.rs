//! Platform capability seam.
//!
//! Every "does this OS support X?" question and every OS entry point (pause/resume,
//! launch intents, opening files externally) lives here, so platform checks don't
//! leak into unrelated UI code. Desktop behaviour is unchanged: all capabilities are
//! available and the OS callbacks are never invoked.

use gtk4::gio;
use gtk4::gio::prelude::*;
#[cfg(target_os = "android")]
use gtk4::glib;
#[cfg(target_os = "android")]
use gtk4::prelude::*;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};

/// A feature whose availability differs between platforms.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Feature {
    Print,
    FolderPicker,
    PenSounds,
}

/// What the current platform can do.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Capabilities {
    pub(crate) can_print: bool,
    pub(crate) supports_folder_picker: bool,
    pub(crate) supports_pen_sounds: bool,
}

impl Capabilities {
    /// Linux, Windows, macOS: everything the UI offers works.
    pub(crate) const DESKTOP: Self = Self {
        can_print: true,
        supports_folder_picker: true,
        supports_pen_sounds: true,
    };
    /// Android: no print backend and no audio backend for pen sounds. GTK's
    /// Android file dialog maps the folder picker to ACTION_OPEN_DOCUMENT_TREE,
    /// so workspace folders are chosen through SAF.
    #[cfg_attr(not(any(target_os = "android", test)), allow(dead_code))]
    pub(crate) const ANDROID: Self = Self {
        can_print: false,
        supports_folder_picker: true,
        supports_pen_sounds: false,
    };

    pub(crate) fn supports(&self, feature: Feature) -> bool {
        match feature {
            Feature::Print => self.can_print,
            Feature::FolderPicker => self.supports_folder_picker,
            Feature::PenSounds => self.supports_pen_sounds,
        }
    }
}

pub(crate) fn capabilities() -> Capabilities {
    #[cfg(target_os = "android")]
    {
        Capabilities::ANDROID
    }
    #[cfg(not(target_os = "android"))]
    {
        Capabilities::DESKTOP
    }
}

pub(crate) fn feature_available(feature: Feature) -> bool {
    capabilities().supports(feature)
}

/// Whether the canvas must draw the IME composition (preedit) string itself.
///
/// Desktop IM modules (IBus, Wayland text-input, Windows IME) draw the composition in
/// their own popup, so drawing it again would duplicate it. GTK's Android IM context
/// does not draw anything: it only emits `preedit-changed`/`commit` and expects the
/// application to render the composition.
pub(crate) fn app_draws_ime_preedit() -> bool {
    cfg!(target_os = "android")
}

/// Resolve the persisted color-scheme choice to the scheme to apply.
///
/// `system_prefers_dark` is the OS night-mode preference, or `None` when the
/// platform does not expose one (desktop). Android's libadwaita integration
/// does not see the Android night mode, so the platform module feeds it in.
#[cfg_attr(not(any(target_os = "android", test)), allow(dead_code))]
pub(crate) fn resolve_system_color_scheme(
    requested: &str,
    system_prefers_dark: Option<bool>,
) -> adw::ColorScheme {
    match requested {
        "force-light" => adw::ColorScheme::ForceLight,
        "force-dark" => adw::ColorScheme::ForceDark,
        _ => match system_prefers_dark {
            Some(true) => adw::ColorScheme::ForceDark,
            Some(false) => adw::ColorScheme::ForceLight,
            None => adw::ColorScheme::Default,
        },
    }
}

/// Android's GDK backend exposes the system night mode as a `night-mode`
/// property (enum: 0 undefined, 1 no, 2 yes). Desktop displays have no such
/// property, so this stays Android-only.
#[cfg(target_os = "android")]
fn system_prefers_dark() -> Option<bool> {
    let display = gtk4::gdk::Display::default()?;
    if display.find_property("night-mode").is_none() {
        return None;
    }

    match display.property_value("night-mode").get::<i32>() {
        Ok(2) => Some(true),
        Ok(1) => Some(false),
        _ => None,
    }
}

/// Make libadwaita follow Android's dark mode when the stored color scheme is
/// "default". The app action's state keeps the user's requested scheme, so a
/// forced light/dark choice still wins and only the effective theme follows the
/// system.
#[cfg(target_os = "android")]
pub(crate) fn follow_system_color_scheme(
    style_manager: &adw::StyleManager,
    action: &gio::SimpleAction,
) {
    let Some(display) = gtk4::gdk::Display::default() else {
        return;
    };
    if display.find_property("night-mode").is_none() {
        return;
    }

    let update = {
        let style_manager = style_manager.clone();
        let action = action.clone();
        move || {
            let requested = action
                .state()
                .and_then(|state| state.get::<String>())
                .unwrap_or_else(|| String::from("default"));

            style_manager.set_color_scheme(resolve_system_color_scheme(
                &requested,
                system_prefers_dark(),
            ));
        }
    };

    update();
    action.connect_notify_local(Some("state"), {
        let update = update.clone();
        move |_, _| update()
    });
    display.connect_notify_local(Some("night-mode"), move |_, _| update());
}

/// Disable an action whose feature the platform cannot back, so a broken dialog
/// is never reachable from the menu or an accelerator.
pub(crate) fn apply_capabilities(
    caps: Capabilities,
    feature: Feature,
    action: &(impl IsA<gio::Action> + 'static),
) {
    // GAction has no generic `set_enabled` in gio-rs, but "enabled" is a property.
    action.set_property("enabled", caps.supports(feature));
}

/// Open a file or folder through the platform's default handler.
pub(crate) fn open_external(file: &gio::File) -> anyhow::Result<()> {
    gio::AppInfo::launch_default_for_uri(&file.uri(), None::<&gio::AppLaunchContext>)?;
    Ok(())
}

type LifecycleHandler = Box<dyn Fn() + Send + Sync>;

fn pause_handlers() -> &'static Mutex<Vec<LifecycleHandler>> {
    static HANDLERS: OnceLock<Mutex<Vec<LifecycleHandler>>> = OnceLock::new();
    HANDLERS.get_or_init(Default::default)
}

fn resume_handlers() -> &'static Mutex<Vec<LifecycleHandler>> {
    static HANDLERS: OnceLock<Mutex<Vec<LifecycleHandler>>> = OnceLock::new();
    HANDLERS.get_or_init(Default::default)
}

fn run_handlers(handlers: &Mutex<Vec<LifecycleHandler>>) {
    if let Ok(handlers) = handlers.lock() {
        for handler in handlers.iter() {
            handler();
        }
    }
}

/// Register a handler run when the OS pauses the app. Desktop never pauses, so the
/// handler simply never runs. Used by the lifecycle/autosave work.
#[cfg_attr(not(any(target_os = "android", test)), allow(dead_code))]
pub(crate) fn on_pause(handler: impl Fn() + Send + Sync + 'static) {
    if let Ok(mut handlers) = pause_handlers().lock() {
        handlers.push(Box::new(handler));
    }
}

/// Register a handler run when the OS resumes the app.
#[cfg_attr(not(any(target_os = "android", test)), allow(dead_code))]
pub(crate) fn on_resume(handler: impl Fn() + Send + Sync + 'static) {
    if let Ok(mut handlers) = resume_handlers().lock() {
        handlers.push(Box::new(handler));
    }
}

/// Called by the Android activity glue when the app is paused. Must run on the GTK
/// main thread. Desktop never calls this.
#[cfg_attr(not(any(target_os = "android", test)), allow(dead_code))]
pub(crate) fn notify_paused() {
    run_handlers(pause_handlers());
}

/// Called by the Android activity glue when the app is resumed.
#[cfg_attr(not(any(target_os = "android", test)), allow(dead_code))]
pub(crate) fn notify_resumed() {
    run_handlers(resume_handlers());
}

/// Android's activity reports window focus, not pause/stop. The app has no
/// focused window exactly while the activity is paused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(not(any(target_os = "android", test)), allow(dead_code))]
enum Lifecycle {
    Paused,
    Resumed,
}

#[cfg_attr(not(any(target_os = "android", test)), allow(dead_code))]
fn lifecycle_transition(was_paused: bool, any_window_active: bool) -> Option<Lifecycle> {
    match (was_paused, any_window_active) {
        (false, false) => Some(Lifecycle::Paused),
        (true, true) => Some(Lifecycle::Resumed),
        _ => None,
    }
}

#[cfg_attr(not(target_os = "android"), allow(dead_code))]
static LIFECYCLE_PAUSED: AtomicBool = AtomicBool::new(false);

/// Turn the current focus state into pause/resume notifications, once per change.
#[cfg_attr(not(target_os = "android"), allow(dead_code))]
fn apply_lifecycle(any_window_active: bool) {
    let was_paused = LIFECYCLE_PAUSED.swap(!any_window_active, Ordering::SeqCst);
    match lifecycle_transition(was_paused, any_window_active) {
        Some(Lifecycle::Paused) => notify_paused(),
        Some(Lifecycle::Resumed) => notify_resumed(),
        None => {}
    }
}

/// Watch a window's focus transitions on Android and translate them into
/// pause/resume notifications. GTK never reports pause itself, but losing the
/// activity's window focus is what pause does on Android. Desktop windows also
/// lose focus, so there this is a no-op and behaviour is unchanged.
#[cfg(target_os = "android")]
pub(crate) fn watch_lifecycle(window: &gtk4::Window) {
    window.connect_notify_local(Some("is-active"), |window, _| {
        let Some(application) = window.application() else {
            return;
        };
        // Defer one main-loop iteration: a closing window is gone by then, so
        // closing does not count as a pause.
        glib::idle_add_local_once(move || {
            let windows = application.windows();
            if windows.is_empty() {
                return;
            }
            apply_lifecycle(windows.iter().any(|window| window.is_active()));
        });
    });
}

#[cfg(not(target_os = "android"))]
pub(crate) fn watch_lifecycle(_window: &gtk4::Window) {}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[test]
    fn unavailable_capabilities_disable_actions() {
        let caps = Capabilities {
            can_print: false,
            supports_folder_picker: false,
            supports_pen_sounds: false,
        };
        let print = gio::SimpleAction::new("print-doc", None);
        let pick_folder = gio::SimpleAction::new("pick-folder", None);

        apply_capabilities(caps, Feature::Print, &print);
        apply_capabilities(caps, Feature::FolderPicker, &pick_folder);

        assert!(!print.is_enabled());
        assert!(!pick_folder.is_enabled());
    }

    #[test]
    fn desktop_capabilities_keep_actions_available() {
        let print = gio::SimpleAction::new("print-doc", None);
        let pick_folder = gio::SimpleAction::new("pick-folder", None);

        apply_capabilities(Capabilities::DESKTOP, Feature::Print, &print);
        apply_capabilities(Capabilities::DESKTOP, Feature::FolderPicker, &pick_folder);

        assert!(print.is_enabled());
        assert!(pick_folder.is_enabled());
    }

    #[test]
    fn pen_sounds_action_is_disabled_when_unavailable() {
        let pen_sounds = gio::SimpleAction::new_stateful("pen-sounds", None, &false.to_variant());

        apply_capabilities(Capabilities::ANDROID, Feature::PenSounds, &pen_sounds);
        assert!(!pen_sounds.is_enabled());

        apply_capabilities(Capabilities::DESKTOP, Feature::PenSounds, &pen_sounds);
        assert!(pen_sounds.is_enabled());
    }

    #[test]
    fn android_capabilities_match_the_spike_limits() {
        assert!(!Capabilities::ANDROID.supports(Feature::Print));
        // GTK's Android file dialog maps a folder picker to ACTION_OPEN_DOCUMENT_TREE.
        assert!(Capabilities::ANDROID.supports(Feature::FolderPicker));
        assert!(!Capabilities::ANDROID.supports(Feature::PenSounds));
        assert!(Capabilities::DESKTOP.supports(Feature::Print));
        assert!(Capabilities::DESKTOP.supports(Feature::FolderPicker));
        assert!(Capabilities::DESKTOP.supports(Feature::PenSounds));
    }

    #[test]
    fn desktop_keeps_drawing_ime_preedit_itself() {
        // Android's IM context relies on the app to draw the composition; desktop IM
        // modules draw their own, so the canvas must leave desktop behaviour unchanged.
        assert_eq!(app_draws_ime_preedit(), cfg!(target_os = "android"));
    }

    #[test]
    fn lifecycle_transitions_only_when_the_pause_state_changes() {
        assert_eq!(lifecycle_transition(false, false), Some(Lifecycle::Paused));
        assert_eq!(lifecycle_transition(true, false), None);
        assert_eq!(lifecycle_transition(true, true), Some(Lifecycle::Resumed));
        assert_eq!(lifecycle_transition(false, true), None);
    }

    #[test]
    fn default_color_scheme_follows_the_android_night_mode() {
        assert_eq!(
            resolve_system_color_scheme("default", Some(true)),
            adw::ColorScheme::ForceDark
        );
        assert_eq!(
            resolve_system_color_scheme("default", Some(false)),
            adw::ColorScheme::ForceLight
        );
        assert_eq!(
            resolve_system_color_scheme("default", None),
            adw::ColorScheme::Default
        );
    }

    #[test]
    fn explicit_color_scheme_wins_over_the_android_night_mode() {
        assert_eq!(
            resolve_system_color_scheme("force-dark", Some(false)),
            adw::ColorScheme::ForceDark
        );
        assert_eq!(
            resolve_system_color_scheme("force-light", Some(true)),
            adw::ColorScheme::ForceLight
        );
    }

    #[test]
    fn pause_and_resume_run_registered_handlers() {
        let pauses = Arc::new(AtomicUsize::new(0));
        let resumes = Arc::new(AtomicUsize::new(0));

        on_pause({
            let pauses = pauses.clone();
            move || {
                pauses.fetch_add(1, Ordering::SeqCst);
            }
        });
        on_resume({
            let resumes = resumes.clone();
            move || {
                resumes.fetch_add(1, Ordering::SeqCst);
            }
        });

        notify_paused();
        notify_resumed();

        assert_eq!(pauses.load(Ordering::SeqCst), 1);
        assert_eq!(resumes.load(Ordering::SeqCst), 1);
    }
}
