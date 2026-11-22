# Android stylus and touch input

Status: wired against GTK's Android backend and covered by synthetic tests.
Real-tablet verification (pressure, tilt, buttons, eraser, palm rejection) is
deferred to the stylus sign-off ticket (#24). Nothing in this document claims
those were measured on hardware.

The desktop code paths are unchanged. The only Android-specific branch is the
stylus-ownership reset described below.

## What GTK 4.22.5's Android backend exposes

Checked against the pinned GTK commit in
[`misc/android/versions.env`](../../misc/android/versions.env)
(`bd25f1e2dc2c2fbf3b8864e61afe642910ba359c`):

- `GdkDeviceTool` exists for stylus and eraser (`GdkDeviceToolType::Pen` and
  `Eraser`) and for mouse. Fingers get **no** tool (touch events carry a `NULL`
  tool), which is exactly what `event_is_stylus()` tests.
  (`gdk/android/gdkandroidseat.c`, tool construction and
  `gdk_android_seat_get_device_tool()`.)
- Stylus tools advertise `X`, `Y`, `PRESSURE`, `DISTANCE`, `XTILT` and `YTILT`
  axes (`gdk_android_seat_stylus_flags`). Pressure is normalized to `[0, 1]`
  from Android's `MotionRange`; if the range cannot be read, the axis stays
  `0.0`. (`gdk_android_seat_create_axes_from_motion_event()`.)
- Buttons arrive as: contact down/up → `GDK_BUTTON_PRIMARY`, stylus primary
  button → `GDK_BUTTON_MIDDLE`, stylus secondary button → `GDK_BUTTON_SECONDARY`.
  (`gdk/android/gdkandroidevents.c`, `BUTTON_STYLUS_{PRIMARY,SECONDARY}`.)
- There are **no** `ProximityIn`/`ProximityOut` events. Hover enter/exit is
  commented out upstream; hovering produces `MotionNotify`. This matters for
  palm rejection, see below.

## rnote mapping

| Android input | GDK event | rnote behaviour |
|---|---|---|
| Stylus contact | `ButtonPress`/`ButtonRelease`, tool `Pen` | Draws in pen mode |
| Eraser tail | tool `Eraser` | `PenMode::Eraser` |
| Stylus primary button | `GDK_BUTTON_MIDDLE` | `ShortcutKey::StylusPrimaryButton` (default: temporary eraser) |
| Stylus secondary button | `GDK_BUTTON_SECONDARY` | `ShortcutKey::StylusSecondaryButton` (default: temporary selector) |
| Stylus motion with pressure | `MotionNotify` + `PRESSURE` axis | Pressure goes into `Element.pressure`; no synthetic curve |
| Stylus hover | `MotionNotify`, not button 1 | `PenState::Proximity` (no stroke) |
| One finger (touch drawing on) | `TouchBegin/Update/End` | Draws with `Element::PRESSURE_DEFAULT` |
| Two or more fingers | touch events | Rejected by the canvas, so `GestureZoom`/`GestureDrag` pan and zoom |
| Mouse | tool `Mouse` | Desktop behaviour |

Tilt axes are exposed by the backend but rnote does not consume tilt on any
platform, desktop included. That is not an Android regression.

## Android-specific behaviour

`update_stylus_active()` in `crates/rnote-ui/src/canvas/input.rs` keeps the
existing desktop rule (stylus owns the pointer until `ProximityOut`, or until a
mouse click). On Android `ProximityOut` never arrives, so a touch event clears
the stuck stylus state; otherwise the first stylus hover would swallow all
later touch input, including touch drawing.

Palm rejection is best-effort:

- A touch while a stylus stroke is down is ignored, independent of the touch
  drawing setting.
- Two or more simultaneous touches are already rejected, which keeps the
  two-finger pan/zoom gestures working. The count comes from active touch
  sequences (`RnCanvas::track_touch_event`), not from `GdkDevice::num-touches`:
  that property is the *maximum* concurrent touches and is `0` on Android.
- A palm that lands while the pen only hovers cannot be distinguished from a
  deliberate finger, because GTK cannot report that the pen left range. This
  relies on the tablet's digitiser/OS palm rejection and is a blocker to verify
  on hardware.
- A stroke starts when the first finger of a two-finger gesture lands, so a pan
  may leave a short mark. Drawing stops as soon as the second finger is down;
  watch for stray marks on the device checklist.

## Synthetic tests

`cargo test -p rnote --bin rnote canvas::input::tests` covers the mapping that
does not need a live GDK event:

- stylus primary/secondary buttons → stylus shortcuts, mouse secondary → mouse
  shortcut;
- device tool → pen/eraser mode;
- pressure is taken only from stylus input; non-stylus input uses the engine
  default;
- touch is rejected unless touch drawing is enabled, and multi-touch is always
  rejected for the canvas;
- stylus ownership transitions (proximity out, mouse hand-back, Android touch
  reset, palm during a stroke).

## Device acceptance steps (for #24)

Run on a tablet with an active stylus. Use the build/install/log commands from
[`misc/android/README.md`](../../misc/android/README.md).

1. **Hardware axes.** `adb shell getevent -lp` and find the stylus input
   device. It must advertise `ABS_PRESSURE`; tilt needs `ABS_TILT_X`/`ABS_TILT_Y`.
   Then run `adb shell getevent -lt` while drawing: `ABS_PRESSURE` values must
   change. If the kernel reports no pressure range, GTK's pressure axis is
   flat `0.0` and the result is a blocker, not a pass.
2. **Pressure.** Draw a slow-to-fast line with the brush. Stroke width must
   vary; a flat line means the axis is not reaching the app.
3. **Buttons.** Hold the barrel button while drawing: the stroke should erase
   (default temporary eraser). Hold the second button: temporary selector.
4. **Eraser.** Flip the pen to the eraser tail and touch the canvas: strokes are
   erased, not drawn.
5. **Touch pan/zoom.** With "Draw with touch" off (default), one finger pans,
   two fingers pinch-zoom, no crash.
6. **Touch drawing.** Enable "Draw with touch". One finger draws; two fingers
   still pan/zoom. Lift the pen, hover it, then draw with a finger: touch
   drawing must still work (this is the Android stylus-ownership reset).
7. **Switching devices.** Alternate stylus, finger and a Bluetooth/USB mouse
   several times. No crash, no stroke jumping to the last mouse position.
8. **Palm rejection.** Rest the palm while writing. Record pass/fail and the
   device model; the hover-only case may fail without an OS-level fix.

## Blockers

- **No Android stylus device available in this environment.** Pressure, tilt,
  buttons, eraser and palm rejection are wired but unverified on hardware.
  Verification is tracked by #24.
- **GTK Android has no proximity events.** Palm rejection while the pen only
  hovers cannot be implemented from GTK's event stream. Re-check when GTK
  emits hover enter/exit or proximity events; the relevant code is commented
  out in `gdk/android/gdkandroidevents.c` at the pinned commit.
- **Pressure depends on `InputDevice.getMotionRange()`.** A device that fails
  to report a pressure range yields `0.0`. Record it in the device matrix
  rather than compensating in the app (do not fake pressure).
