// Imports
use crate::RnApp;
use adw::prelude::*;
use gtk4::{gio, glib, glib::clone};

impl RnApp {
    pub(crate) fn setup_actions(&self) {
        let action_quit = gio::SimpleAction::new("quit", None);
        self.add_action(&action_quit);

        let action_color_scheme = gio::SimpleAction::new_stateful(
            "color-scheme",
            Some(&glib::VariantType::new("s").unwrap()),
            &String::from("system").to_variant(),
        );
        self.add_action(&action_color_scheme);

        let action_new_window = gio::SimpleAction::new("new-window", None);
        self.add_action(&action_new_window);

        let style_manager = self.style_manager();
        let color_scheme_binding = action_color_scheme
            .bind_property("state", &style_manager, "color-scheme")
            .transform_to(move |_, val: glib::Variant| {
                match val.get::<String>().unwrap().as_str() {
                    "default" => Some(adw::ColorScheme::Default),
                    "force-light" => Some(adw::ColorScheme::ForceLight),
                    "force-dark" => Some(adw::ColorScheme::ForceDark),
                    _ => None,
                }
            })
            .transform_from(move |_, val: adw::ColorScheme| match val {
                adw::ColorScheme::Default => Some(String::from("default").to_value()),
                adw::ColorScheme::ForceLight => Some(String::from("force-light").to_value()),
                adw::ColorScheme::ForceDark => Some(String::from("force-dark").to_value()),
                _ => None,
            })
            .sync_create();

        // Desktop lets libadwaita and the action mirror each other. Android has
        // to translate the system night mode into an effective scheme, so the
        // action state must keep the user's "default"/forced choice instead of
        // being overwritten by the effective one.
        #[cfg(not(target_os = "android"))]
        color_scheme_binding.bidirectional().build();
        #[cfg(target_os = "android")]
        color_scheme_binding.build();

        #[cfg(target_os = "android")]
        crate::platform::follow_system_color_scheme(&style_manager, &action_color_scheme);

        // Quit App
        action_quit.connect_activate(clone!(
            #[weak(rename_to = app)]
            self,
            move |_, _| {
                // Request closing all windows. They then get the chance to display a save dialog on unsaved changes
                for appwindow in app.windows() {
                    appwindow.close();
                }

                if app.windows().is_empty() {
                    app.quit();
                }
            }
        ));

        action_new_window.connect_activate(clone!(
            #[weak(rename_to = app)]
            self,
            move |_, _| {
                app.new_appwindow_init_show();
            }
        ));
    }

    // Accelerators / Keyboard Shortcuts
    pub(crate) fn setup_action_accels(&self) {
        self.set_accels_for_action("app.quit", &["<Ctrl>q"]);
        self.set_accels_for_action("app.new-window", &["<Ctrl>n"]);
    }
}
