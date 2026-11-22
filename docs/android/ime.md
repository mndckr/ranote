# Canvas text tool and Android IME (ticket #13)

The canvas typewriter tool accepts Android IME composition and commit by using
GTK's Android IM context. No parallel input stack and no file-format change.

## How it works

- The canvas already owns a `GtkIMMulticontext` on its key controller and inserts
  committed text through the typewriter pen (`crates/rnote-ui/src/canvas/input.rs`).
- GTK 4.22's Android IM context never draws the composition itself. It only emits
  `preedit-changed` / `preedit-end` / `commit`
  (`gtk/gtkimcontextandroid.c`). Desktop IM modules (IBus, Wayland text-input,
  Windows IME) draw their composition in their own popup, so the app must not
  draw it again.
- On Android only, the canvas subscribes to `preedit-changed` and the engine draws
  the composition at the typewriter caret, underlined, with the same text style
  (`Typewriter::preedit_placement` / `draw_preedit`). The composition is
  **display-only**: it is never inserted into the document, undo history or the
  saved file. A commit inserts the committed string exactly as the IME delivers
  it, through `PenEvent::Text`.
- The composition is cleared on commit, cancel and when the typewriter is
  deinitialised.
- Desktop is unchanged: `platform::app_draws_ime_preedit()` is `false` off
  Android, so the canvas does not connect the signal and desktop IME popup
  behaviour is untouched.

Format: text strokes store UTF-8 strings, so non-Latin input needs no format
change. The new round-trip test saves and reloads CJK, Arabic and a ZWJ emoji
sequence.

## Verified in this environment

No SDK/NDK, Android Rust targets, emulator or device were available, so these are
the only checks that could run:

```sh
# host cargo check / clippy (config.rs generated locally, not committed)
cargo check -p rnote
cargo clippy -p rnote-engine --all-targets
cargo clippy -p rnote --all-targets

# engine tests
cargo test -p rnote-engine --lib
#   ime_composition_does_not_modify_the_document
#   ime_commit_inserts_non_latin_text_exactly_once
#   ime_preedit_lays_out_and_draws
#   non_latin_text_strokes_survive_save_and_load

# desktop unchanged
cargo test -p rnote --bin rnote platform::
```

`cargo check -p rnote` still has the pre-existing `Capabilities::ANDROID is never
used` warning on desktop; it predates this ticket and is not introduced here.

## Device / emulator verification steps

Prerequisites: the pinned toolchain from
[`misc/android/README.md`](../../misc/android/README.md) (`just android-app-setup`),
an emulator or tablet with the desired keyboards installed (Gboard + a non-Latin
language pack such as Japanese/Chinese/Korean).

1. Build, install, run, watch logs:

   ```sh
   just android-app-setup
   just android-app-build
   just android-app-install
   just android-app-logcat
   ```

   `ImContext.ImeConnection` logs `IME: setComposingText(...)`,
   `IME: finishComposingText()` and `IME: commitText(...)` at INFO. If no
   composition events appear, confirm the android IM module was selected by
   looking for `No IM module matching GTK_IM_MODULE` and by running GTK with
   `GTK_DEBUG=modules`.

2. Select the typewriter tool, tap the canvas (keyboard must appear), and test:

   - Gboard Latin with prediction/autocorrect: compose, pick a suggestion, commit.
     Characters must arrive once, in order.
   - Gboard `日本語`: type `にほん`, the underlined composition must be visible on
     the canvas before commit, then `日本` must be inserted once.
   - Chinese pinyin and Korean Hangul composition and commit.
   - Emoji keyboard (including a ZWJ family sequence) and a right-to-left language.
   - Compose in the middle of an existing text stroke and commit (the inline
     composition overlays the tail before commit; see gap 4).
   - Cancel the composition / press Back mid-composition: nothing is inserted.
   - Switch to the pen tool and back: the keyboard hides/shows, no stale
     composition remains.
   - Rotate or background the app while composing: record whether the composition
     (not committed text) survives.
3. Save through SAF, close the document, reopen it: all committed non-Latin text
   must be intact.
4. Repeat on a second keyboard brand (e.g. Samsung Keyboard) with autocorrect on.

## Known gaps and blockers

| # | Gap | Evidence | Status |
|---|---|---|---|
| 1 | GTK's Android activity forces `outAttrs.inputType = InputType.TYPE_NULL`; `ImContext.getInputType()` is commented out. IMEs see a non-text editor, so some keyboards may send raw key events instead of composing. rnote cannot override this from `GtkIMContext` (`set_input_purpose`/`set_input_hints` only feed the unused `getInputType`). | GTK 4.22.5 `gdk/android/glue/java/org/gtk/android/ToplevelActivity.java` (`onCreateInputConnection`), `ImContext.java`. Needs a small GTK patch (`TYPE_CLASS_TEXT \| TYPE_TEXT_FLAG_MULTI_LINE \| TYPE_TEXT_FLAG_IME_MULTI_LINE`) and upstreaming to GTK MR [!7555](https://gitlab.gnome.org/GNOME/gtk/-/merge_requests/7555). | **Blocker for reliable non-Latin composition**; verify per keyboard on device. |
| 2 | Surrounding text is not wired to the IM context: `ImContext.getSurrounding()` exists as a native method but `ImeConnection` never calls it; `deleteSurroundingText()` deletes by synthesizing Backspace key events. | GTK 4.22.5 `ImContext.java`. Japanese reconversion and whole-word autocorrect deletion cannot work; single-character backspace works through rnote's key handling. | Gap; does not block basic compose/commit. |
| 3 | Cursor location is not propagated to the Android IME (the Android `GtkIMContext` does not use `gtk_im_context_set_cursor_location`), so the candidate bar / extract mode may not follow the caret. | `gtk/gtkimcontextandroid.c` implements no cursor-location handling. | Verify on device; cosmetic until then. |
| 4 | The composition is drawn inline at the caret without reflowing following text, so composing in the middle of an existing stroke overlaps the tail until commit. Composition at the end of a stroke (the common case) is correct. The preedit never persists. | `Typewriter::draw_preedit`. | App-side visual limitation; acceptable for the spike. |
| 5 | `commitText()` commits the whole editable content after `setComposingText()`; possible duplicated composing text is unverified. | GTK 4.22.5 `ImContext.java`. | Device checklist item (step 2). |

References: GTK [MR !7555](https://gitlab.gnome.org/GNOME/gtk/-/merge_requests/7555)
(Android GL/IME, SDK 36), gtk-rs Android build
[#1997](https://github.com/gtk-rs/gtk4-rs/issues/1997).
