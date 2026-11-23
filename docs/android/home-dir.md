# The `$HOME`-equivalent on Android

Status: research note, no code change. Written because rnote's Android build has
three call sites that need a home directory and fail today.

Where it fails (`crates/rnote-ui/src/env.rs`):

- `android_dirs()` line 159 — `env::home_dir().context("Unable to retrieve home dir")`;
  this feeds `pkg_data_dir()` (46-50), `app_cache_dir()` (212-214) and therefore
  `drafts_dir()` (231).
- `cache_dir()` line 97 — desktop path, dead code on Android.
- The default workspace dir falls back through `pkg_data_dir()`
  (`crates/rnote-ui/src/workspacebrowser/workspacesbar/workspacelistentry.rs:44-60`,
  Android branch 49-54).

Why it is `None` rather than some fallback: Rust's Unix `home_dir()` only reads
`$HOME` on Android and deliberately skips the passwd lookup
(`std` 1.89, `library/std/src/sys/pal/unix/os.rs:635-651`: the `fallback()`
implementation for `target_os = "android"` is `None`). GLib, by contrast, never
fails: `g_build_home_dir()` tries `$HOME`, then the user database, then warns and
returns `"/"` (`misc/android/app/subprojects/glib/glib/gutils.c:836-897`).

## 1. Platform storage facts

`ContextImpl` builds every path from `getDataDir()` (the app's `/data/user/<id>/<pkg>`)
and `Environment` builds the external ones
(`frameworks/base/core/java/android/app/ContextImpl.java:850-931`,
`frameworks/base/core/java/android/os/Environment.java:218-233`):

| API | Real path | Notes |
| --- | --- | --- |
| `getFilesDir()` | `<dataDir>/files` (`ContextImpl.java:850-857`) | app-private, no permission needed, `0700` |
| `getNoBackupFilesDir()` | `<dataDir>/no_backup` (`ContextImpl.java:876-883`) | same as files, excluded from Auto Backup |
| `getCacheDir()` | `<dataDir>/cache` (`ContextImpl.java:918-926`) | system deletes on low disk |
| `getExternalFilesDir()` | `<external>/Android/data/<pkg>/files` (`Environment.java:231-233`) | may return `null`; media volume |

Facts per directory:

- **Internal app-specific storage** – "The system prevents other apps from
  accessing these locations, and on Android 10 (API level 29) and higher, these
  locations are encrypted" — [app-specific storage](https://developer.android.com/training/data-storage/app-specific).
  Same page: "these directories tend to be small. Before writing app-specific
  files to internal storage, your app should query the free space".
- **Cache eviction** – `getCacheDir()` javadoc: "The system will automatically
  delete files in this directory as disk space is needed elsewhere on the device.
  The system will always delete older files first, as reported by
  `File.lastModified()`" (`frameworks/base/core/java/android/content/Context.java:1722-1758`).
  The guide adds: "When the device is low on internal storage space, Android may
  delete these cache files" and "you shouldn't rely on the system to clean up
  these files for you". Global clearing is also user-triggerable via
  `ACTION_CLEAR_APP_CACHE` ("clear all caches", [Android 11 storage](https://developer.android.com/about/versions/11/privacy/storage)).
- **External app-specific storage** – no storage permission needed since KITKAT,
  content is "internal to the applications, and not typically visible to the user
  as media", and it is "automatically created for you"
  (`Context.java:1463-1547`). It can be unavailable: "files in these directories
  aren't always accessible, such as when a removable SD card is removed"
  (app-specific guide). Since Android 11 apps "can no longer access files in *any*
  other app's dedicated, app-specific directory" in external storage
  ([Android 11 storage](https://developer.android.com/about/versions/11/privacy/storage)),
  and SAF pickers can no longer select `Android/data/` or `Android/obb/` at all
  (same page), plus "apps cannot create their own app-specific directory on
  external storage" (app-specific guide). `getExternalFilesDir()` "may return
  `null` if shared storage is not currently available" (`Context.java:1537-1539`).
- **Uninstall** – for every app-specific location: "the files saved in
  app-specific storage are removed" / "deleted when the application is
  uninstalled" (app-specific guide; `Context.java:1468-1470`). Nothing in the
  table survives uninstall.
- **Path stability** – every one of them is documented as "The returned path may
  change over time if the calling app is moved to an adopted storage device, so
  only relative paths should be persisted" (`Context.java:1380-1390` for
  `getDataDir`/`getFilesDir`, 1447-1451 for `no_backup`, 1752-1755 for cache,
  1497-1500 for external: "may change … if different shared storage media is
  inserted"). So the base path must be resolved at each launch, not stored.
- **Backup** – Auto Backup includes `getFilesDir()` and `getExternalFilesDir()`,
  and "excludes files in directories returned by `getCacheDir()`,
  `getCodeCacheDir()`, and `getNoBackupFilesDir()`"
  ([Auto Backup](https://developer.android.com/identity/data/autobackup)). The
  pixiewood manifest sets `android:allowBackup="true"`
  (`~/.cache/ranote-android/pixiewood/generate/manifest.xsl:20`), quota 25 MB.

One Android-specific hazard for *this* app: GTK's Android runtime wipes
`getFilesDir()` wholesale when the packaged-asset fingerprint changes, i.e. on an
app update (`misc/android/app/subprojects/gtk/gdk/android/glue/java/org/gtk/android/SystemFilesystem.java:42-47,81-88,91-111`:
`cleanDirectory(context.getFilesDir())` before re-copying assets). Anything rnote
keeps under `files/` is deleted by the GTK runtime on an update that ships new
assets, without rnote being involved.

## 2. What ported desktop apps use

### GTK's Android runtime (what rnote actually links)

Before the app's `main` is entered, the runtime maps XDG dirs and sets no `HOME`
(`misc/android/app/subprojects/gtk/gdk/android/gdkandroidruntime.c:259-281`):

```c
jobject files_dir = ... getFilesDir ...
configdir = files_path/etc;   datadir = files_path/share;
jobject ext_files_dir = ... getExternalFilesDir(null) ...
userconfigdir = ext_files_path/etc;  userdatadir = ext_files_path/share;

g_set_user_dirs ("XDG_CONFIG_DIRS", config_dirs,
                 "XDG_DATA_DIRS",   data_dirs,
                 "XDG_CONFIG_HOME", userconfigdir,
                 "XDG_DATA_HOME",   userdatadir,
                 NULL);
```

So: `XDG_DATA_HOME`/`XDG_CONFIG_HOME` → `Android/data/<pkg>/files/{share,etc}`,
`XDG_DATA_DIRS`/`XDG_CONFIG_DIRS` → `files/{share,etc}`, `HOME` untouched, no
`XDG_CACHE_HOME`, no `XDG_STATE_HOME`, no `XDG_RUNTIME_DIR`. Consequence in
rnote today: `pkg_data_dir()` (which reads `XDG_DATA_HOME`) would resolve, but the
`home_dir()` call that precedes it aborts first, and drafts/settings follow the
`$HOME/.cache` fallback. rnote's GSettings keyfile backend therefore writes under
`Android/data/<pkg>/files/etc` (`crates/rnote-ui/src/env.rs:305-313` sets
`GSETTINGS_BACKEND=keyfile`).

Note `g_set_user_dirs()` can set `HOME` (`glib/gutils.c:1815,1853-1854`), but it
is private API: declared only in `glib/gutilsprivate.h:35`, and pixiewood's
`prepare/wraps/glib/hack.patch` exists solely to add `GLIB_AVAILABLE_IN_ALL` so
that this call links. GTK's call simply does not pass `"HOME"`.

### GLib resolution order

`g_build_home_dir()`: `$HOME` if set (no existence check on Unix) → passwd
(`g_get_user_database_entry()`) → warning + `"/"`
(`misc/android/app/subprojects/glib/glib/gutils.c:836-897`). There is no `G_HOME`
variable anywhere in GLib (`rg G_HOME` over `glib/*.c glib/*.h` matches only
`XDG_*_HOME`). `g_get_user_data_dir()` = `$XDG_DATA_HOME` else
`$HOME/.local/share` (`gutils.c:1922-1949`); cache = `$XDG_CACHE_HOME` else
`$HOME/.cache` (`gutils.c:2059-2103`); state = `$XDG_STATE_HOME` else
`$HOME/.local/state`. All three are cached on first use, so the environment must
be set before any GLib call — which is why GTK does it inside the JNI runtime
before `g_module_open`-ing the app.

### Rust `std`

On `target_os = "android"` the passwd fallback is stubbed to `None`, so
`std::env::home_dir()` == `$HOME` (`library/std/src/sys/pal/unix/os.rs:635-651`).
Setting `HOME` in the process is therefore sufficient for rnote's own code, and
for GLib at the same time — but only if it happens before `main` or before the
first `g_get_home_dir()`/`g_get_user_*_dir()` call.

### Qt (`QStandardPaths`, the base for all KDE ports)

Qt's own Android mapping table
([QStandardPaths](https://doc.qt.io/qt-6/qstandardpaths.html), "Android" column):
`HomeLocation` = `<APPROOT>/files`; `AppLocalDataLocation` = `<APPROOT>/files`;
`ConfigLocation` = `<APPROOT>/files/settings`; `CacheLocation` =
`<APPROOT>/cache`; `GenericDataLocation` = `<USER>` or
`<USER>/<APPNAME>/files`. I.e. the idiomatic Qt answer for "user home" in a
sandbox is the internal `files` dir.

### Krita (KDE's desktop→Android port)

Krita's project statements confirm the Qt behaviour and the friction it causes:

- MR "Make recorder docker not trap files"
  ([invent.kde.org/graphics/krita!2616](https://invent.kde.org/graphics/krita/-/merge_requests/2616)):
  "On Android, no user-visible directories are writeable by default. That means
  `QDir::homePath()` is an application-internal directory by default, which means
  the recorder docker's default storage location dumps file in a place
  inaccessible to the user."
- Work item "Get a Qt6 build of Krita on Android"
  ([invent.kde.org/graphics/krita#121](https://invent.kde.org/graphics/krita/-/work_items/121))
  lists the port patches, including "Android: Use External Storage for some
  Standard Paths Types: AppConfigLocation, AppDataLocation, AppLocalDataLocation,
  HomeLocation" (unchecked) and "Android: Set TMPDIR to point to cache dir rather
  than a persistent dir" (applied).

So Krita's answer is: home-equivalent = app files dir, and where files are meant
to be user-reachable, deliberately relocate to the *external* app dir
(`Android/data/<pkg>/files`) — while noting that this is not reachable by other
apps' pickers since Android 11 either.

### Termux (Linux userland on Android)

`$HOME` is a real path inside the app's internal files dir:
`TERMUX_HOME_DIR_PATH = TERMUX_FILES_DIR_PATH + "/home"` = `/data/data/com.termux/files/home`
(`termux-shared/.../TermuxConstants.java:588,656-658`), and the shell environment
sets `environment.put(ENV_HOME, TermuxConstants.TERMUX_HOME_DIR_PATH)`
(`termux-shared/.../TermuxShellEnvironment.java:78`). Termux's `$PREFIX` is the
sibling `files/usr`. It also sets `TMPDIR` to the internal `usr/tmp`, not the
Android cache dir, because a package manager needs a persistent tmp.

### SDL3 (other C/C++ ports)

SDL sets no `HOME`; it exposes the two app dirs instead
([SDL_system.h](https://github.com/libsdl-org/SDL/blob/main/include/SDL3/SDL_system.h)):

- `SDL_GetAndroidInternalStoragePath()` — "typically `/data/data/your.app.package/files`",
  a C wrapper over `getFilesDir()`;
- `SDL_GetAndroidExternalStoragePath()` — "typically
  `/storage/sdcard0/Android/data/your.app.package/files`", wrapper over
  `getExternalFilesDir()`;
- `SDL_GetAndroidCachePath()` — wrapper over `getCacheDir()`.

Implementation: `src/core/android/SDL_android.c:2427-2441` (`getFilesDir`),
`:2500-2530` (`getExternalFilesDir`, `getAbsolutePath` — note GTK uses
`getAbsolutePath` too, SDL internal uses `getCanonicalPath`).

### pixiewood (the builder itself)

pixiewood contributes no runtime environment: it generates the Gradle project and
symlinks GTK's Java glue
(`~/.cache/ranote-android/pixiewood/pixiewood:731-733`, default
`subprojects/gtk/gdk/android/glue/java/`), and its `android.environment` map is
build-time only (`:417`, used for `MESON`). The manifest it emits is the only
storage-relevant output (`android:allowBackup="true"`, `manifest.xsl:20`). So the
env vars rnote sees are exactly GTK's four `XDG_*` plus whatever rnote's
`setup_env()` sets.

## 3. Practical constraint check

rnote needs a real filesystem path for drafts, settings and a default workspace
folder, used from `std::fs` (and `gio::File::new_for_path` for the browser).

- All four `Context` dirs are ordinary absolute POSIX paths in a directory the
  app uid owns; `std::fs::{create_dir, read_dir, write, rename}` work unchanged
  (`ContextImpl.java:850-931`).
- They are *not* guaranteed app-stable across launches: resolve per launch
  (`Context.java:1380-1390`, 1497-1500). rnote's current
  `cache_home_from_data_dirs()` derivation (`crates/rnote-ui/src/env.rs:144-156`)
  assumes `dataDir/files` and `dataDir/cache` are siblings, which is true of
  `ContextImpl` but is a layout assumption, not an API guarantee.
- A SAF tree URI is *not* a candidate for `$HOME`: it is a `content://` URI, not a
  path, cannot be handed to `std::fs`, and grants are not persisted in this build
  (`docs/android/saf.md:21-31`), so it would not survive a process restart anyway.
- Size: internal storage "tends to be small" (app-specific guide) and is backed by
  the data partition; the external app dir lives on the same physical partition on
  emulated-storage devices — `getExternalFilesDir()`'s javadoc notes "little
  benefit to storing data here instead of the private directories returned by
  `getFilesDir()`" when the volume is emulated (`Context.java:1480-1488`), and the
  app-specific guide warns that emulated external storage is still internal flash.
- Visibility: `files` and `cache` are invisible to everything outside the app
  (except root/`adb`/backup). `Android/data/<pkg>/files` is on the shared volume
  but is no longer selectable by SAF pickers or other apps since Android 11
  (Android 11 storage page), and is removed on uninstall.
- Backup: `files` and `Android/data/<pkg>/files` are uploaded by Auto Backup (25 MB
  cap, only at install/restore); `cache` and `no_backup` never are (Auto Backup).

## Options for rnote

Candidate home-equivalent, i.e. what `$HOME` (or the code's home fallback) would
point at. "system-evictable" = cleared without rnote's involvement.

| Candidate | Stable path? | User-visible? | System-evictable? | Survives uninstall? | Portable (backup / other device)? |
| --- | --- | --- | --- | --- | --- |
| `getFilesDir()` (`/data/user/0/<pkg>/files`) | yes, real path; base may move with adopted storage | no (app-private) | no by the OS — but GTK's `SystemFilesystem` wipes it on an asset-changing update | no | yes via Auto Backup (≤25 MB) |
| `getFilesDir()/home` (own subdir) | same as above | no | only via the asset-wipe above if it is inside `files/` | no | yes via Auto Backup |
| `getNoBackupFilesDir()` (`.../no_backup`) | yes | no | no | no | no (excluded from backup by design) |
| `getCacheDir()` (`.../cache`) | yes | no | yes — evicted under disk pressure, oldest first; clearable via "clear cache"/`ACTION_CLEAR_APP_CACHE` | no | no (excluded from backup) |
| `getExternalFilesDir()` (`/storage/emulated/0/Android/data/<pkg>/files`) | yes, but `null` when the volume is unavailable; changes if media is swapped | on shared storage, but unreachable from SAF pickers/other apps since API 30 | no by the OS (`getExternalCacheDir` javadoc: platform "does not always monitor" shared storage) | no | yes via Auto Backup; moves with the volume, not with the app |
| SAF tree chosen by the user (`content://…`) | not a path — `std::fs` cannot use it | yes | no | no (grant lost on process death, `docs/android/saf.md:21-31`) | no |

Adjacent facts that constrain the choice, not a recommendation:

- GTK already owns `files/share` + `files/etc` (asset copy, wiped on update) and
  `Android/data/<pkg>/files/{share,etc}` (XDG user dirs), so a home placed inside
  either of those shares directory with data GTK rewrites.
- `HOME` must be set before the first GLib call (all `g_get_user_*_dir` results are
  cached, `gutils.c:1919-1937`), and rnote reads it from Rust too, where only
  `$HOME` counts (`os.rs:635-651`).
- `std::env::set_var` in `main` is enough for both, but GTK's runtime runs before
  `main`, so a `HOME` set by rnote does not change the `XDG_*` mapping GTK already
  installed.

## Sources

Platform

- [App-specific storage](https://developer.android.com/training/data-storage/app-specific) — permissions, encryption, size, external app dirs, cache removal, uninstall.
- [Storage in Android 11](https://developer.android.com/about/versions/11/privacy/storage) — other apps' `Android/data` inaccessible, SAF document restrictions, `ACTION_CLEAR_APP_CACHE`.
- [Auto Backup](https://developer.android.com/identity/data/autobackup) — backed-up vs excluded directories, 25 MB quota, restore at install.
- AOSP `frameworks/base/core/java/android/content/Context.java` (javadoc for `getFilesDir`, `getNoBackupFilesDir`, `getExternalFilesDir`, `getCacheDir`).
- AOSP `frameworks/base/core/java/android/app/ContextImpl.java` — path construction.
- AOSP `frameworks/base/core/java/android/os/Environment.java` — `buildExternalStorageAppFilesDirs`.

This repo / local checkouts

- `misc/android/app/subprojects/gtk/gdk/android/gdkandroidruntime.c:259-281` — XDG mapping, no `HOME`.
- `misc/android/app/subprojects/gtk/gdk/android/glue/java/org/gtk/android/SystemFilesystem.java:42-111` — `cleanDirectory(getFilesDir())`.
- `misc/android/app/subprojects/glib/glib/gutils.c:836-897,1815-1862,1922-1949,2059-2103` — home/XDG resolution, `g_set_user_dirs`.
- `misc/android/app/subprojects/glib/glib/gutilsprivate.h:35` — `g_set_user_dirs` is private.
- `~/.cache/ranote-android/pixiewood/generate/manifest.xsl:20`, `pixiewood:417,731-733`, `prepare/wraps/glib/hack.patch` — builder contributions.
- `crates/rnote-ui/src/env.rs`, `crates/rnote-ui/src/workspacebrowser/workspacesbar/workspacelistentry.rs` — the failing call sites.
- `misc/android/README.md:393-400`, `docs/android/saf.md` — existing notes.

Other projects

- Rust `std` 1.89 `library/std/src/sys/pal/unix/os.rs:635-651` — `home_dir()` on Android.
- [Qt QStandardPaths](https://doc.qt.io/qt-6/qstandardpaths.html) — Android path table.
- Krita [MR !2616](https://invent.kde.org/graphics/krita/-/merge_requests/2616), [work item #121](https://invent.kde.org/graphics/krita/-/work_items/121).
- Termux `TermuxConstants.java:588,656-658`, `TermuxShellEnvironment.java:78`.
- SDL3 `include/SDL3/SDL_system.h`, `src/core/android/SDL_android.c:2427-2530`.
