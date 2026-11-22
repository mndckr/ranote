// Imports
use crate::FileType;
use crate::i18n::pgettext;
use gtk4::{gdk, gio, glib, prelude::*};
use palette::convert::IntoColor;
use path_absolutize::Absolutize;
use rnote_compose::Color;
use std::cell::Ref;
use std::fs;
use std::path::{Path, PathBuf};
use std::slice::Iter;
use tracing::error;

/// The suffix delimiter when duplicating/renaming already existing files
pub(crate) const FILE_DUP_SUFFIX_DELIM: &str = " - ";
/// The suffix delimiter when duplicating/renaming already existing files for usage in a regular expression
pub(crate) const FILE_DUP_SUFFIX_DELIM_REGEX: &str = r"\s-\s";

/// An asynchronous adaptation of the [`rnote_engine::utils::atomic_save_to_file`] function.
pub(crate) async fn atomic_save_to_file_future<Q>(filepath: Q, bytes: Vec<u8>) -> anyhow::Result<()>
where
    Q: AsRef<std::path::Path>,
{
    let filepath = filepath.as_ref().to_path_buf();

    blocking::unblock(move || rnote_engine::utils::atomic_save_to_file(filepath, bytes)).await
}

pub(crate) fn find_drafts() -> Vec<PathBuf> {
    let drafts_dir = match crate::env::drafts_dir() {
        Ok(d) => d,
        Err(err) => {
            error!("Unable to retrieve drafts directory: {err}");
            return Vec::new();
        }
    };
    let read_dir = match fs::read_dir(&drafts_dir) {
        Ok(r) => r,
        Err(err) => {
            error!(
                "Unable to list drafts in directory '{}': {err}",
                drafts_dir.display()
            );
            return Vec::new();
        }
    };

    read_dir
        .into_iter()
        .filter_map(|entry| {
            let entry = entry.ok()?.path();
            if FileType::lookup_file_type(&gio::File::for_path(&entry)) == FileType::RnoteFile {
                Some(entry)
            } else {
                None
            }
        })
        .collect()
}

/// Create a new file or replace if it already exists, asynchronously.
///
/// Uses `gio::File`, so it works for local paths and URIs without requiring `file.path()`.
pub(crate) async fn create_replace_file_future(
    bytes: Vec<u8>,
    file: &gio::File,
) -> anyhow::Result<()> {
    file.replace_contents_future(
        bytes,
        None,
        false,
        gio::FileCreateFlags::REPLACE_DESTINATION,
    )
    .await
    .map_err(|(_, err)| anyhow::anyhow!("Failed to write file `{file:?}`, Err: {err}"))?;
    Ok(())
}

/// Write fully prepared export bytes.
///
/// A failed export must not leave a partial file. On Android the SAF
/// create-document picker only ever returns a freshly created document, so
/// removing it on failure cannot touch existing user data.
pub(crate) async fn write_export_file(bytes: Vec<u8>, file: &gio::File) -> anyhow::Result<()> {
    match create_replace_file_future(bytes, file).await {
        Ok(()) => Ok(()),
        Err(err) => {
            #[cfg(target_os = "android")]
            if let Err(cleanup_err) = file.delete_future().await {
                error!("Removing failed export `{file:?}` failed, Err: {cleanup_err}");
            }
            Err(err)
        }
    }
}

/// Whether two [`gio::File`]s refer to the same document.
///
/// Compares the file identity GIO derives from the URI, so a local path and its
/// `file://` URI match, and no `PathBuf` or `path()` is assumed.
pub(crate) fn files_eq(first: &gio::File, second: &gio::File) -> bool {
    first.equal(second)
}

/// The directory a document is displayed as living in.
///
/// Content URIs have no parent directory, so show their provider instead.
pub(crate) fn folder_display(file: &gio::File) -> Option<String> {
    if let Some(parent) = file.parent() {
        return Some(
            parent
                .path()
                .map(|p| p.display().to_string())
                .unwrap_or_else(|| parent.uri().to_string()),
        );
    }

    file.uri()
        .strip_prefix("content://")
        .and_then(|rest| rest.split('/').next())
        .map(|authority| format!("content://{authority}"))
}

/// The local path to watch for external changes, if the document has one.
///
/// GdkAndroidContentFile reports its URI path as `path()`, which is not a
/// filesystem path, so non-native files must not be watched with `notify`.
pub(crate) fn local_watch_path(file: &gio::File) -> Option<PathBuf> {
    if !file.is_native() {
        return None;
    }
    file.path()
}

/// Give a save-as target the `.rnote` extension.
///
/// Android's create-document picker ignores GtkFileDialog's suggested name, so a
/// new document can come back without the extension that identifies it on reopen.
pub(crate) fn ensure_rnote_extension(file: gio::File) -> gio::File {
    if !file.has_uri_scheme("content") {
        return file;
    }
    let Some(name) = file.basename() else {
        return file;
    };
    let name = name.to_string_lossy();
    if name.is_empty() || name.to_ascii_lowercase().ends_with(".rnote") {
        return file;
    }

    match file.set_display_name(&format!("{name}.rnote"), gio::Cancellable::NONE) {
        Ok(renamed) => renamed,
        Err(err) => {
            error!(
                "Renaming save target `{}` to add the .rnote extension failed: {err}",
                file.uri()
            );
            file
        }
    }
}

/// Resolve a stored workspace directory, which is either a local path or a URI.
pub(crate) fn file_for_dir_str(dir: &str) -> gio::File {
    // "://" keeps Windows drive paths ("C:\notes") from parsing as a URI scheme.
    if dir.contains("://") && glib::uri_parse_scheme(dir).is_some() {
        gio::File::for_uri(dir)
    } else {
        gio::File::for_path(dir)
    }
}

/// The stable identity used to store a workspace directory: its local path, or its URI when it has none.
pub(crate) fn file_dir_identity(file: &gio::File) -> String {
    file.path()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|| file.uri().to_string())
}

/// Find a non-existing file in `folder` named after `base_title` and `file_ext`,
/// appending the duplicate suffix as needed.
///
/// Returns the file and the next free duplicate index, so callers can keep names
/// unique across several unsaved documents.
pub(crate) fn unique_file_in_dir(
    folder: &gio::File,
    base_title: &str,
    file_ext: &str,
    start_index: u32,
) -> (gio::File, u32) {
    let mut index = start_index;
    loop {
        let name = if index == 0 {
            format!("{base_title}.{file_ext}")
        } else {
            format!("{base_title}{FILE_DUP_SUFFIX_DELIM}{index}.{file_ext}")
        };
        let candidate = folder.child(&name);

        if !candidate.query_exists(gio::Cancellable::NONE) {
            return (candidate, index + 1);
        }

        index += 1;
    }
}

/// The location shown for a selected file: its local path, or its URI when it has none.
pub(crate) fn file_location_label(file: &gio::File) -> String {
    file.path()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|| file.uri().to_string())
}

/// What an export success toast should open: the containing folder for local
/// files, or the file itself when there is no browsable folder (content URI).
pub(crate) fn export_view_target(file: &gio::File) -> gio::File {
    file.parent()
        .filter(|parent| parent.path().is_some())
        .unwrap_or_else(|| file.clone())
}

pub(crate) fn str_from_u8_nul_utf8(utf8_src: &[u8]) -> Result<&str, std::str::Utf8Error> {
    let nul_range_end = utf8_src
        .iter()
        .position(|&c| c == b'\0')
        .unwrap_or(utf8_src.len()); // default to length if no `\0` present
    std::str::from_utf8(&utf8_src[0..nul_range_end])
}

/// Get the index of the AxisUse enum
///
/// TODO: Report to gtk-rs that [gdk::AxisUse] needs a [`Into<std::ops::Index>`] implementation
/// for usage to retrieve pointer axes in [gdk::TimeCoord]
pub(crate) fn axis_use_idx(a: gdk::AxisUse) -> usize {
    match a {
        gdk::AxisUse::Ignore => 0,
        gdk::AxisUse::X => 1,
        gdk::AxisUse::Y => 2,
        gdk::AxisUse::DeltaX => 3,
        gdk::AxisUse::DeltaY => 4,
        gdk::AxisUse::Pressure => 5,
        gdk::AxisUse::Xtilt => 6,
        gdk::AxisUse::Ytilt => 7,
        gdk::AxisUse::Wheel => 8,
        gdk::AxisUse::Distance => 9,
        gdk::AxisUse::Rotation => 10,
        gdk::AxisUse::Slider => 11,
        _ => unreachable!(),
    }
}

pub(crate) fn default_file_title_for_export(
    output_file: Option<gio::File>,
    fallback: Option<&str>,
    suffix: Option<&str>,
) -> String {
    let mut title = output_file
        .and_then(|f| Some(f.basename()?.file_stem()?.to_string_lossy().to_string()))
        .unwrap_or_else(|| {
            fallback
                .map(|f| f.to_owned())
                .unwrap_or_else(rnote_engine::utils::now_formatted_string)
        });

    if let Some(suffix) = suffix {
        title += suffix;
    }

    title
}

/// Absolutizes two paths and checks if they are equal.
///
/// Compared to `canonicalize()`, the files do not need to exist on the fs and symlinks are not resolved.
#[inline]
pub(crate) fn paths_abs_eq(
    first: impl AsRef<Path>,
    second: impl AsRef<Path>,
) -> anyhow::Result<bool> {
    let first = first.as_ref().absolutize()?.into_owned();
    let second = second.as_ref().absolutize()?.into_owned();
    Ok(first == second)
}

/// Wrapper type that enables iterating over [`std::cell::RefCell<Vec<T>>`]
pub(crate) struct VecRefWrapper<'a, T: 'a> {
    r: Ref<'a, Vec<T>>,
}

impl<'a, 'b: 'a, T: 'a> IntoIterator for &'b VecRefWrapper<'a, T> {
    type IntoIter = Iter<'a, T>;
    type Item = &'a T;

    fn into_iter(self) -> Iter<'a, T> {
        self.r.iter()
    }
}

impl<'a, T> VecRefWrapper<'a, T>
where
    T: 'a,
{
    pub(crate) fn new(r: Ref<'a, Vec<T>>) -> Self {
        Self { r }
    }
}

/// Create a string for display the hue, saturation and value properties of the color.
pub(crate) fn color_to_hsv_label_string(color: Color) -> String {
    let palette_color: palette::Okhsv<f64> = color.into_color();
    let alpha = color.a;
    let hue = palette_color.hue.into_inner();
    let saturation = palette_color.saturation;
    let value = palette_color.value;

    // Since the color might come from gdk which only has f32 precision, let's use f32's epsilon
    let min_saturated =
        approx::relative_eq!(saturation, 0.0, epsilon = f32::EPSILON as f64) || saturation <= 0.0;
    //let max_saturated = approx::relative_eq!(saturation, 1.0, epsilon = f32::EPSILON as f64) || saturation >= 1.0;
    let min_bright =
        approx::relative_eq!(value, 0.0, epsilon = f32::EPSILON as f64) || value <= 0.0;
    let max_bright =
        approx::relative_eq!(value, 1.0, epsilon = f32::EPSILON as f64) || value >= 1.0;
    let min_alpha = approx::relative_eq!(alpha, 0.0, epsilon = f32::EPSILON as f64) || alpha <= 0.0;
    //let max_alpha = approx::relative_eq!(alpha, 1.0, epsilon = f32::EPSILON as f64) || alpha >= 1.0;

    // The ranges are tweaked by hand, because the perceived color might not always match slices of 30deg of the value
    // pie exactly.
    let hue_str = match hue {
        _ if min_saturated => pgettext(
            "used in string representation of the current selected color",
            "grey",
        ),
        v if v < 0.0 => pgettext("part of string representation of a color", "rose"),
        v if (0.0..40.0).contains(&v) => {
            pgettext("part of string representation of a color", "red")
        }
        v if (40.0..80.0).contains(&v) => {
            pgettext("part of string representation of a color", "orange")
        }
        v if (80.0..108.0).contains(&v) => {
            pgettext("part of string representation of a color", "yellow")
        }
        v if (108.0..120.0).contains(&v) => pgettext(
            "part of string representation of a color",
            "chartreuse-green",
        ),
        v if (120.0..150.0).contains(&v) => {
            pgettext("part of string representation of a color", "green")
        }
        v if (150.0..180.0).contains(&v) => {
            pgettext("part of string representation of a color", "spring-green")
        }
        v if (180.0..210.0).contains(&v) => {
            pgettext("part of string representation of a color", "cyan")
        }
        v if (210.0..240.0).contains(&v) => {
            pgettext("part of string representation of a color", "azure")
        }
        v if (240.0..280.0).contains(&v) => {
            pgettext("part of string representation of a color", "blue")
        }
        v if (280.0..315.0).contains(&v) => {
            pgettext("part of string representation of a color", "violet")
        }
        v if (315.0..345.0).contains(&v) => {
            pgettext("part of string representation of a color", "magenta")
        }
        v if v >= 345.0 => pgettext("part of string representation of a color", "rose"),
        _ => unreachable!(),
    };
    let saturation_str = match saturation {
        _ if min_saturated => "".to_string(),
        v if v < 0.25 => pgettext("part of string representation of a color", "greyish"),
        v if (0.25..0.50).contains(&v) => "".to_string(),
        v if (0.50..0.75).contains(&v) => {
            pgettext("part of string representation of a color", "strong")
        }
        v if v >= 0.75 => pgettext("part of string representation of a color", "vivid"),
        _ => unreachable!(),
    };
    let value_str = match value {
        v if v < 0.25 => pgettext("part of string representation of a color", "very-dark"),
        v if (0.25..0.50).contains(&v) => {
            pgettext("part of string representation of a color", "dark")
        }
        v if (0.50..0.75).contains(&v) => {
            pgettext("part of string representation of a color", "mid")
        }
        v if v >= 0.667 => pgettext("part of string representation of a color", "bright"),
        _ => unreachable!(),
    };
    let alpha_str = match alpha {
        v if v < 0.333 => pgettext("part of string representation of a color", "transparent"),
        v if (0.333..0.667).contains(&v) => {
            pgettext("part of string representation of a color", "translucent")
        }
        v if (0.667..1.0).contains(&v) => pgettext(
            "part of string representation of a color",
            "slightly-translucent",
        ),
        v if v >= 1.0 => "".to_string(),
        _ => unreachable!(),
    };

    if min_alpha {
        pgettext(
            "part of string representation of a color",
            "fully transparent",
        )
    } else if min_saturated && min_bright {
        pgettext("part of string representation of a color", "black")
    } else if min_saturated && max_bright {
        pgettext("part of string representation of a color", "white")
    } else {
        format!("{alpha_str} {saturation_str} {value_str} {hue_str}")
    }
}

pub(crate) fn path_walk_up_until_exists(path: impl AsRef<Path>) -> anyhow::Result<PathBuf> {
    let mut path = path.as_ref().absolutize()?.to_path_buf();
    while !path.exists() {
        path = path
            .parent()
            .filter(|p| p.parent().is_some())
            .map(|p| p.to_path_buf())
            .ok_or_else(|| anyhow::anyhow!("Path {} has no parent", path.display()))?;
    }
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn content_uris_compare_by_document_identity() {
        let note =
            gio::File::for_uri("content://com.example.documents/document/notes%2Fnote.rnote");
        let same =
            gio::File::for_uri("content://com.example.documents/document/notes%2Fnote.rnote");
        let other =
            gio::File::for_uri("content://com.example.documents/document/notes%2Fother.rnote");

        assert!(files_eq(&note, &same));
        assert!(!files_eq(&note, &other));
    }

    #[test]
    fn folder_display_uses_content_authority_without_a_parent() {
        assert_eq!(
            folder_display(&gio::File::for_uri(
                "content://com.example.documents/document/notes%2Fnote.rnote"
            )),
            Some("content://com.example.documents".to_string())
        );
    }

    #[test]
    fn folder_display_uses_the_parent_for_local_files() {
        let dir = std::env::temp_dir().join("rsnote-folder-display");
        let file = gio::File::for_path(dir.join("note.rnote"));

        assert_eq!(
            folder_display(&file),
            Some(dir.display().to_string().trim_end_matches('/').to_string())
        );
    }

    #[test]
    fn only_local_files_get_a_file_watcher() {
        let path = std::env::temp_dir().join("rsnote-watch.rnote");
        assert_eq!(
            local_watch_path(&gio::File::for_path(&path)),
            Some(path.clone())
        );
        assert_eq!(
            local_watch_path(&gio::File::for_uri(&format!("file://{}", path.display()))),
            Some(path)
        );
        assert_eq!(
            local_watch_path(&gio::File::for_uri(
                "content://com.example.documents/document/notes%2Fnote.rnote"
            )),
            None
        );
    }

    #[test]
    fn save_target_keeps_local_paths_and_nameless_documents() {
        let path = std::env::temp_dir().join("rsnote-save-target.rnote");
        assert_eq!(
            ensure_rnote_extension(gio::File::for_path(&path)).uri(),
            gio::File::for_path(&path).uri()
        );

        let content = gio::File::for_uri("content://com.example.documents/document/notes");
        assert_eq!(ensure_rnote_extension(content.clone()).uri(), content.uri());
    }

    #[test]
    fn files_eq_matches_path_and_uri_for_same_file() {
        let path = std::env::temp_dir().join("rsnote-files-eq-doc.rnote");
        let by_path = gio::File::for_path(&path);
        let by_uri = gio::File::for_uri(&format!("file://{}", path.display()));

        assert!(files_eq(&by_path, &by_uri));
        assert!(files_eq(&by_uri, &by_path));
    }

    #[test]
    fn files_eq_distinct_for_different_files() {
        let by_path = gio::File::for_path(std::env::temp_dir().join("rsnote-files-eq-a.rnote"));
        let by_uri = gio::File::for_uri("file:///tmp/rsnote-files-eq-b.rnote");

        assert!(!files_eq(&by_path, &by_uri));
    }

    #[test]
    fn workspace_dirs_resolve_from_paths_and_uris() {
        let path = std::env::temp_dir().join("rsnote-workspace-dir");
        let by_path = file_for_dir_str(&path.to_string_lossy());
        let by_uri = file_for_dir_str(&format!("file://{}", path.display()));

        assert!(files_eq(&by_path, &by_uri));

        let content_uri = "content://com.example.documents/tree/notes";
        assert_eq!(file_for_dir_str(content_uri).uri(), content_uri);
    }

    #[test]
    fn windows_drive_paths_are_not_uris() {
        let file = file_for_dir_str(r"C:\Users\rnote\notes");

        assert!(file.path().is_some());
        assert!(!file.uri().starts_with("c:"));
    }

    #[test]
    fn workspace_dir_identity_keeps_local_paths_and_remote_uris() {
        let path = std::env::temp_dir().join("rsnote-workspace-dir");
        assert_eq!(
            file_dir_identity(&gio::File::for_path(&path)),
            path.to_string_lossy()
        );

        let content_uri = "content://com.example.documents/tree/notes";
        assert_eq!(
            file_dir_identity(&gio::File::for_uri(content_uri)),
            content_uri
        );
    }

    #[test]
    fn workspace_dir_lists_entries_through_paths_and_uris() {
        let dir = std::env::temp_dir().join(format!("rsnote-list-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("note.rnote"), b"rnote").unwrap();

        for folder in [
            gio::File::for_path(&dir),
            gio::File::for_uri(&format!("file://{}", dir.display())),
        ] {
            glib::MainContext::new().block_on(async {
                let enumerator = folder
                    .enumerate_children_future(
                        "standard::name",
                        gio::FileQueryInfoFlags::NONE,
                        glib::Priority::DEFAULT,
                    )
                    .await
                    .unwrap();
                let entries = enumerator
                    .next_files_future(10, glib::Priority::DEFAULT)
                    .await
                    .unwrap()
                    .into_iter()
                    .map(|info| info.name())
                    .collect::<Vec<_>>();
                assert_eq!(entries, vec![PathBuf::from("note.rnote")]);
            });
        }

        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn missing_workspace_dir_lists_no_entries() {
        let folder = gio::File::for_uri(&format!(
            "file://{}/rsnote-missing-{}",
            std::env::temp_dir().display(),
            uuid::Uuid::new_v4()
        ));

        glib::MainContext::new().block_on(async {
            assert!(
                folder
                    .enumerate_children_future(
                        "standard::name",
                        gio::FileQueryInfoFlags::NONE,
                        glib::Priority::DEFAULT,
                    )
                    .await
                    .is_err()
            );
        });
    }

    #[test]
    fn unique_file_in_dir_avoids_collisions_for_paths_and_uris() {
        let dir = std::env::temp_dir().join(format!("rsnote-unique-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("note.rnote"), b"rnote").unwrap();
        std::fs::write(dir.join("note - 1.rnote"), b"rnote").unwrap();

        for folder in [
            gio::File::for_path(&dir),
            gio::File::for_uri(&format!("file://{}", dir.display())),
        ] {
            let (file, next) = unique_file_in_dir(&folder, "note", "rnote", 0);
            assert_eq!(file.basename().unwrap(), PathBuf::from("note - 2.rnote"));
            assert_eq!(next, 3);
        }

        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn unique_file_in_dir_uses_the_requested_file_extension() {
        let dir = std::env::temp_dir().join(format!("rsnote-unique-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("page-1.png"), b"png").unwrap();

        let (file, next) = unique_file_in_dir(&gio::File::for_path(&dir), "page-1", "png", 0);
        assert_eq!(file.basename().unwrap(), PathBuf::from("page-1 - 1.png"));
        assert_eq!(next, 2);

        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn file_location_label_uses_the_path_or_the_uri() {
        let path = std::env::temp_dir().join("rsnote-location.pdf");
        assert_eq!(
            file_location_label(&gio::File::for_path(&path)),
            path.to_string_lossy()
        );

        let content_uri = "content://com.example.documents/document/report.pdf";
        assert_eq!(
            file_location_label(&gio::File::for_uri(content_uri)),
            content_uri
        );
    }

    #[test]
    fn export_view_target_is_the_folder_for_local_files_and_the_file_for_content_uris() {
        let dir = std::env::temp_dir().join(format!("rsnote-view-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("report.pdf");

        let local_target = export_view_target(&gio::File::for_path(&file));
        assert!(files_eq(&local_target, &gio::File::for_path(&dir)));

        let content_file =
            gio::File::for_uri("content://com.example.documents/document/report.pdf");
        let content_target = export_view_target(&content_file);
        assert!(files_eq(&content_target, &content_file));

        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn create_replace_writes_and_reads_through_file_uri() {
        let path =
            std::env::temp_dir().join(format!("rsnote-write-{}.rnote", uuid::Uuid::new_v4()));
        let file = gio::File::for_uri(&format!("file://{}", path.display()));
        let contents = b"rnote test bytes".to_vec();

        gtk4::glib::MainContext::default().block_on(async {
            create_replace_file_future(contents.clone(), &file)
                .await
                .unwrap();
            let (bytes, _) = file.load_bytes_future().await.unwrap();
            assert_eq!(bytes.as_ref(), contents.as_slice());
        });

        std::fs::remove_file(&path).unwrap();
    }

    #[test]
    fn write_export_file_writes_the_prepared_bytes() {
        let path = std::env::temp_dir().join(format!("rsnote-export-{}.pdf", uuid::Uuid::new_v4()));
        let file = gio::File::for_uri(&format!("file://{}", path.display()));
        let contents = b"pdf test bytes".to_vec();

        gtk4::glib::MainContext::new().block_on(async {
            write_export_file(contents.clone(), &file).await.unwrap();
            let (bytes, _) = file.load_bytes_future().await.unwrap();
            assert_eq!(bytes.as_ref(), contents.as_slice());
        });

        std::fs::remove_file(&path).unwrap();
    }
}
