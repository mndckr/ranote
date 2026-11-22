//! App-internal clipboard fallback.
//!
//! GTK's Android backend only shares plain text, HTML and URI lists with the
//! system clipboard. Rnote's stroke content is a custom MIME type, so it is kept
//! here for paste inside the app while still being offered to GTK for the
//! platforms that can carry it.
#![cfg_attr(not(target_os = "android"), allow(dead_code))]

use rnote_engine::engine::StrokeContent;
use std::sync::{LazyLock, Mutex};

type ClipboardContent = Vec<(Vec<u8>, String)>;

#[derive(Debug, Default)]
struct InternalClipboard {
    content: Mutex<Option<ClipboardContent>>,
}

impl InternalClipboard {
    fn store(&self, content: ClipboardContent) {
        if let Ok(mut internal) = self.content.lock() {
            *internal = Some(content);
        }
    }

    fn stroke_content(&self) -> Option<Vec<u8>> {
        self.content.lock().ok()?.as_ref().and_then(|content| {
            content
                .iter()
                .find(|(_, mime_type)| mime_type == StrokeContent::MIME_TYPE)
                .map(|(data, _)| data.clone())
        })
    }

    fn clear(&self) {
        if let Ok(mut internal) = self.content.lock() {
            *internal = None;
        }
    }
}

static INTERNAL: LazyLock<InternalClipboard> = LazyLock::new(InternalClipboard::default);

/// Keep a copy of the last content copied in this app.
pub(crate) fn store(content: ClipboardContent) {
    INTERNAL.store(content);
}

/// The stroke content of the last copy, if it had any.
pub(crate) fn stroke_content() -> Option<Vec<u8>> {
    INTERNAL.stroke_content()
}

/// Forget the internal copy, e.g. when another app takes over the clipboard.
pub(crate) fn clear() {
    INTERNAL.clear();
}

/// Whether GTK's Android clipboard backend can hand a MIME type to other apps.
///
/// Upstream `gdkandroidclipboard.c` only serializes plain text, HTML and URI
/// lists; every other MIME type is dropped when claiming the system clipboard.
pub(crate) fn android_remote_supports(mime_type: &str) -> bool {
    matches!(
        mime_type,
        "text/plain" | "text/plain;charset=utf-8" | "text/html" | "text/uri-list"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn content(stroke_json: &[u8]) -> ClipboardContent {
        vec![
            (stroke_json.to_vec(), StrokeContent::MIME_TYPE.to_string()),
            (b"<svg/>".to_vec(), "image/svg+xml".to_string()),
            (b"png".to_vec(), "image/png".to_string()),
        ]
    }

    #[test]
    fn internal_clipboard_round_trips_stroke_content() {
        let clipboard = InternalClipboard::default();
        clipboard.store(content(br#"{"strokes":[]}"#));

        assert_eq!(
            clipboard.stroke_content(),
            Some(br#"{"strokes":[]}"#.to_vec())
        );
    }

    #[test]
    fn newer_copy_replaces_older_content() {
        let clipboard = InternalClipboard::default();
        clipboard.store(content(b"old"));
        clipboard.store(content(b"new"));

        assert_eq!(clipboard.stroke_content(), Some(b"new".to_vec()));
    }

    #[test]
    fn clear_forgets_content() {
        let clipboard = InternalClipboard::default();
        clipboard.store(content(b"strokes"));

        clipboard.clear();

        assert_eq!(clipboard.stroke_content(), None);
    }

    #[test]
    fn content_without_strokes_has_no_stroke_fallback() {
        let clipboard = InternalClipboard::default();
        clipboard.store(vec![(b"png".to_vec(), "image/png".to_string())]);

        assert_eq!(clipboard.stroke_content(), None);
    }

    #[test]
    fn android_remote_clipboard_only_supports_text_html_and_uris() {
        for mime_type in [
            "text/plain",
            "text/plain;charset=utf-8",
            "text/html",
            "text/uri-list",
        ] {
            assert!(android_remote_supports(mime_type), "{mime_type}");
        }
        for mime_type in [
            StrokeContent::MIME_TYPE,
            "image/svg+xml",
            "image/png",
            "image/jpeg",
        ] {
            assert!(!android_remote_supports(mime_type), "{mime_type}");
        }
    }
}
