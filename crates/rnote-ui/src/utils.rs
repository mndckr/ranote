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

/// Whether two [`gio::File`]s refer to the same document.
///
/// Compares the file identity GIO derives from the URI, so a local path and its
/// `file://` URI match, and no `PathBuf` or `path()` is assumed.
pub(crate) fn files_eq(first: &gio::File, second: &gio::File) -> bool {
    first.equal(second)
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

/// Find a non-existing `.rnote` file in `folder` named after `base_title`,
/// appending the duplicate suffix as needed.
///
/// Returns the file and the next free duplicate index, so callers can keep names
/// unique across several unsaved documents.
pub(crate) fn unique_file_in_dir(
    folder: &gio::File,
    base_title: &str,
    start_index: u32,
) -> (gio::File, u32) {
    let mut index = start_index;
    loop {
        let name = if index == 0 {
            format!("{base_title}.rnote")
        } else {
            format!("{base_title}{FILE_DUP_SUFFIX_DELIM}{index}.rnote")
        };
        let candidate = folder.child(&name);

        if !candidate.query_exists(gio::Cancellable::NONE) {
            return (candidate, index + 1);
        }

        index += 1;
    }
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
            let (file, next) = unique_file_in_dir(&folder, "note", 0);
            assert_eq!(file.basename().unwrap(), PathBuf::from("note - 2.rnote"));
            assert_eq!(next, 3);
        }

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
}
