// Imports
use self::imp::RnWorkspaceListEntryInner;
use gtk4::prelude::*;
use gtk4::subclass::prelude::*;
use gtk4::{gdk, gio, glib};
use once_cell::sync::Lazy;
use rnote_compose::color;
use rnote_engine::ext::GdkRGBAExt;
use std::cell::RefCell;
use std::path::PathBuf;

mod imp {
    use super::*;

    #[derive(Debug, Clone, glib::Variant, serde::Serialize, serde::Deserialize)]
    #[serde(default, rename = "workspacelistentryinner")]
    pub(crate) struct RnWorkspaceListEntryInner {
        /// A local path or a URI (`file://`, `content://`, ...).
        #[serde(rename = "dir")]
        pub(crate) dir: String,
        #[serde(rename = "icon")]
        pub(crate) icon: String,
        #[serde(rename = "color")]
        pub(crate) color: u32,
        #[serde(rename = "name")]
        pub(crate) name: String,
    }

    impl Default for RnWorkspaceListEntryInner {
        fn default() -> Self {
            let dir = default_workspace_dir();
            Self {
                dir: dir.to_string_lossy().to_string(),
                icon: String::from("workspacelistentryicon-folder-symbolic"),
                color: super::RnWorkspaceListEntry::COLOR_DEFAULT.as_rgba_u32(),
                name: String::from("default"),
            }
        }
    }

    /// The initial workspace directory.
    ///
    /// Android's `$HOME` is set at startup to the app-specific external files
    /// dir (env::setup_env, ADR 0001), so keep the usual `<home>/Documents`
    /// layout - the workspace browser lists it like any other folder.
    pub(super) fn default_workspace_dir() -> PathBuf {
        #[cfg(target_os = "android")]
        {
            // Keep the pre-fix `./` placeholder when even the home setup failed:
            // visibly broken beats a mysterious CWD-relative folder.
            let dir = match std::env::home_dir() {
                Some(home) => home.join("Documents"),
                None => return PathBuf::from("./"),
            };
            let _ = std::fs::create_dir_all(&dir);
            return dir;
        }

        #[cfg(not(target_os = "android"))]
        std::env::home_dir().unwrap_or_else(|| {
            if cfg!(target_os = "windows") {
                PathBuf::from("C:\\")
            } else {
                PathBuf::from("./")
            }
        })
    }

    #[derive(Debug, Default)]
    pub(crate) struct RnWorkspaceEntry {
        pub(crate) inner: RefCell<RnWorkspaceListEntryInner>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for RnWorkspaceEntry {
        const NAME: &'static str = "RnWorkspaceListEntry";
        type Type = super::RnWorkspaceListEntry;
    }

    impl ObjectImpl for RnWorkspaceEntry {
        fn properties() -> &'static [glib::ParamSpec] {
            static PROPERTIES: Lazy<Vec<glib::ParamSpec>> = Lazy::new(|| {
                vec![
                    glib::ParamSpecString::builder("dir")
                        .default_value(None)
                        .build(),
                    glib::ParamSpecString::builder("icon")
                        .default_value(None)
                        .build(),
                    glib::ParamSpecBoxed::builder::<gdk::RGBA>("color").build(),
                    glib::ParamSpecString::builder("name")
                        .default_value(None)
                        .build(),
                ]
            });
            PROPERTIES.as_ref()
        }

        fn set_property(&self, _id: usize, value: &glib::Value, pspec: &glib::ParamSpec) {
            match pspec.name() {
                "dir" => {
                    let dir = value
                        .get::<String>()
                        .expect("The value needs to be of type `String`");

                    self.inner.borrow_mut().dir = dir;
                }
                "icon" => {
                    let icon = value.get::<String>().expect("value not of type `String`");
                    self.inner.borrow_mut().icon = icon;
                }
                "color" => {
                    let color = value
                        .get::<gdk::RGBA>()
                        .expect("value not of type `gdk::RGBA`");
                    self.inner.borrow_mut().color = color.into_compose_color().into();
                }
                "name" => {
                    let name = value.get::<String>().expect("value not of type `String`");
                    self.inner.borrow_mut().name = name;
                }
                _ => unimplemented!(),
            }
        }

        fn property(&self, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
            match pspec.name() {
                "dir" => self.inner.borrow().dir.to_value(),
                "icon" => self.inner.borrow().icon.to_value(),
                "color" => gdk::RGBA::from_compose_color(rnote_compose::Color::from(
                    self.inner.borrow().color,
                ))
                .to_value(),
                "name" => self.inner.borrow().name.to_value(),
                _ => unimplemented!(),
            }
        }
    }
}

glib::wrapper! {
    pub(crate) struct RnWorkspaceListEntry(ObjectSubclass<imp::RnWorkspaceEntry>);
}

impl Default for RnWorkspaceListEntry {
    fn default() -> Self {
        Self::new(RnWorkspaceListEntryInner::default())
    }
}

impl RnWorkspaceListEntry {
    pub(crate) const COLOR_DEFAULT: piet::Color = color::GNOME_BLUES[4];

    pub(crate) fn new(inner: RnWorkspaceListEntryInner) -> Self {
        glib::Object::builder()
            .property("dir", inner.dir.to_value())
            .property("icon", inner.icon.to_value())
            .property(
                "color",
                gdk::RGBA::from_compose_color(rnote_compose::Color::from(inner.color)).to_value(),
            )
            .property("name", inner.name.to_value())
            .build()
    }

    pub(crate) fn replace_data(&self, entry: &Self) {
        self.set_name(entry.name());
        self.set_icon(entry.icon());
        self.set_color(entry.color());
        self.set_dir(entry.dir());
    }

    pub(crate) fn dir(&self) -> String {
        self.property::<String>("dir")
    }

    pub(crate) fn set_dir(&self, dir: String) {
        self.set_property("dir", dir.to_value());
    }

    pub(crate) fn dir_file(&self) -> gio::File {
        crate::utils::file_for_dir_str(&self.dir())
    }

    pub(crate) fn icon(&self) -> String {
        self.property::<String>("icon")
    }

    pub(crate) fn set_icon(&self, icon: String) {
        self.set_property("icon", icon.to_value());
    }

    pub(crate) fn color(&self) -> gdk::RGBA {
        self.property::<gdk::RGBA>("color")
    }

    pub(crate) fn set_color(&self, color: gdk::RGBA) {
        self.set_property("color", color.to_value());
    }

    pub(crate) fn name(&self) -> String {
        self.property::<String>("name")
    }

    pub(crate) fn set_name(&self, name: String) {
        self.set_property("name", name.to_value());
    }

    /// Move a missing local directory to its closest existing parent.
    /// Non-local URIs can't be walked up and are left unchanged.
    pub(crate) fn ensure_dir(&self) -> anyhow::Result<()> {
        let file = self.dir_file();
        if let Some(path) = file.path() {
            let p = crate::utils::path_walk_up_until_exists(path)?;
            self.set_dir(p.to_string_lossy().to_string());
        }
        Ok(())
    }

    /// The pre-home-dir-fix workspace dirs: `./` (the schema default) and
    /// `/` (where `ensure_dir` walked it up).
    #[cfg(any(target_os = "android", test))]
    fn is_broken_workspace_dir(dir: &str) -> bool {
        dir == "./" || dir == "/"
    }

    /// Re-defaults the dir when it is the broken pre-home-dir-fix fallback.
    /// The settings re-save the re-defaulted dir; no migration.
    #[cfg(target_os = "android")]
    pub(super) fn redefault_broken_dir(&self) {
        if Self::is_broken_workspace_dir(&self.dir()) {
            self.set_dir(imp::default_workspace_dir().to_string_lossy().to_string());
        }
    }
}

impl glib::variant::StaticVariantType for RnWorkspaceListEntry {
    fn static_variant_type() -> std::borrow::Cow<'static, glib::VariantTy> {
        let ty = RnWorkspaceListEntryInner::static_variant_type();
        let variant_type = glib::VariantType::new(ty.as_str()).unwrap();
        std::borrow::Cow::from(variant_type)
    }
}

impl glib::variant::ToVariant for RnWorkspaceListEntry {
    fn to_variant(&self) -> glib::Variant {
        self.imp().inner.borrow().to_variant()
    }
}

impl glib::variant::FromVariant for RnWorkspaceListEntry {
    fn from_variant(variant: &glib::Variant) -> Option<Self> {
        Some(Self::new(RnWorkspaceListEntryInner::from_variant(variant)?))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn broken_workspace_dirs_are_the_placeholder_and_root() {
        assert!(RnWorkspaceListEntry::is_broken_workspace_dir("./"));
        assert!(RnWorkspaceListEntry::is_broken_workspace_dir("/"));
        assert!(!RnWorkspaceListEntry::is_broken_workspace_dir(
            "/storage/emulated/0/Android/data/com.github.flxzt.rnote/files/Documents"
        ));
        assert!(!RnWorkspaceListEntry::is_broken_workspace_dir(
            "content://media/external/file/42"
        ));
    }

    #[test]
    fn ensure_dir_walks_up_missing_local_paths() {
        let missing = std::env::temp_dir()
            .join(format!("ranote-missing-{}", uuid::Uuid::new_v4()))
            .join("a")
            .join("b");
        let entry = RnWorkspaceListEntry::default();
        entry.set_dir(missing.to_string_lossy().to_string());

        entry.ensure_dir().unwrap();

        assert_eq!(entry.dir(), std::env::temp_dir().to_string_lossy());
    }

    #[test]
    fn ensure_dir_leaves_nonlocal_uris_unchanged() {
        let entry = RnWorkspaceListEntry::default();
        let uri = "content://com.example.documents/tree/missing";
        entry.set_dir(uri.to_string());

        entry.ensure_dir().unwrap();

        assert_eq!(entry.dir(), uri);
    }
}
