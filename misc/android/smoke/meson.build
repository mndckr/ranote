project(
    'rsnote-android-smoke',
    'c',
    version: '0.1.0',
    meson_version: '>= 1.9.0',
    default_options: ['warning_level=2'],
)

gtk_dep = dependency('gtk4', version: '>= 4.22')
adw_dep = dependency('libadwaita-1', version: '>= 1.9')

# android_exe_type: 'application' is what turns the binary into the shared
# object loaded by GTK's org.gtk.android.RuntimeApplication. Requires meson 1.9.
executable(
    'rsnote-smoke',
    sources: 'src/main.c',
    c_args: ['-DG_LOG_DOMAIN="rsnote-smoke"'],
    dependencies: [gtk_dep, adw_dep],
    android_exe_type: 'application',
    install: true,
)
