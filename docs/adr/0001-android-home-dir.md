---
status: accepted
---

# Android home directory is the app-specific external files dir

On Android there is no user home: `$HOME` is unset, there is no passwd entry for
the app uid, and `std::env::home_dir()` returns `None` — which silently broke
drafts, the default workspace dir, and all other home-derived paths. The GTK
Android runtime already points GLib's writable `XDG_DATA_HOME`/`XDG_CONFIG_HOME`
at `getExternalFilesDir()/{share,etc}` (via `g_set_user_dirs`, *not* the process
env — `gdk/android/gdkandroidruntime.c`), so we adopt that same root as the
home-equivalent: rnote derives it per launch from GLib's resolved
`XDG_DATA_HOME` parent, exports it both as `$HOME` (`g_setenv`) and GLib-side
(a second `g_set_user_dirs("HOME", …)` call — the `hack` patch exports exactly
that symbol), and keeps rnote's usual subpath layout
(`<home>/Documents`, `<home>/.local/share`, `<home>/.cache`) under it. Never
persisted: base paths change with adopted storage and user profiles.

## Considered Options

- **Internal `files/` (`getFilesDir()`)** — rejected for anything
  user-irreplaceable: pixiewood's `SystemFilesystem.doWriteResources()` wipes
  the whole directory and re-copies assets whenever the asset fingerprint
  changes, i.e. **every asset-changing app update**. Termux and Qt use this
  root, but neither runs under pixiewood.
- **Shared storage (SAF/MediaStore) as the default workspace** — rejected as a
  default: content URIs instead of real paths, extra permission flows, and
  conflicts with the adopted per-app sandbox model of the XDG runtime above.
  Stays available as an explicit opt-in (workspaces accept SAF tree URIs).

## Consequences

- The default workspace is invisible to users and other apps on Android 11+
  and is **deleted on uninstall**. Accepted constraint: user-saved files must
  survive uninstalls only after the user explicitly picks another workspace
  (#28 tracks the onboarding gap). Drafts stay transient by design.
- The root is re-derived at every launch, so uninstall/reinstall or adopted
  storage moves are the only events that can relocate it — no migration is
  needed for the pre-fix `"./"` default, beyond re-defaulting it once.
- System cache eviction only ever touches cache-class dirs (sibling `cache/`),
  which is consistent with rnote's cache usage.
