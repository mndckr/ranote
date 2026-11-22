# Debugging on a real Android device

For bugs that only reproduce on the GTK Android backend (input, lifecycle,
persistence). Desktop poking is covered by `live-poking.md`; this is the adb
workflow.

## Connect and drive

```sh
ADB=$HOME/Android/Sdk/platform-tools/adb
S=<ip>:<port>                    # adb connect <ip>:<port> first; check `adb devices`
$ADB -s $S shell am force-stop com.github.flxzt.rnote
$ADB -s $S shell am start -n com.github.flxzt.rnote/org.gtk.android.ToplevelActivity
$ADB -s $S shell input swipe 480 1400 200 1400 1500   # drag; same x,y + long ms = long-press
$ADB -s $S shell input tap 800 155
$ADB -s $S exec-out screencap -p > /tmp/opencode/shot.png
```

- `scripts/install.sh` runs plain `adb install` and fails when more than one
  device is attached — use `$ADB -s $S install -r <apk>` directly.
- uiautomator only sees the native SurfaceView, not GTK widgets. Find button
  coordinates from screenshots instead (header icons sit around y≈155 on a
  960x2142 Pixel screen).
- The input dispatcher occasionally logs `dropping inconsistent event` and
  eats a synthetic swipe; just retry.
- Verdict scripts comparing before/after screenshots live in
  `/tmp/opencode/touchdrag*.sh`: STROKE = dark-pixel count grows, PAN = content
  shifts. On the light theme the border heuristic misfires; trust the dark-pixel
  delta.

## Build, install, watch logs

```sh
just android-app-build           # -> misc/android/build/ranote-android.apk
$ADB -s $S install -r misc/android/build/ranote-android.apk
$ADB -s $S logcat -c              # clear
$ADB -s $S logcat -d --pid=$($ADB -s $S shell pidof com.github.flxzt.rnote | tr -d ' \r')
```

- rnote's tracing output lands in logcat under the tag `rnote`, but the default
  `EnvFilter` hides `debug!`/`warn!` unless `RUST_LOG` is set. For temporary
  instrumentation use `tracing::error!` — or set `RUST_LOG=rnote=debug` when
  launching. `eprintln!`/stdout is `/dev/null` in the app process; it will
  never appear in logcat.
- GSettings on Android uses the keyfile backend under
  `/storage/emulated/0/Android/data/com.github.flxzt.rnote/files/etc/` —
  readable via `run-as com.github.flxzt.rnote`, not via plain `adb shell`.

## Gotchas that cost a session

- In-session toggles of `win.touch-drawing` do propagate live (verified with
  `tracing::error!` in both `set_property` impls); the "needs a restart" symptom
  was a stale touch-sequence count (`num_touches` stuck > 1 after a popover
  claimed a touch), not a binding problem. Log the touch count before blaming
  the property chain.
- `GtkGesture` is abstract in gtk4-rs 0.11 and cannot be instantiated; use a
  concrete subclass or track state from the existing controllers.
