#!/usr/bin/env python3
"""Add the `.rnote` extension intent filter to the generated AndroidManifest.

Pixiewood emits a VIEW filter for every AppStream `<mediatype>`, but Android
has no MIME mapping for `.rnote`: file managers hand the document over as
`application/octet-stream`. A path pattern is the only way to appear in
"Open with" for that type, and pixiewood cannot express one, so build.sh runs
this after `pixiewood generate` and before `pixiewood build`.

Only the `content://` scheme is matched. Path patterns require an authority,
and `file://` URIs have none; Android 7+ file managers use content URIs.
"""

from __future__ import annotations

import sys
import xml.etree.ElementTree as ET
from pathlib import Path

ANDROID = "http://schemas.android.com/apk/res/android"
ACTIVITY = "org.gtk.android.ToplevelActivity"
PATH_PATTERN = r".*\.rnote"
MIME_TYPE = "application/octet-stream"


def android(tag: str) -> str:
    return f"{{{ANDROID}}}{tag}"


def activity(manifest: ET.Element) -> ET.Element:
    for app in manifest.findall("application"):
        for act in app.findall("activity"):
            if act.get(android("name")) == ACTIVITY:
                return act
    raise SystemExit(f"activity {ACTIVITY} not found in the generated manifest")


def has_extension_filter(manifest: ET.Element) -> bool:
    for filt in activity(manifest).findall("intent-filter"):
        for data in filt.findall("data"):
            if data.get(android("pathPattern")) == PATH_PATTERN:
                return True
    return False


def add_extension_filter(manifest: ET.Element) -> bool:
    """Add the filter once. Returns whether the manifest changed."""
    if has_extension_filter(manifest):
        return False

    filt = ET.SubElement(activity(manifest), "intent-filter")
    ET.SubElement(filt, "action", {android("name"): "android.intent.action.VIEW"})
    ET.SubElement(
        filt, "category", {android("name"): "android.intent.category.DEFAULT"}
    )
    ET.SubElement(
        filt,
        "data",
        {
            android("scheme"): "content",
            # Android only matches pathPattern when the filter has an authority.
            android("host"): "*",
            android("mimeType"): MIME_TYPE,
            android("pathPattern"): PATH_PATTERN,
        },
    )
    return True


def patch(path: Path) -> bool:
    ET.register_namespace("android", ANDROID)
    ET.register_namespace("android-tools", "http://schemas.android.com/tools")
    tree = ET.parse(path)
    if not add_extension_filter(tree.getroot()):
        return False
    ET.indent(tree, space="  ")
    tree.write(path, encoding="utf-8", xml_declaration=True)
    return True


SELF_TEST_MANIFEST = f"""<?xml version="1.0" encoding="UTF-8"?>
<manifest xmlns:android="{ANDROID}">
  <application android:name="org.gtk.android.RuntimeApplication">
    <activity android:name="{ACTIVITY}" android:exported="true">
      <intent-filter>
        <action android:name="android.intent.action.MAIN"/>
        <category android:name="android.intent.category.LAUNCHER"/>
      </intent-filter>
    </activity>
  </application>
</manifest>
"""


def self_test() -> None:
    manifest = ET.fromstring(SELF_TEST_MANIFEST)
    assert add_extension_filter(manifest)
    assert not add_extension_filter(manifest)
    filters = [
        filt
        for filt in activity(manifest).findall("intent-filter")
        if any(
            data.get(android("pathPattern")) == PATH_PATTERN
            for data in filt.findall("data")
        )
    ]
    assert len(filters) == 1
    data = filters[0].find("data")
    assert data is not None
    assert data.get(android("scheme")) == "content"
    assert data.get(android("host")) == "*"
    assert data.get(android("mimeType")) == MIME_TYPE
    # The output must still parse after the namespace prefixes are written.
    assert has_extension_filter(ET.fromstring(ET.tostring(manifest)))
    print("patch-manifest: ok")


def main(argv: list[str]) -> int:
    if argv == ["--self-test"]:
        self_test()
        return 0
    if len(argv) != 1:
        print(f"usage: {Path(sys.argv[0]).name} AndroidManifest.xml | --self-test",
              file=sys.stderr)
        return 2
    path = Path(argv[0])
    if patch(path):
        print(f"patched {path}")
    else:
        print(f"{path} already has the .rnote filter")
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
