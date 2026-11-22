# Android non-stylus performance (ticket #12)

Status: instrumentation, device-check wiring and basic tuning have landed.
**The gate has not been measured: there is no Android device in this
environment.** The result table stays empty until the run in
[Device procedure](#device-procedure) happens.

Stylus input-to-pixel latency is **deferred** to the stylus verification ticket
(#24). This document and the instrumentation only make claims about touch
(pan/zoom and, with touch drawing enabled, finger strokes).

Parent gate (issue #1): a 20-page document scrolls and draws at **>= 55 fps
median** with **p95 frame time <= 33 ms**, and a 30-minute session does not OOM.

## Instrumentation

The recorder lives in `crates/rnote-ui/src/perf.rs`. It is debug-only and off by
default:

- it only activates in a debug build on Android;
- it needs an explicit opt-in: the `RSNOTE_PERF` environment variable
  (`1`/`true`/`yes`/`on`) or an app-private `perf.on` marker file;
- in a release build the hooks return immediately and nothing is recorded.

What it records:

| Sample | Source | Log line (`rnote::perf`) |
|---|---|---|
| Frame interval | `RnCanvas::snapshot`, after `draw_to_gtk_snapshot` | `frame_summary frames= median_fps= p95_frame_ms= gate=pass\|fail` |
| Touch input-to-snapshot | non-stylus pointer event time -> next `snapshot` | `touch_latency_summary samples= median_ms= p95_ms=` |
| Stylus pressure | `gdk::AxisUse::Pressure` on stylus pointer events | `pressure_summary samples= min= max= mean=` |
| Process RSS | `/proc/self/status` every 10 s | `memory rss_mb=` |

`frame_summary` is emitted at most every 5 s over the last ~600 rendered frames
(idle gaps over 250 ms are dropped instead of counted as slow frames).
`gate` compares that window with 55 fps median and 33 ms p95.

Input-to-snapshot is a **lower bound** for input-to-pixel: the GTK snapshot is
the last point the app controls; compositor/present time is not observable from
the widget. Treat a slow value as real, a fast value as "no worse than this".

Privacy: the recorder logs only numbers and fixed labels. It never receives a
document, stroke, file name or path. Stylus latency is still not sampled
(deferred), but pressure values are recorded as bare numbers.
Tracing is filtered to the `rnote::perf` target in debug Android builds, so
normal `RUST_LOG` settings do not hide it. On Android the tracing writer sends
lines to logcat under the tag `rnote` (app stdout is `/dev/null` there), which is
what `logcat.sh` and `device-check.sh` read.

## Enabling on a device

```sh
just android-app-perf-on     # drops perf.on into the app's private data/cache dirs
just android-app-install     # restart so the opt-in is read at startup
# exercise the document, then:
just android-app-device-check
just android-app-perf-off    # when done; the marker is removed again
```

`android-app-device-check` writes:
`misc/android/build/device-check-<timestamp>.txt`, including the refresh rate,
`gfxinfo` percentiles, the raw `rnote::perf` lines and the last
`frame_summary` with its gate verdict.

## Basic tuning applied

- GL renderer is forced on Android (`GSK_RENDERER=gl`, ticket #6). There is no
  cairo fallback.
- GTK UI transition animations are disabled on Android
  (`gtk-enable-animations=false`), so they cannot take frame budget from touch
  drawing. Desktop keeps animations enabled.
- Touch/stylus ownership and multi-touch rejection from ticket #11 are the
  input-path tuning; no extra gesture stack was added.

No renderer change, no partial redraw and no engine tuning was made: the gate
has not been observed on hardware, so tuning beyond the obvious is guesswork.
If a device run fails the gate, start from the `frame_summary` numbers and the
`gfxinfo` percentiles in the device report.

## Device procedure

1. Build a 20-page document on desktop: a new document, add 19 pages with
   "Add page", draw something on several pages, save as
   `perf-20-pages.rnote`.
2. `adb push perf-20-pages.rnote /sdcard/Download/`.
3. `just android-app-perf-on && just android-app-install`.
4. Open the document through the system picker (Open).
5. Exercise it for at least 5 minutes: pan/zoom with two fingers on several
   pages, turn on "Draw with touch" and draw on a few pages, scroll the sidebar
   and page thumbnails.
6. In a 30-minute session, leave the document open and keep interacting; watch
   `memory rss_mb` for a steady climb that ends in a kill (OOM).
7. `just android-app-device-check` and record the results below.

Leave the app in the foreground while collecting so `gfxinfo` has fresh data.

## Results

Fill this in per device. None of these rows was measured in this environment.

| Device / GPU | Android | Document | Median fps | p95 frame ms | Gate | Notes |
|---|---|---|---|---|---|---|
| _not measured (no device)_ | | 20 pages | | | | |
| | | | | | | |

Also record:

- touch input-to-snapshot median/p95 from `touch_latency_summary`;
- RSS at start/end of the 30-minute session from `memory`;
- `gfxinfo` "Janky frames" and the 95th percentile from the report.

## Blockers

- **No Android device or emulator with a GL stack** is available here, so the
  20-page document was not opened on Android and no threshold was measured.
  This is the only blocker for the gate itself; the code and tooling paths are
  verified without a device (see `misc/android/README.md`).
- **Stylus latency is deferred** to #24 by the ticket, not measured here.
- `gfxinfo` percentiles include all app frames, while `frame_summary` covers
  rendered canvas frames. When they disagree, prefer the app's numbers for the
  canvas gate and use `gfxinfo` to spot UI/compiler jank.
