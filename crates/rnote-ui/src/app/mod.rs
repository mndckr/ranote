// Modules
mod appactions;

// Imports
use crate::dialogs;
use crate::{
    RnAppMenu, RnAppWindow, RnCanvas, RnCanvasMenu, RnCanvasWrapper, RnColorPicker, RnIconPicker,
    RnMainHeader, RnOverlays, RnPenPicker, RnPensSideBar, RnSettingsPanel, RnSidebar,
    RnStrokeContentPreview, RnStrokeWidthPicker, RnUnitEntry, RnWorkspaceBrowser,
    colorpicker::RnColorPad, colorpicker::RnColorSetter, config, penssidebar::RnBrushPage,
    penssidebar::RnEraserPage, penssidebar::RnSelectorPage, penssidebar::RnShaperPage,
    penssidebar::RnToolsPage, penssidebar::RnTypewriterPage, settingspanel::RnPenShortcutRow,
    strokewidthpicker::RnStrokeWidthPreview, strokewidthpicker::RnStrokeWidthSetter,
    strokewidthpicker::StrokeWidthPreviewStyle, workspacebrowser::RnFileRow,
    workspacebrowser::RnWorkspacesBar, workspacebrowser::workspacesbar::RnWorkspaceRow,
};
use adw::subclass::prelude::AdwApplicationImpl;
use gtk4::{WindowGroup, gio, glib, glib::clone, prelude::*, subclass::prelude::*};

mod imp {
    use super::*;

    #[derive(Debug)]
    pub(crate) struct RnApp {
        pub(crate) app_settings: Option<gio::Settings>,
        /// Android: set by `activate` so the `open` that GApplication emits
        /// right after a cold-start activation opens in the new window. A warm
        /// start arrives without `activate` and gets its own window, which the
        /// new Android activity binds to.
        #[cfg(target_os = "android")]
        pub(crate) activated_window_open: std::cell::Cell<bool>,
    }

    impl Default for RnApp {
        fn default() -> Self {
            let app_settings = gio::SettingsSchemaSource::default().and_then(|schema_source| {
                Some(gio::Settings::new_full(
                    &schema_source.lookup(config::APP_ID, true)?,
                    None::<&gio::SettingsBackend>,
                    None,
                ))
            });

            Self {
                app_settings,
                #[cfg(target_os = "android")]
                activated_window_open: std::cell::Cell::new(false),
            }
        }
    }

    #[glib::object_subclass]
    impl ObjectSubclass for RnApp {
        const NAME: &'static str = "RnApp";
        type Type = super::RnApp;
        type ParentType = adw::Application;
    }

    impl ObjectImpl for RnApp {}

    impl ApplicationImpl for RnApp {
        fn startup(&self) {
            let obj = self.obj();
            self.parent_startup();

            self.setup_buildables();
            obj.setup_actions();
            obj.setup_action_accels();
        }

        fn activate(&self) {
            self.parent_activate();

            #[cfg(target_os = "android")]
            {
                // GApplication emits `open` right after this activation on a
                // cold start; let that call reuse the window created below.
                self.activated_window_open.set(true);
                let app = self.obj().downgrade();
                glib::idle_add_local_once(move || {
                    if let Some(app) = app.upgrade() {
                        app.imp().activated_window_open.set(false);
                    }
                });
            }

            // Android hands "Open with" files over through the platform seam;
            // desktop never queues one. Route them through the normal open path.
            let launch_file = crate::platform::take_launch_file();
            self.new_appwindow_init_show(launch_file);
        }

        fn open(&self, files: &[gio::File], hint: &str) {
            self.parent_open(files, hint);

            let input_file = files.first().cloned();

            // Desktop reuses the active window. On Android a cold start reuses
            // the activation window, while a warm start must create a window so
            // the new activity has a toplevel to bind to.
            #[cfg(target_os = "android")]
            let reuse_active = self.activated_window_open.replace(false);
            #[cfg(not(target_os = "android"))]
            let reuse_active = true;

            if reuse_active
                && let Some(appwindow) = self
                    .obj()
                    .active_window()
                    .map(|w| w.downcast::<RnAppWindow>().unwrap())
            {
                if let Some(input_file) = input_file {
                    glib::spawn_future_local(clone!(
                        #[weak]
                        appwindow,
                        async move {
                            appwindow.open_file_w_dialogs(input_file, None, true).await;
                        }
                    ));
                }
                return;
            }
            self.new_appwindow_init_show(input_file);
        }
    }

    impl GtkApplicationImpl for RnApp {}

    impl AdwApplicationImpl for RnApp {}

    impl RnApp {
        /// Custom buildable Widgets need to register
        fn setup_buildables(&self) {
            RnAppWindow::static_type();
            RnOverlays::static_type();
            RnCanvasWrapper::static_type();
            RnCanvas::static_type();
            RnColorPicker::static_type();
            RnColorSetter::static_type();
            RnColorPad::static_type();
            RnCanvasMenu::static_type();
            RnSettingsPanel::static_type();
            RnAppMenu::static_type();
            RnMainHeader::static_type();
            RnPensSideBar::static_type();
            RnBrushPage::static_type();
            RnShaperPage::static_type();
            RnEraserPage::static_type();
            RnSelectorPage::static_type();
            RnTypewriterPage::static_type();
            RnToolsPage::static_type();
            RnWorkspaceBrowser::static_type();
            RnWorkspacesBar::static_type();
            RnFileRow::static_type();
            RnWorkspaceRow::static_type();
            RnUnitEntry::static_type();
            RnIconPicker::static_type();
            RnPenShortcutRow::static_type();
            RnStrokeWidthPicker::static_type();
            RnStrokeWidthSetter::static_type();
            RnStrokeWidthPreview::static_type();
            StrokeWidthPreviewStyle::static_type();
            RnStrokeContentPreview::static_type();
            RnSidebar::static_type();
            RnPenPicker::static_type();
        }

        /// Initializes and shows a new app window
        pub(crate) fn new_appwindow_init_show(&self, input_file: Option<gio::File>) {
            let appwindow = RnAppWindow::new(self.obj().upcast_ref::<gtk4::Application>());
            appwindow.init(true);
            // create a window group for each app window
            // to make modals only impact the current app
            // window.
            // See issue: https://github.com/flxzt/rnote/issues/1461
            let window_group = WindowGroup::new();
            window_group.add_window(&appwindow);

            appwindow.present();

            glib::spawn_future_local(clone!(
                #[weak]
                appwindow,
                async move {
                    // Loading in input file in the first tab, if supplied.
                    if let Some(input_file) = input_file {
                        appwindow.open_file_w_dialogs(input_file, None, false).await;
                    }
                    // Recover drafts, if present.
                    let drafts = crate::utils::find_drafts();
                    if !drafts.is_empty() {
                        dialogs::dialog_recover_drafts(&appwindow, drafts).await;
                    }
                }
            ));
        }

        pub(crate) fn new_appwindow_init_return_tab(&self) -> adw::TabView {
            let appwindow = RnAppWindow::new(self.obj().upcast_ref::<gtk4::Application>());
            appwindow.init(false);

            // create a window group for each app window
            // to make modals only impact the current app
            // window.
            // See issue: https://github.com/flxzt/rnote/issues/1461
            let window_group = WindowGroup::new();
            window_group.add_window(&appwindow);
            appwindow.present();

            appwindow.overlays().tabview()
        }
    }
}

glib::wrapper! {
    pub(crate) struct RnApp(ObjectSubclass<imp::RnApp>)
        @extends gio::Application, gtk4::Application, adw::Application,
        @implements gio::ActionGroup, gio::ActionMap;
}

impl Default for RnApp {
    fn default() -> Self {
        Self::new()
    }
}

impl RnApp {
    pub(crate) fn new() -> Self {
        glib::Object::builder()
            .property("application-id", config::APP_ID)
            .property("resource-base-path", config::APP_IDPATH)
            .property("flags", gio::ApplicationFlags::HANDLES_OPEN)
            .property("register-session", true)
            .build()
    }

    /// Returns the app settings, if the schema is found in the compiled gschema. If not, returns None.
    ///
    /// Callers that query the settings should implement good default fallback accordingly
    pub(crate) fn app_settings(&self) -> Option<gio::Settings> {
        self.imp().app_settings.clone()
    }

    pub(crate) fn settings_schema_found(&self) -> bool {
        self.app_settings().is_some()
    }

    pub(crate) fn new_appwindow_init_show(&self) {
        self.imp().new_appwindow_init_show(None);
    }

    pub(crate) fn new_appwindow_init_return_tab(&self) -> adw::TabView {
        self.imp().new_appwindow_init_return_tab()
    }
}
