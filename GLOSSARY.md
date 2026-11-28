# ranote

The context of the ranote codebase: a GTK4 note-taking app and its Android build.
This glossary records the project's canonical language.

## Language

### Storage locations

**Home (Android)**:
The single app-owned external files directory (`getExternalFilesDir()`) that
stands in for the desktop home directory; derived per launch from GLib's
resolved `XDG_DATA_HOME`, never persisted. rnote's usual subpaths
(`Documents`, `.local/share`, `.cache`) live under it.
_Avoid_: home dir (ambiguous on Android), user profile, data dir

**Workspace**:
The folder a user works in, listed by the workspace browser. Either a local
path or a SAF content tree URI. The default workspace lives under the Android
home; a user-chosen workspace may be any user-visible folder and is the
supported way to keep files alive across app uninstalls.
_Avoid_: folder (default word when convenient), documents dir

**Draft**:
Transient per-document recovery data, kept under the Android home and lost on
uninstall by design. Distinct from user-saved files, which live wherever the
user saves them — not necessarily the home.

### Files on Android

**Native file**:
One backed by a real filesystem path (`file://`). Only native files may use
path-based atomic saves and monitoring; content URIs must go through GIO.
_Avoid_: local file when a content URI is also local to the device
