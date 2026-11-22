use crate::platform;
use crate::{RnAppWindow, RnWorkspaceBrowser};
use gettextrs::gettext;
use gtk4::{gio, glib, glib::clone, prelude::*};

pub(crate) fn open_folder(
    workspacebrowser: &RnWorkspaceBrowser,
    appwindow: &RnAppWindow,
) -> gio::SimpleAction {
    let open_folder_action = gio::SimpleAction::new("open-folder", None);

    open_folder_action.connect_activate(clone!(
        #[weak]
        workspacebrowser,
        #[weak]
        appwindow,
        move |_, _| {
            if let Some(folder) = workspacebrowser.dir_list_file() {
                if let Err(e) = platform::open_external(&folder) {
                    tracing::error!(
                        "Opening the workspace folder '{}' in the file manager failed, Err: {e:?}",
                        folder.uri()
                    );
                    appwindow.overlays().dispatch_toast_error(&gettext(
                        "Failed to open the file in the file manager",
                    ));
                }
            } else {
                tracing::warn!("No workspace folder selected");
                appwindow
                    .overlays()
                    .dispatch_toast_error(&gettext("Failed to open the file in the file manager"));
            }
        }
    ));

    open_folder_action
}
