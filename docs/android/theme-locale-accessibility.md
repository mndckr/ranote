# Android theme, locale, settings, and TalkBack labels

Ticket #17. Desktop behaviour is unchanged; everything here is either Android
only or a correctness fix that is a no-op on desktop.

## What changed

### Dark mode follows Android

Android's GDK backend exposes the system night mode as the `night-mode`
property on `GdkDisplay` and sets `gtk-application-prefer-dark-theme`.
libadwaita 1.9 ignores that setting and has no portal or GNOME desktop schema
on Android, so it would stay light. `platform::follow_system_color_scheme()`
bridges it:

- the `app.color-scheme` action still stores what the user asked for
  (`default`, `force-light`, `force-dark`);
- when the request is `default`, the effective scheme comes from the Android
  night mode (dark -> force dark, light -> force light);
- a `notify::night-mode` handler re-applies the scheme when the system theme
  changes, including while the app is open.

The action-to-style-manager binding is one-way on Android so the effective
scheme never overwrites the requested setting. Desktop keeps the existing
bidirectional binding.

### UI locale from Android settings

Android does not export the Java locale to the POSIX environment.
`env::setup_env()` reads `persist.sys.locale` (then `ro.product.locale`) through
`__system_property_get`, converts `de-DE` to `de_DE`, and sets `LANGUAGE`
before `i18n::setup()`. The existing `.mo` reader picks the catalog from the
packaged `share/locale` directory. A user-provided `LANGUAGE` is left alone.

### Settings survive a restart

The GSettings schema has been bundled since ticket #6. Android has no dconf
module, so `setup_env()` selects GLib's `keyfile` backend. GTK's Android runtime
points `XDG_CONFIG_HOME` at the app's files directory, so settings are written
to an app-private keyfile:

```
.../Android/data/com.github.flxzt.rnote/files/etc/glib-2.0/settings/keyfile
```

No separate settings store was added: GSettings with an app-private keyfile is
the fallback, behind the environment/platform setup.

### Basic TalkBack labels

Icon-only controls with no text now set `GtkAccessible` labels (translated) for:

- header: new tab, view open tabs, focus mode, save, both sidebar toggles;
- pen picker: brush, shaper, typewriter, eraser, selector, tools, undo, redo;
- sidebar close buttons, app menu button, canvas menu button;
- app menu quick controls: left/right sidebar, fullscreen.

The menu items themselves already have labels.

## Device verification

Requires a device or emulator with the APK installed
(`just android-app-build && just android-app-install`). This environment has no
SDK or device, so these steps were not run here.

### Dark mode

1. Set the system to light, launch rnote with the app color scheme on
   "Use System Colors" (the default). The app is light.
2. Switch the system to dark while rnote is in the foreground. The app turns
   dark without a restart.
3. Switch back to light. The app follows.
4. Pick "Use Dark Colors", switch the system to light: the app stays dark.
5. Pick "Use System Colors" again and confirm it follows the system.

Logs: `just android-app-logcat` should show `night mode changed` from the GTK
Android backend when the system theme changes.

### Locale

1. Set a non-English system language that rnote ships (for example German or
   French), then force-stop and relaunch rnote.
2. Check `adb shell getprop persist.sys.locale` returns e.g. `de-DE`, and that
   main labels (title bar, menu, dialogs) are translated.
3. Switch to a language rnote does not ship; the UI falls back to English
   without crashing.

### Settings restart

1. Change a persisted setting, for example hide the sidebar or change the
   autosave interval.
2. `adb shell am force-stop com.github.flxzt.rnote`
3. Relaunch. The setting is still in effect.
4. Optional: `adb shell ls /sdcard/Android/data/com.github.flxzt.rnote/files/etc/glib-2.0/settings`
   and confirm `keyfile` exists after the change.

### TalkBack

1. Enable TalkBack (Settings > Accessibility > TalkBack).
2. Focus the header buttons: each is announced with its action name, not just
   "button".
3. Focus the pen picker tools and undo/redo, the two menu buttons, and the
   sidebar close button.

## Automated checks used instead

```sh
cargo test -p rnote --bins
cargo check -p rnote --target aarch64-linux-android
```

The dark-mode request/effective-scheme mapping and the locale tag normalization
have unit tests. Accessible labels are set through GTK's accessibility API and
need the device check above.
