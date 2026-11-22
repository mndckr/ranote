# Clipboard on Android

## What GTK's Android backend can carry

Upstream (`gdk/android/gdkandroidclipboard.c`) serializes only these MIME types
when rnote claims the system clipboard:

- `text/plain` / `text/plain;charset=utf-8`
- `text/html`
- `text/uri-list`

On read, it decodes text and HTML directly, and decodes any other MIME type when
the ClipData item is a content URI whose provider reports that same MIME type.
That covers image paste from apps that put an image content URI on the
clipboard (Gallery, screenshot tools, browsers).

## Rnote behaviour

- Copy/cut still build the full payload (stroke content JSON, SVG, PNG) and give
  it to `GdkClipboard`.
- On Android, when the payload contains a format the system clipboard cannot
  carry, the same payload is also kept in an app-internal slot
  (`crates/rnote-ui/src/clipboard.rs`). Paste inside rnote reads the stroke
  content from that slot, so internal copy/cut/paste does not depend on the
  Android clipboard backend.
- The internal slot is cleared when the clipboard changes and no stroke content
  is advertised, so another app's copy cannot paste stale rnote strokes.
- Desktop is unchanged: the internal slot is not compiled and GTK carries every
  format.

## Format gaps (blockers for cross-app copy)

| Gap | Effect | Upstream |
| --- | --- | --- |
| Outgoing `application/rnote-stroke-content` is not serialized | Strokes cannot be copied from rnote into another app | [gdkandroidclipboard.c](https://gitlab.gnome.org/GNOME/gtk/-/blob/gtk-4-22/gdk/android/gdkandroidclipboard.c) (`gdk_android_clipboard_clipdata_from_provider_async`) |
| Outgoing `image/svg+xml` and `image/png` are not serialized | A stroke selection cannot be copied as an image into another app | same |
| Unsupported outgoing formats log `No compatible clipboard transfer format found` | The system clipboard keeps its previous content; rnote's local paste still works through the internal slot | same |
| Incoming images require a content URI with an exact MIME match | Apps that put image bytes in `text/html`/`text/plain`, or report `image/*`, cannot be pasted | same (`gdk_android_clipdata_read_async`) |

Cross-app copy needs an upstream GTK change (serialize image data as a URI or
add an image ClipData path) or an Android-side share intent. Both are out of
scope for this ticket.

## Device verification (pending)

No Android device was available for this change; the Android target was only
type-checked. Run on a device:

1. Copy a stroke selection (`Ctrl+C`), open another note, paste (`Ctrl+V`).
   Expected: strokes appear at the paste position.
2. Cut a selection, paste it into another note. Expected: strokes move.
3. Copy an image in Gallery or a screenshot tool, paste in rnote. Expected: the
   bitmap is inserted.
4. Copy text in another app, paste in rnote. Expected: a text stroke.
5. Copy strokes in rnote, then copy something in another app, then paste in
   rnote. Expected: the other app's content, not stale strokes.
6. Copy strokes in rnote and paste in another app. Expected: nothing usable;
   record whether the app shows an error. This is the known GTK gap above.

Capture results in the spike blocker log with `just android-app-logcat`.
