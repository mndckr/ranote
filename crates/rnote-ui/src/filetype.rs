// Imports
use gtk4::{gio, prelude::*};
use std::path::Path;
use tracing::warn;

/// File types supported by Rnote.
#[derive(Debug, PartialEq, PartialOrd)]
pub(crate) enum FileType {
    Folder,
    RnoteFile,
    VectorImageFile,
    BitmapImageFile,
    XoppFile,
    PdfFile,
    PlaintextFile,
    Unsupported,
}

impl FileType {
    pub(crate) fn lookup_file_type(file: &gio::File) -> Self {
        if let Ok(info) = file.query_info(
            "standard::*",
            gio::FileQueryInfoFlags::NONE,
            None::<&gio::Cancellable>,
        ) {
            if let Some(file_type) = Self::from_file_info(&info) {
                return file_type;
            }
        } else {
            warn!("Looking up file type failed, failed to query FileInfo from file `{file:?}`.");
        }

        // Android content files report no standard::type and an opaque MIME type,
        // so the extension is the only thing left that identifies them.
        match file.basename() {
            Some(basename) => Self::from_name(&basename).unwrap_or(Self::Unsupported),
            None => {
                warn!("Looking up file type failed, no basename for file `{file:?}`.");
                Self::Unsupported
            }
        }
    }

    /// Classify from file info. `None` means the info says nothing useful and the
    /// name has to decide.
    fn from_file_info(info: &gio::FileInfo) -> Option<Self> {
        match info.file_type() {
            gio::FileType::Directory => Some(Self::Folder),
            // Android SAF files and folders report their type as unknown.
            gio::FileType::Regular | gio::FileType::Unknown => info
                .content_type()
                .and_then(|content_type| Self::from_content_type(content_type.as_str())),
            file_type => {
                warn!("Looking up file type failed, unsupported file type `{file_type:?}`");
                Some(Self::Unsupported)
            }
        }
    }

    fn from_content_type(content_type: &str) -> Option<Self> {
        match content_type {
            "application/rnote" => Some(Self::RnoteFile),
            "vnd.android.document/directory" => Some(Self::Folder),
            "image/svg+xml" => Some(Self::VectorImageFile),
            "image/png" | "image/jpeg" => Some(Self::BitmapImageFile),
            "application/x-xopp" => Some(Self::XoppFile),
            "application/pdf" => Some(Self::PdfFile),
            "text/plain" => Some(Self::PlaintextFile),
            _ => None,
        }
    }

    fn from_name(name: &Path) -> Option<Self> {
        match name.extension()?.to_string_lossy().as_ref() {
            "rnote" => Some(Self::RnoteFile),
            "svg" => Some(Self::VectorImageFile),
            "jpg" | "jpeg" | "png" => Some(Self::BitmapImageFile),
            "xopp" => Some(Self::XoppFile),
            "pdf" => Some(Self::PdfFile),
            "txt" => Some(Self::PlaintextFile),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn info(file_type: gio::FileType, content_type: Option<&str>) -> gio::FileInfo {
        let info = gio::FileInfo::new();
        info.set_file_type(file_type);
        if let Some(content_type) = content_type {
            info.set_content_type(content_type);
        }
        info
    }

    #[test]
    fn android_content_files_are_classified_without_standard_type() {
        // GdkAndroidContentFile only reports a MIME type, never standard::type.
        assert_eq!(
            FileType::from_file_info(&info(gio::FileType::Unknown, Some("application/rnote"))),
            Some(FileType::RnoteFile)
        );
        assert_eq!(
            FileType::from_file_info(&info(gio::FileType::Unknown, Some("application/pdf"))),
            Some(FileType::PdfFile)
        );
    }

    #[test]
    fn android_content_folders_are_folders() {
        assert_eq!(
            FileType::from_file_info(&info(
                gio::FileType::Unknown,
                Some("vnd.android.document/directory")
            )),
            Some(FileType::Folder)
        );
    }

    #[test]
    fn unknown_content_type_defers_to_the_name() {
        assert_eq!(
            FileType::from_file_info(&info(
                gio::FileType::Unknown,
                Some("application/octet-stream")
            )),
            None
        );
        assert_eq!(
            FileType::from_file_info(&info(gio::FileType::Unknown, None)),
            None
        );
        assert_eq!(
            FileType::from_name(Path::new("note.rnote")),
            Some(FileType::RnoteFile)
        );
    }

    #[test]
    fn special_files_are_unsupported() {
        assert_eq!(
            FileType::from_file_info(&info(gio::FileType::Special, None)),
            Some(FileType::Unsupported)
        );
    }

    #[test]
    fn names_classify_by_extension() {
        for (name, expected) in [
            ("note.rnote", FileType::RnoteFile),
            ("drawing.svg", FileType::VectorImageFile),
            ("scan.png", FileType::BitmapImageFile),
            ("sheet.xopp", FileType::XoppFile),
            ("paper.pdf", FileType::PdfFile),
            ("readme.txt", FileType::PlaintextFile),
        ] {
            assert_eq!(FileType::from_name(Path::new(name)), Some(expected));
        }
        assert_eq!(FileType::from_name(Path::new("note")), None);
    }
}
