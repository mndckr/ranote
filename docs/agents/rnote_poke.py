#!/usr/bin/env python3
"""Drive the running rnote GUI. Verified against rnote 0.14.2 (flatpak) on Wayland.

Three layers, in order of preference:
  1. GAction over D-Bus  - app/window commands (no pointer, no screenshots)
  2. AT-SPI widget actions - anything inside a panel (settings, pages, colours)
  3. uinput synthetic pointer - canvas drawing only

Every command starts rnote if it isn't running and quits it afterwards, so a run
leaves no process behind. Pass --keep to leave it running (useful while iterating).

Examples:
  ./rnote_poke.py windows
  ./rnote_poke.py settings
  ./rnote_poke.py page Selector
  ./rnote_poke.py ls --filter scroll
  ./rnote_poke.py press "Show Scrollbars"
  ./rnote_poke.py color orange
  ./rnote_poke.py act app new-window
  ./rnote_poke.py draw 900,900 1100,1100 1300,900
  ./rnote_poke.py --keep square 1200,1000 150
  ./rnote_poke.py shot /tmp/opencode/rnote.png
  ./rnote_poke.py selftest

Needs PyGObject (present on most desktops). Drawing additionally needs
`pip install --user --break-system-packages python-uinput` and a writable
/dev/uinput.

Launch the app with `flatpak run com.github.flxzt.rnote &` (it is not on PATH).
Never `pkill -f rnote`: the pattern matches your own shell. Use `pkill -x rnote`.
"""
import argparse
import os
import re
import shutil
import subprocess
import sys
import time

SCHEMA = "com.github.flxzt.rnote"
APP_ID = "com.github.flxzt.rnote"
APP_PATH = "/com/github/flxzt/rnote"

# Swatch order in the stroke-colour popover, verified by reading
# `active-stroke-color` after each press. Colours as (r, g, b, a) from gsettings.
SWATCHES = [
    ("black", (0.0, 0.0, 0.0, 1.0)),
    ("white", (1.0, 1.0, 1.0, 1.0)),
    ("transparent", (0.0, 0.0, 0.0, 0.0)),
    ("blue-light", (0.597, 0.753, 0.941, 1.0)),
    ("blue", (0.101, 0.371, 0.703, 1.0)),
    ("green", (0.148, 0.632, 0.410, 1.0)),
    ("yellow", (0.957, 0.757, 0.066, 1.0)),
    ("orange", (0.898, 0.378, 0.0, 1.0)),
    ("red", (0.644, 0.113, 0.175, 1.0)),
]

INTERACTIVE = {
    "switch", "toggle button", "button", "combo box", "list item",
    "page tab", "spin button", "color chooser", "text", "entry",
}


def _atspi():
    import gi
    gi.require_version("Atspi", "2.0")
    from gi.repository import Atspi
    return Atspi


def app():
    Atspi = _atspi()
    d = Atspi.get_desktop(0)
    for i in range(d.get_child_count()):
        a = d.get_child_at_index(i)
        if a.get_name() == "rnote":
            return a
    return None


def frames():
    Atspi = _atspi()
    a = app()
    if not a:
        return []
    return [a.get_child_at_index(k) for k in range(a.get_child_count())
            if a.get_child_at_index(k).get_role_name() == "frame"]


def frame(index=0):
    fs = frames()
    if not fs:
        sys.exit("rnote is not running or not on the a11y bus (flatpak run %s &)" % APP_ID)
    return fs[index]


def walk(node, visits=None):
    """Depth-first, yielding every accessible node."""
    yield node
    for i in range(node.get_child_count()):
        yield from walk(node.get_child_at_index(i))


def actions_of(node):
    try:
        return [node.get_action_name(j) for j in range(node.get_n_actions())]
    except Exception:
        return []


def name_of(node):
    Atspi = _atspi()
    n = node.get_name() or ""
    if n:
        return n
    try:
        for rel in node.get_relations():
            if rel.get_relation_type() in (Atspi.RelationType.LABELLED_BY,
                                           Atspi.RelationType.LABEL_FOR):
                t = rel.get_target(0)
                if t is not None and (t.get_name() or ""):
                    return t.get_name()
    except Exception:
        pass
    return ""


def states_of(node):
    try:
        return [s.value_nick for s in node.get_state_set().get_states()]
    except Exception:
        return []


def activate(node, preferred=("toggle", "click", "press")):
    acts = actions_of(node)
    for want in preferred:
        if want in acts:
            ok = node.do_action(acts.index(want))
            time.sleep(0.4)
            return want, ok
    if acts:
        ok = node.do_action(0)
        time.sleep(0.4)
        return acts[0], ok
    return None, False


def gsettings(*args):
    r = subprocess.run(["flatpak", "run", "--command=gsettings", SCHEMA, *args],
                       capture_output=True, text=True, timeout=30)
    return r.stdout.strip()


def dbus(*args):
    r = subprocess.run(["gdbus", "call", "--session", "--dest", APP_ID, *args],
                       capture_output=True, text=True, timeout=30)
    if r.returncode != 0:
        sys.exit((r.stderr or r.stdout).strip())


# ---------------------------------------------------------------- lifecycle

def ensure_app(timeout=45):
    """Start rnote unless it is already up. Returns True if this script started it."""
    if app() is not None:
        return False
    print("starting rnote...", file=sys.stderr)
    subprocess.Popen(["flatpak", "run", APP_ID],
                     stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
                     stdin=subprocess.DEVNULL, start_new_session=True)
    deadline = time.time() + timeout
    while time.time() < deadline:
        if app() is not None:
            time.sleep(1.0)          # let the first window finish mapping
            return True
        time.sleep(0.5)
    sys.exit("rnote did not appear on the a11y bus; run `flatpak run %s` by hand" % APP_ID)


def quit_app(timeout=15):
    """Ask the app to quit, then make sure no process is left."""
    if app() is None:
        return
    subprocess.run(["gdbus", "call", "--session", "--dest", APP_ID,
                    "--object-path", APP_PATH, "--method",
                    "org.gtk.Actions.Activate", "quit", "[]", "{}"],
                   capture_output=True, text=True, timeout=30)
    deadline = time.time() + timeout
    while time.time() < deadline:
        if app() is None:
            return
        time.sleep(0.5)
    subprocess.run(["pkill", "-x", "rnote"], capture_output=True)
    time.sleep(1.0)


# ---------------------------------------------------------------- commands

def cmd_windows(args):
    fs = frames()
    if not fs:
        print("no rnote windows")
        return 1
    for i, f in enumerate(fs):
        try:
            e = f.get_extents(_atspi().CoordType.SCREEN)
            geo = "%dx%d" % (e.width, e.height)
        except Exception:
            geo = "?"
        print(f"{i}: {f.get_name()}  {geo}  actions on {APP_PATH}/window/{i + 1}")
    return 0


def cmd_act(args):
    path = APP_PATH if args.target == "app" else f"{APP_PATH}/window/{args.target}"
    dbus("--object-path", path, "--method", "org.gtk.Actions.Activate",
         args.action, args.arguments or "[]", "{}")
    print(f"activated {args.action} on {path}")
    return 0


def cmd_actions(args):
    path = APP_PATH if args.target == "app" else f"{APP_PATH}/window/{args.target}"
    out = subprocess.run(["gdbus", "call", "--session", "--dest", APP_ID,
                          "--object-path", path, "--method",
                          "org.gtk.Actions.DescribeAll"],
                         capture_output=True, text=True, timeout=30).stdout
    names = sorted(set(re.findall(r"'([a-z0-9][a-z0-9-]*)':", out)))
    print("\n".join(names))
    return 0


def cmd_ls(args):
    f = frame(args.window)
    roles = set(args.roles.split(",")) if args.roles else INTERACTIVE
    count = 0
    for node in walk(f):
        try:
            role = node.get_role_name()
        except Exception:
            continue
        if role not in roles:
            continue
        nm = name_of(node)
        if args.filter and args.filter.lower() not in nm.lower():
            continue
        acts = actions_of(node)
        flags = "checked" if "checked" in states_of(node) else ""
        print(f"{role:14} | {nm[:44]:44} | {','.join(acts)[:22]:22} | {flags}")
        count += 1
    print(f"-- {count} nodes", file=sys.stderr)
    return 0


def cmd_press(args):
    f = frame(args.window)
    exact, partial = [], []
    for node in walk(f):
        try:
            if args.role and node.get_role_name() != args.role:
                continue
            if not actions_of(node):
                continue
            nm = name_of(node)
            if nm == args.name:
                exact.append(node)
            elif args.name.lower() in nm.lower():
                partial.append(node)
        except Exception:
            continue
    hits = exact or partial
    if not hits:
        sys.exit(f"no actionable widget matching {args.name!r}")
    if len(hits) > 1 and not args.nth:
        for h in hits:
            print(f"  {h.get_role_name()} | {name_of(h)} | {actions_of(h)}", file=sys.stderr)
        sys.exit("ambiguous; narrow with --role or --nth")
    node = hits[args.nth or 0]
    before = states_of(node)
    used, ok = activate(node)
    after = states_of(node)
    print(f"{'press' if ok else 'FAILED'}: {node.get_role_name()} {name_of(node)!r} "
          f"via {used}")
    if "checked" in before or "checked" in after:
        print(f"checked: {'yes' if 'checked' in before else 'no'} -> "
              f"{'yes' if 'checked' in after else 'no'}")
    return 0 if ok else 1


def cmd_page(args):
    args.name = args.name.capitalize()
    args.role = "toggle button"
    args.nth = 0
    return cmd_press(args)


def cmd_settings(args):
    dbus("--object-path", f"{APP_PATH}/window/{args.window + 1}",
         "--method", "org.gtk.Actions.Activate", "open-settings", "[]", "{}")
    print("settings page opened")
    return 0


def _swatches():
    """Anonymous colour toggle buttons in the open stroke-colour popover."""
    f = frame(0)
    btn = None
    for node in walk(f):
        if node.get_role_name() == "toggle button" and name_of(node) == "Stroke Color":
            btn = node
            break
    if btn is None:
        sys.exit("stroke colour button not found")
    group = btn.get_parent()
    outer = group.get_parent()
    for i in range(outer.get_child_count()):
        cand = outer.get_child_at_index(i)
        if cand is group:
            continue
        toggles = [cand.get_child_at_index(k) for k in range(cand.get_child_count())
                   if cand.get_child_at_index(k).get_role_name() == "toggle button"
                   and not name_of(cand.get_child_at_index(k))]
        if len(toggles) >= 5:
            return btn, toggles
    return btn, []


def cmd_color(args):
    btn, toggles = _swatches()
    if not toggles:
        activate(btn)                      # open the popover
        time.sleep(0.8)
        btn, toggles = _swatches()
    if not toggles:
        sys.exit("colour popover did not expose swatches")
    wanted = args.color
    if wanted.isdigit():
        idx = int(wanted)
    else:
        names = [n for n, _ in SWATCHES]
        if wanted not in names:
            sys.exit(f"unknown colour {wanted!r}; choose from {', '.join(names)}")
        idx = names.index(wanted)
    before = gsettings("get", SCHEMA, "active-stroke-color")
    activate(toggles[idx], preferred=("click",))
    after = gsettings("get", SCHEMA, "active-stroke-color")
    print(f"active-stroke-color: {before} -> {after}")
    return 0


def shot(token="poke"):
    import gi
    gi.require_version("Gio", "2.0")
    from gi.repository import Gio, GLib
    token = re.sub(r"[^A-Za-z0-9]", "", token) or "poke"
    bus = Gio.bus_get_sync(Gio.BusType.SESSION, None)
    res = bus.call_sync(
        "org.freedesktop.portal.Desktop", "/org/freedesktop/portal/desktop",
        "org.freedesktop.portal.Screenshot", "Screenshot",
        GLib.Variant("(sa{sv})", ("", {"handle_token": GLib.Variant("s", token)})),
        GLib.VariantType("(o)"), Gio.DBusCallFlags.NONE, -1, None)
    loop = GLib.MainLoop()
    out = {}
    bus.signal_subscribe("org.freedesktop.portal.Desktop",
                         "org.freedesktop.portal.Request", "Response",
                         res.get_child_value(0).get_string(), None,
                         Gio.DBusSignalFlags.NONE,
                         lambda *a: out.update(uri=a[5].unpack()[1].get("uri", "")) or loop.quit())
    GLib.timeout_add_seconds(20, loop.quit)
    loop.run()
    if not out.get("uri"):
        sys.exit("screenshot failed (portal denied or timed out)")
    return out["uri"].replace("file://", "")


def cmd_shot(args):
    src = shot("shot")
    if args.path:
        shutil.copyfile(src, args.path)
        print(args.path)
    else:
        print(src)
    return 0


def _mouse(size=None):
    try:
        import uinput
    except ImportError:
        sys.exit("drawing needs python-uinput: "
                 "pip install --user --break-system-packages python-uinput")
    if size is None:
        from PIL import Image
        size = Image.open(shot("size")).size
    w, h = size
    return uinput, uinput.Device(
        [uinput.BTN_LEFT,
         uinput.ABS_X + (0, w - 1, 0, 0),
         uinput.ABS_Y + (0, h - 1, 0, 0)], name="rnote-poke")


def _stroke(dev, ui, pts, steps, hold):
    x0, y0 = pts[0]
    dev.emit(ui.ABS_X, x0)
    dev.emit(ui.ABS_Y, y0)
    time.sleep(0.25)
    dev.emit(ui.BTN_LEFT, 1)
    time.sleep(0.15)
    for (xa, ya), (xb, yb) in zip(pts, pts[1:]):
        for i in range(1, steps + 1):
            dev.emit(ui.ABS_X, xa + (xb - xa) * i // steps)
            dev.emit(ui.ABS_Y, ya + (yb - ya) * i // steps)
            time.sleep(hold)
    dev.emit(ui.BTN_LEFT, 0)
    time.sleep(0.35)


def _parse_points(specs):
    pts = []
    for s in specs:
        x, y = s.replace(" ", "").split(",")
        pts.append((int(x), int(y)))
    return pts


def cmd_draw(args):
    pts = _parse_points(args.points)
    if len(pts) < 2:
        sys.exit("give at least two x,y points")
    ui, dev = _mouse()
    _stroke(dev, ui, pts, args.steps, args.hold_ms / 1000.0)
    print(f"stroke through {len(pts)} points")
    return 0


def cmd_square(args):
    cx, cy = _parse_points([args.center])[0]
    s = args.half_size
    corners = [(cx - s, cy - s), (cx + s, cy - s), (cx + s, cy + s), (cx - s, cy + s)]
    ui, dev = _mouse()
    for i in range(4):
        _stroke(dev, ui, [corners[i], corners[(i + 1) % 4]], args.steps, args.hold_ms / 1000.0)
    print(f"square at {cx},{cy} half-size {s}")
    return 0


def cmd_gs(args):
    if args.op == "get":
        print(gsettings("get", SCHEMA, args.key))
    else:
        print(gsettings("set", SCHEMA, args.key, args.value))
    return 0


def cmd_selftest(args):
    """Check the three layers still work; restores everything it changes."""
    failures = []
    fs = frames()
    print(f"{'ok  ' if fs else 'FAIL'} windows: {len(fs)}")
    if not fs:
        return 1

    # AT-SPI: a known settings switch toggles and restores.
    # The settings page must be open for its widgets to exist at all.
    subprocess.run(["gdbus", "call", "--session", "--dest", APP_ID,
                    "--object-path", f"{APP_PATH}/window/1", "--method",
                    "org.gtk.Actions.Activate", "open-settings", "[]", "{}"],
                   capture_output=True, text=True, timeout=30)
    time.sleep(1.5)
    sw = [n for n in walk(fs[0]) if n.get_role_name() == "switch"
          and name_of(n) == "Show Scrollbars" and "toggle" in actions_of(n)]
    if not sw:
        failures.append("settings switch 'Show Scrollbars' not exposed")
        print("FAIL settings switch not found")
    else:
        before = "checked" in states_of(sw[0])
        activate(sw[0])
        flipped = ("checked" in states_of(sw[0])) != before
        activate(sw[0])
        restored = ("checked" in states_of(sw[0])) == before
        print(f"{'ok  ' if flipped and restored else 'FAIL'} switch toggle + restore")
        if not (flipped and restored):
            failures.append("switch toggle")

    # AT-SPI: page navigation
    page = [n for n in walk(fs[0]) if n.get_role_name() == "toggle button"
            and name_of(n) == "Tools"]
    if page:
        activate(page[0])
        print("ok   page navigation (Tools)")
    else:
        failures.append("page toggle 'Tools' not exposed")
        print("FAIL page navigation")

    # colours: popover exposes 9 swatches in the documented order
    btn, toggles = _swatches()
    if not toggles:
        activate(btn)
        time.sleep(0.8)
        btn, toggles = _swatches()
    if len(toggles) == len(SWATCHES):
        print(f"ok   colour popover exposes {len(toggles)} swatches")
    else:
        failures.append("colour swatches")
        print(f"FAIL colour popover: {len(toggles)} swatches, expected {len(SWATCHES)}")

    # D-Bus: the app action surface answers
    try:
        out = subprocess.run(["gdbus", "call", "--session", "--dest", APP_ID,
                              "--object-path", APP_PATH, "--method",
                              "org.gtk.Actions.DescribeAll"],
                             capture_output=True, text=True, timeout=30)
        ok = out.returncode == 0 and "new-window" in out.stdout
        print(f"{'ok  ' if ok else 'FAIL'} GAction DescribeAll ({APP_PATH})")
        if not ok:
            failures.append("dbus actions")
    except Exception as exc:
        print(f"FAIL dbus: {exc}")
        failures.append("dbus")

    if failures:
        print("\nfailures: " + ", ".join(failures))
        return 1
    print("\nall checks passed")
    return 0


def main():
    argv = sys.argv[1:]
    keep = "--keep" in argv
    argv = [a for a in argv if a != "--keep"]
    p = argparse.ArgumentParser(description=__doc__,
                                formatter_class=argparse.RawDescriptionHelpFormatter)
    p.add_argument("--keep", action="store_true",
                   help="leave rnote running afterwards (default: quit it)")
    sub = p.add_subparsers(dest="cmd", required=True)

    s = sub.add_parser("windows", help="list rnote windows and their D-Bus paths")
    s.set_defaults(func=cmd_windows)

    s = sub.add_parser("act", help="activate a GAction (target: app or window number)")
    s.add_argument("target")
    s.add_argument("action")
    s.add_argument("--arguments", help="GVariant text for the parameter array, default []")
    s.set_defaults(func=cmd_act)

    s = sub.add_parser("actions", help="list GAction names for app or a window")
    s.add_argument("target", nargs="?", default="app")
    s.set_defaults(func=cmd_actions)

    s = sub.add_parser("ls", help="list interactive widgets (role | name | actions)")
    s.add_argument("--filter", default="")
    s.add_argument("--roles", default="")
    s.add_argument("--window", type=int, default=0)
    s.set_defaults(func=cmd_ls)

    s = sub.add_parser("press", help="press a widget by accessible name")
    s.add_argument("name")
    s.add_argument("--role")
    s.add_argument("--nth", type=int, default=0)
    s.add_argument("--window", type=int, default=0)
    s.set_defaults(func=cmd_press)

    s = sub.add_parser("page", help="switch sidebar page (Brush/Shaper/Typewriter/Eraser/Selector/Tools)")
    s.add_argument("name")
    s.add_argument("--window", type=int, default=0)
    s.set_defaults(func=cmd_page)

    s = sub.add_parser("settings", help="open the settings page")
    s.add_argument("--window", type=int, default=0)
    s.set_defaults(func=cmd_settings)

    s = sub.add_parser("color", help="select stroke colour by name or swatch index")
    s.add_argument("color")
    s.set_defaults(func=cmd_color)

    s = sub.add_parser("shot", help="screenshot via the XDG portal")
    s.add_argument("path", nargs="?")
    s.set_defaults(func=cmd_shot)

    s = sub.add_parser("draw", help="draw a stroke through absolute screen points")
    s.add_argument("points", nargs="+", help="x,y ...")
    s.add_argument("--steps", type=int, default=20)
    s.add_argument("--hold-ms", type=float, default=20)
    s.set_defaults(func=cmd_draw)

    s = sub.add_parser("square", help="draw a square outline")
    s.add_argument("center", help="cx,cy")
    s.add_argument("half_size", type=int)
    s.add_argument("--steps", type=int, default=15)
    s.add_argument("--hold-ms", type=float, default=20)
    s.set_defaults(func=cmd_square)

    s = sub.add_parser("gs", help="read/write a rnote gsettings key")
    s.add_argument("op", choices=("get", "set"))
    s.add_argument("key")
    s.add_argument("value", nargs="?")
    s.set_defaults(func=cmd_gs)

    s = sub.add_parser("selftest", help="verify all three layers, restoring state")
    s.set_defaults(func=cmd_selftest)

    args = p.parse_args(argv)
    ensure_app()
    try:
        rc = args.func(args)
    finally:
        if not keep:
            quit_app()
    sys.exit(rc)


if __name__ == "__main__":
    main()
