# SAF storage on Android

Status: open, save, save-as, export and workspace folders go through SAF content
URIs. No broad storage permission is requested. Desktop keeps using local
paths; the storage code only branches on `gio::File::is_native()`.

## How it works

- Documents and workspace folders are `gio::File`s, so the same code handles
  local paths and `content://` URIs.
- Native files are written with the engine's temp-file + fsync + rename path;
  content URIs use `gio::File::replace_contents_future`, the only write path a
  provider exposes.
- `FileType` classifies content files by MIME type because SAF reports
  `standard::type` as unknown. `vnd.android.document/directory` is a folder.
- Page export only appends collision suffixes for non-native folders, so a
  desktop re-export still overwrites and a SAF folder keeps both versions.
- The Android catalog reader prefers `rnote.mo` and falls back to any `*.mo`
  file in `LC_MESSAGES`.

## Device blocker: persisted URI grants

`takePersistableUriPermission` is not called. GIO exposes `gio::File` and the
picker, but neither the pinned GTK 4.22.5 Android backend nor GIO has an API
for persisting a SAF grant; reaching `ContentResolver.takePersistableUriPermission`
would need raw JNI glue, which this spike deliberately does not add.

Consequence: a document or workspace folder opened through SAF cannot be
reopened after the process is killed without picking it again. The grant lasts
for the process lifetime, which covers normal backgrounding because GTK keeps
the activity's windows alive (ticket #10). This is a device-verification item,
not a build failure.
