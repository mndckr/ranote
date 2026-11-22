## Problem Statement

Android tablet users who keep their handwritten notes in rnote on Linux, Windows, or macOS cannot open or edit those `.rnote` files on Android. The project has no Android build, no device feedback, and no measured answer to the question upstream left open in #390: **does the existing GTK app actually run well on GTK's Android backend?**

The old blockers were real in February 2025 — only the cairo renderer was usable, text fields were broken, and the Rust bindings did not build for Android. Since then GTK 4.22+ has added GL rendering, IME work, and SDK 36 support, and libadwaita builds on Android. What is missing is not another discussion; it is a time-boxed experiment that either produces a reusable Android foundation or a precise, evidence-based reason to stop.

## Solution

Run a **six-week, production-shaped Android enablement spike** that builds the existing GTK app for Android, validates the hard gates, and leaves reusable build, CI, test, and platform-seam work behind.

The spike covers:

- a reproducible Android build of `rnote-ui` against GTK 4.22+ and libadwaita 1.9+;
- stylus fidelity: pressure, tilt, stylus buttons, eraser, palm rejection;
- canvas performance on real tablet GPUs;
- document open/save through Android's Storage Access Framework;
- workspace folder access, app lifecycle/autosave, IME, file association, clipboard, and a graceful no-print path;
- a device test matrix, a blocker log, an ADR, and a go/no-go report with measured thresholds.

It is **not** a feature-parity port and **not** a GUI rewrite. If the gates pass, the Android work continues on the foundation left by the spike. If they fail, the report names the blocker and recommends a separate Android client.

## User Stories

1. As an Android tablet user, I want to install an rnote APK, so that I can use rnote on my tablet without a Linux desktop.
2. As an Android tablet user, I want to open a `.rnote` file from my file manager, so that my existing notes are available on Android.
3. As an Android tablet user, I want to choose a `.rnote` file through the system file picker, so that I can open notes from local storage or a cloud provider.
4. As an Android tablet user, I want to save changes back to the same document, so that I do not lose work.
5. As an Android tablet user, I want to save a copy to a new location, so that I can duplicate or share a note.
6. As an Android tablet user, I want to create a new note, so that I can start writing immediately.
7. As a stylus user, I want pen pressure to affect stroke width, so that handwriting feels natural.
8. As a stylus user, I want tilt, barrel button, and eraser input to behave like they do on desktop, so that my tools are consistent.
9. As a stylus user, I want palm rejection while writing, so that my hand does not leave stray marks.
10. As a touch user, I want two-finger pan and zoom, so that I can navigate a page without switching tools.
11. As a user, I want the text tool to accept Android IME input, so that I can write in languages that need composition.
12. As a user, I want to open a workspace folder through the system picker, so that I can browse related notes.
13. As a user, I want drafts and autosave to survive backgrounding or a kill, so that I do not lose work.
14. As a user, I want rotation and resume to preserve the open document and view, so that orientation changes are safe.
15. As a user, I want to export a page or document to PDF or PNG, so that I can share it.
16. As a user, I want to copy and paste strokes and images, so that I can reuse content within rnote and in other apps.
17. As a user, I want rnote to follow the system dark mode, so that it matches my device.
18. As a user, I want my UI language to work, so that rnote is usable in my locale.
19. As a user, I want a large document to stay responsive, so that long note-taking sessions remain practical.
20. As a stylus user, I want low input-to-pixel latency, so that writing feels direct rather than laggy.
21. As a user, I want the Android back gesture and app pause not to discard work, so that navigation is safe.
22. As a user, I want printing to be hidden or clearly unavailable, so that I am not sent into a broken dialog.
23. As a screen-reader user, I want basic TalkBack labels for the main actions, so that core controls are discoverable.
24. As a user, I want `.rnote` files to appear in Android's "Open with" list, so that rnote is a first-class handler.
25. As a maintainer, I want a clean Android build from documented commands, so that contributors can reproduce it.
26. As a maintainer, I want CI to build an APK on every change, so that Android support does not silently bitrot.
27. As a maintainer, I want desktop behavior to remain unchanged, so that the existing Linux, Windows, and macOS users are not regressed.
28. As a maintainer, I want document and workspace identity to work for both filesystem paths and content URIs, so that SAF works without special-case hacks.
29. As a maintainer, I want Android-only behavior isolated in one capability module, so that platform differences are auditable.
30. As a maintainer, I want upstream patches to be minimal and documented, so that we can contribute them and drop them later.
31. As a maintainer, I want a blocker log with device evidence, so that the go/no-go decision is based on measurements rather than impressions.
32. As a tester, I want a device checklist and a log-collection script, so that results from different tablets are comparable.
33. As a maintainer, I want a go/no-go report with explicit thresholds, so that we can decide to continue or pivot.
34. As a maintainer, I want an ADR recording the Android decision, so that future work knows why we chose this path.
35. As a security reviewer, I want SAF-only file access and no broad storage permissions, so that user data is protected.
36. As a maintainer, I want debug instrumentation to be off by default, so that production logs do not leak note contents.
37. As a maintainer, I want the experiment artifacts to be reusable, so that a successful spike becomes the Android foundation rather than a throwaway prototype.
38. As a maintainer, I want a documented pivot plan, so that a failed experiment still produces useful work.
39. As a contributor, I want GTK/libadwaita Android build notes, so that I can reproduce the environment without rediscovering the toolchain.
40. As a maintainer, I want to coordinate with GTK Android and Xournal++ upstream, so that we do not duplicate fixes.

## Implementation Decisions

### Experiment shape

- The spike runs for **six weeks** on a dedicated branch, but all code is written as if it will be merged: no throwaway app, no debug-only shortcuts in production paths, no hardcoded device paths.
- Android-only code is compiled behind `target_os = "android"` and isolated in a small platform capability module. Desktop code paths must remain behaviourally identical.
- Any refactor that is useful on desktop (file identity and IO) lands independently of the Android gates and is covered by tests.
- The spike ends with a written go/no-go report, even if it succeeds.

### Build and packaging

- Build against a pinned GTK release that contains the Android GL and IME fixes (GTK 4.22 or newer; prefer the latest stable at the time of the spike) and libadwaita 1.9 or newer. Record the exact versions and commits.
- Use GTK's Android build integration (meson cross file, NDK toolchain, Gradle wrapper) rather than inventing a parallel build system. Follow GTK's Android application template for the entry point and activity wiring.
- Pin the Android NDK, Rust target, Gradle, and JDK versions. Keep the Android toolchain reproducible in a container or documented script.
- Add `just` recipes for setup, build, install, logcat, and device-check so that a new contributor can reproduce the flow.
- Add a CI job that builds the APK/AAB on Linux. Device tests are not run in CI.

### Rendering and input

- Force the GL renderer on Android (`GSK_RENDERER=gl`). Do not rely on the cairo renderer and do not add a cairo fallback; if GL cannot initialize, that is a gate failure with logs.
- Keep the existing `RnCanvas` and engine rendering path. Do not introduce a second renderer.
- Use `GdkDeviceTool` and GDK axes for stylus detection, pressure, tilt, and buttons, as the canvas already does on desktop. If the Android backend does not expose the required tool/axis data, record it as a blocker; do not fake pressure.
- Test touch gestures against the existing canvas behavior. Do not add a separate gesture stack.
- Tablet-first: keep the existing adaptive UI, but fix touch-only blockers (hit target size, hover-only affordances). Phone-specific layout is out of scope.

### File and workspace access

- **Primary seam: `gio::File`.** Make document and workspace files first-class `gio::File` values with URI-or-path identity instead of assuming `PathBuf` and `file.path()`.
  - Document tab identity should compare stable file identity (URI or canonical path), not `PathBuf`.
  - Saving must work through `gio::File` async replace operations for both local paths and content URIs.
  - Loading should take a `gio::File`, not a path.
  - Workspace listing should use `gio::File` directories, including SAF tree URIs where GTK/GIO supports them.
  - Drafts and app-private files remain local paths under the Android app cache.
- If a content URI cannot be enumerated by `DirectoryList`, use a small SAF bridge behind the platform capability module. Do not request broad storage permissions.
- Keep "save as" behaviour and filename collision handling working for both paths and URIs.
- Do not add a broad platform trait if the `gio::File` seam is sufficient. Keep platform-specific capability checks in one module.

### Lifecycle, capabilities, and platform integration

- Add one small platform capability module for: `can_print`, `supports_folder_picker`, `on_pause`/`on_resume`, `open_external`, and launch-intent handling.
- Wire Android activity pause/stop to draft saving and state flushing. Test a kill/restore cycle.
- Hide or disable printing on Android. Do not ship a broken print dialog.
- Handle Android "Open with" intents for `.rnote` files and route them through the existing application-open path.
- Reuse GTK's clipboard and IME where they work; fall back to app-internal clipboard only if a format is unsupported. Record any format gaps.
- Resolve app data, cache, locale, and settings directories for Android in the existing environment module. Bundle `.mo` files and the GSettings schema; if GSettings is not usable on Android, use a small app-private settings fallback behind the platform module.
- Disable pen sounds on Android for the spike if the audio backend blocks the build; record it as a known gap.

### Dependency and build gaps

- `poppler` is no longer a concern because PDF rendering uses the pure-Rust `hayro` path; keep it that way.
- If `librsvg` blocks the Android cross-build, replace the SVG import path with the existing `usvg`/pure-Rust path rather than forking a C dependency.
- Resolve `gettext` by bundling `libintl` or using a pure-Rust lookup behind the i18n module. Do not fork `gettext-rs` unless no smaller option exists.
- Apply the smallest documented workaround for `gtk-rs` Android builds. Upstream the workaround if it is clean; otherwise keep it in one build script with a comment.
- Do not add new runtime dependencies for the spike unless the existing stack cannot do the job.

### Observability and reuse

- Add debug-only instrumentation for frame timings, input-to-pixel latency, pressure samples, and memory. It must be off by default and must never log note contents.
- Add a device-check script that collects app version, device/GPU, Android version, frame stats, memory, and filtered logcat.
- Write `docs/android/` build notes, a device matrix, a blocker log with upstream links, and a go/no-go report.
- Write an ADR for the Android decision. No ADR exists for platform integration today; this spike creates the first one.
- Keep upstream patches in a clearly named directory or branch, each linked to a blocker and a minimal reproduction. Submit them to GTK, gtk-rs, or libadwaita where appropriate.
- Coordinate with the GTK Android maintainers and the Xournal++ GTK Android effort to avoid duplicate work.

### Go/no-go thresholds

The spike passes only if:

- A stylus is detected as a pen, pressure varies across a meaningful range, and barrel/eraser buttons work on at least two physical tablets with different GPUs.
- A 20-page document scrolls and draws at **≥55 fps median** and **p95 frame time ≤33 ms** on the reference mid-range tablet, with no OOM in a 30-minute session.
- A `.rnote` file can be opened, edited, and saved through SAF without data loss, including a content URI from a cloud provider.
- The Android build is reproducible from a clean checkout and CI produces an APK.
- Desktop tests and a desktop smoke run remain green.

The spike stops early and reports a no-go if:

- pressure is flat/default or no stylus tool is exposed on both reference devices;
- the canvas stays below 30 fps after basic tuning;
- SAF folder access requires broad storage permissions or an unreasonable custom bridge;
- the build requires a large GTK/gtk-rs fork rather than a small patch set.

## Testing Decisions

### Seams (please confirm before implementation)

- **Primary seam: the existing `gio::File` document/workspace boundary.** Tests use local paths, `file://` URIs, and (on Android) content URIs through the same interface. This is an existing type, not a new abstraction.
- **Secondary seam: one small platform capability module** (`can_print`, folder-picker support, pause/resume, launch intents). Tests use a fake capability set to verify that the UI hides or disables the right actions.
- No broad `PlatformHost` trait is planned. If the file seam cannot cover workspace/lifecycle needs without leaking platform checks across the UI, stop and revise the seam before writing more code.

### What makes a good test

Good tests assert externally visible behaviour: a file opens, bytes round-trip, a tab is reused, a capability hides an action, a pause saves a draft, a document renders. They do not assert which GTK widget or private method was called.

### Unit tests

- File identity: the same local path and the same `file://` URI resolve to the same document; different files do not.
- Document write: saving through the `gio::File` seam writes bytes for both a local path and a `file://` URI, with no `path()` requirement.
- Workspace listing: a `gio::File` directory yields the expected entries; a missing/unreadable directory fails gracefully.
- Capability gating: print and folder-picker actions are hidden or disabled when the capability set says so.
- Environment selection: Android app data/cache/locale paths are chosen correctly; desktop paths are unchanged.
- Filename collision handling still produces unique names for path-based and URI-based saves.

### Integration tests

- Round-trip a small `.rnote` document through open → edit → save → reopen using the file seam.
- Existing engine file-format compatibility tests remain unchanged and green.
- Desktop smoke test using the existing live-poking flow, covering open, draw, save, and close.
- Android device acceptance checklist covering open/save, pressure, buttons, pan/zoom, IME, workspace, pause/resume, rotation, export, clipboard, dark mode, and file association.

### Device tests

- At least two physical tablets: one Mali GPU and one Adreno GPU, different Android versions, both with an active stylus.
- Use the device-check script to capture logs, frame timings, memory, and pressure samples.
- Record pass/fail and measurements in the experiment report. A device test is not "it launched"; it must exercise the gate.

### CI

- Add an Android build job that produces an APK artifact. No emulator-based stylus tests.
- Keep the existing desktop CI job unchanged.
- Fail the Android job on build errors, not on device-gate results.

### Prior art

- Unit tests in the camera and workspace duplicate-file modules.
- The engine's file-format compatibility suite (`test-file-compatibility`).
- The existing nextest workspace run and live-poking smoke flow.

## Out of Scope

- Full feature parity with desktop rnote.
- Phone-specific layout and one-handed UI.
- iOS, Windows, or macOS changes.
- Play Store or F-Droid release.
- Printing on Android.
- Cloud sync, collaboration, AI features, or multi-window.
- A full accessibility audit; only basic TalkBack labels for core actions.
- Rewriting the renderer or porting to Slint, Tauri, Flutter, or Qt.
- Taking over GTK Android maintenance. Upstream only the specific patches rnote needs.
- Performance optimization beyond the gate; fix the gate first.

## Further Notes

- Upstream rnote discussion: [#390](https://github.com/flxzt/rnote/issues/390), including the February 2025 assessment that blocked on cairo-only rendering, broken text fields, and Rust bindings.
- GTK Android work: [MR !7555](https://gitlab.gnome.org/GNOME/gtk/-/merge_requests/7555), GL/IME and SDK 36 changes in GTK 4.22+, Vulkan WSI [MR !10050](https://gitlab.gnome.org/GNOME/gtk/-/merge_requests/10050) (optional for this spike).
- gtk-rs Android build: [#1997](https://github.com/gtk-rs/gtk4-rs/issues/1997).
- libadwaita Android: [MR !1709](https://gitlab.gnome.org/GNOME/libadwaita/-/merge_requests/1709).
- Xournal++ is exploring the same backend: [#5892](https://github.com/xournalpp/xournalpp/issues/5892).
- Security: use SAF only; do not request `MANAGE_EXTERNAL_STORAGE` or broad storage permissions. Do not log document contents.
- Reuse contract: if the spike succeeds, the file seam, platform capability module, build scripts, CI job, docs, tests, and upstream patches are the foundation for the Android port. If it fails, merge only the safe desktop refactors and docs, keep the Android branch for reference, and follow the pivot plan in the report.
- Pivot plan: if pressure or performance fails, build a separate Android client that shares the `.rnote` format and optionally the engine, rather than rewriting the desktop GUI.
