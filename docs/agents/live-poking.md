# Live poking: drive the running GTK app (Wayland)

Use `rnote_poke.py` in this folder. It starts rnote when it isn't running, quits it
afterwards, and leaves no process behind. `--keep` leaves it running between calls.

X11 is out of scope.

## Three layers, in order of preference

1. **GAction over D-Bus** — app and window commands. No pointer, no screenshots.
2. **AT-SPI widget actions** — anything inside a panel: settings rows, sidebar pages,
   colour swatches. Coordinate-free; works on Wayland.
3. **uinput synthetic pointer** — canvas drawing only, because strokes only enter as
   pointer events.

```sh
./rnote_poke.py windows                 # windows + their D-Bus paths
./rnote_poke.py actions app             # what the app exposes
./rnote_poke.py act app new-window      # run one
./rnote_poke.py settings                # open the settings page
./rnote_poke.py page Selector           # Brush|Shaper|Typewriter|Eraser|Selector|Tools
./rnote_poke.py ls --filter scroll      # role | name | actions
./rnote_poke.py press "Show Scrollbars" # do_action, reports before/after
./rnote_poke.py color orange            # black|white|transparent|blue-light|blue|green|yellow|orange|red
./rnote_poke.py draw 900,900 1100,1100  # absolute screen points
./rnote_poke.py --keep square 1200,1000 150
./rnote_poke.py shot /tmp/opencode/rnote.png
./rnote_poke.py selftest                # checks all three layers, restores state
```

Static map of the same UI, when reading beats driving:
`crates/rnote-ui/**/*.ui`, `crates/rnote-ui/src/appwindow/actions.rs`,
`crates/rnote-ui/data/app.gschema.xml.in`.

## Facts that cost time to rediscover

- rnote runs as the flatpak `com.github.flxzt.rnote`; it is not on `PATH`.
  Launch with `flatpak run com.github.flxzt.rnote &`.
- App actions live at `/com/github/flxzt/rnote` (`new-window`, `quit`,
  `color-scheme`); window actions at `/com/github/flxzt/rnote/window/N`. On the bus
  the names are **unprefixed**; the `win.`/`app.` prefixes only exist in AT-SPI.
- AT-SPI exposes roughly 1000 nodes with real names (`Show Scrollbars`, `Orientation`,
  `Format`), but **every extent is `(0,0,w,h)`** — coordinate clicking is impossible,
  so always go through `do_action`. Actions are `toggle`, `click`, `press`.
- Colour swatches are anonymous `toggle button`s inside the stroke-colour popover.
  Their order is: black, white, transparent, blue-light, blue, green, yellow, orange,
  red. Press them; the value lands in `active-stroke-color`.
- Writing `active-stroke-color` with `gsettings set` does **not** update the live pen.
  Press the swatch instead.
- Screenshots use the XDG portal (`org.freedesktop.portal.Screenshot`); `grim` is not
  installed. Portal handle tokens must be alphanumeric.
- `Atspi.generate_mouse_event` and XTEST return success on this session but never move
  the pointer. `uinput` does (needs `pip install --user --break-system-packages
  python-uinput`, writable `/dev/uinput`).
- Quitting discards the session, so rnote's draft save is the only recovery. Run with
  `--keep` while iterating.
- Use `pkill -x rnote`. `pkill -f rnote` matches your own shell command and kills it.
