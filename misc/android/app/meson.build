project(
    'rsnote-android',
    ['c', 'cpp'],
    version: '0.15.0',
    meson_version: '>= 1.9.0',
    default_options: ['warning_level=2'],
)

# Android build of the real rnote UI (ticket #6).
#
# Pixiewood configures this project once per ABI with its GTK cross files, runs
# the normal ninja build, then `meson install --tags runtime` into an asset
# root. The desktop meson project stays the source of truth for the Rust
# workspace; this project only reuses its templates and installs the Android
# packaging bits (locale .mo files, GSettings schema, the app library).

app_name = 'rnote'
app_name_capitalized = 'Rnote'
app_id = 'com.github.flxzt.rnote'
app_idpath = '/com/github/flxzt/rnote/'
version = meson.project_version()
profile = 'default'
gettext_package = 'rnote'

datadir = get_option('datadir')
localedir = get_option('localedir')
libdir = get_option('libdir')

repo_root = meson.project_source_root() / '../../..'
ui_src_dir = repo_root / 'crates/rnote-ui/src'
po_dir = repo_root / 'crates/rnote-ui/po'

# GTK and libadwaita are pixiewood wraps. The library targets are needed so the
# cargo target is scheduled after them; GTK's generated headers and built
# libraries must exist when cargo's pkg-config lookup and linker run.
gtk_dep = dependency('gtk4', version: '>= 4.22', fallback: ['gtk', 'libgtk_dep'])
adw_dep = dependency(
    'libadwaita-1',
    version: '>= 1.9',
    fallback: ['libadwaita', 'libadwaita_dep'],
)
libgtk = subproject('gtk').get_variable('libgtk')
libadwaita = subproject('libadwaita').get_variable('libadwaita')

# --- config.rs ---------------------------------------------------------------
# Same template as the desktop build, with the release app id. Cargo's `config`
# module needs the file in the source tree, as on desktop.
message('Configuring config.rs')

config_conf = configuration_data()
config_conf.set_quoted('APP_NAME', app_name)
config_conf.set_quoted('APP_NAME_CAPITALIZED', app_name_capitalized)
config_conf.set_quoted('APP_ID', app_id)
config_conf.set_quoted('APP_IDPATH', app_idpath)
config_conf.set_quoted('APP_VERSION', version)
config_conf.set_quoted('APP_VERSION_SUFFIX', '')
config_conf.set_quoted('APP_AUTHOR_NAME', 'Felix Zwettler')
config_conf.set_quoted(
    'APP_AUTHORS',
    run_command('cat', repo_root / 'AUTHORS', check: true).stdout().strip(),
)
config_conf.set_quoted('APP_WEBSITE', 'https://rnote.flxzt.net')
config_conf.set_quoted('APP_REPO_URL', 'https://github.com/flxzt/rnote')
config_conf.set_quoted('APP_ISSUES_URL', 'https://github.com/flxzt/rnote/issues')
config_conf.set_quoted(
    'APP_SUPPORT_URL',
    'https://github.com/flxzt/rnote/discussions',
)
config_conf.set_quoted('APP_DONATE_URL', 'https://rnote.flxzt.net/#support')
config_conf.set_quoted('PROFILE', profile)
config_conf.set_quoted('GETTEXT_PACKAGE', gettext_package)
config_conf.set_quoted('DATADIR', datadir)
config_conf.set_quoted('LIBDIR', libdir)
config_conf.set_quoted('LOCALEDIR', localedir)

config_rs = configure_file(
    input: files('../../../crates/rnote-ui/src/config.rs.in'),
    output: 'config.rs',
    configuration: config_conf,
)
run_command(
    'cp',
    config_rs,
    ui_src_dir / 'config.rs',
    check: true,
)

# --- GSettings schema --------------------------------------------------------
# Installed under share/glib-2.0/schemas so pixiewood compiles and packages it.
message('Configuring GSettings schema')

schema_conf = configuration_data()
schema_conf.set('APP_ID', app_id)
schema_conf.set('APP_IDPATH', app_idpath)
schema_conf.set('APP_NAME', app_name)
configure_file(
    input: files('../../../crates/rnote-ui/data/app.gschema.xml.in'),
    output: app_id + '.gschema.xml',
    configuration: schema_conf,
    install: true,
    install_dir: datadir / 'glib-2.0' / 'schemas',
    install_tag: 'runtime',
)

# --- translations ------------------------------------------------------------
# i18n.gettext() tags .mo files as "i18n", but pixiewood installs only the
# "runtime" tag. Build the .mo files here with the same msgfmt invocation but a
# runtime tag. The Android reader in src/i18n.rs scans LC_MESSAGES for *.mo, so
# the <lang>.mo filename is fine.
message('Configuring translations')

msgfmt = find_program('msgfmt', required: true)
linguist_langs = run_command('cat', po_dir / 'LINGUAS', check: true).stdout()
foreach line : linguist_langs.split('\n')
    lang = line.strip()
    if lang == '' or lang.startswith('#')
        continue
    endif
    custom_target(
        'mo-' + lang,
        input: po_dir / (lang + '.po'),
        output: lang + '.mo',
        command: [msgfmt, '-c', '-o', '@OUTPUT@', '@INPUT@'],
        build_by_default: true,
        install: true,
        install_dir: localedir / lang / 'LC_MESSAGES',
        install_tag: 'runtime',
    )
endforeach

# --- rnote-ui -----------------------------------------------------------------
# Android loads the app as a shared object with an exported `main` (GTK's
# RuntimeApplication), not as an ELF executable. cargo rustc emits exactly that
# with -shared; the wrapper script also points pkg-config at the meson-built
# GTK .pc files. See misc/android/README.md.
message('Configuring rnote-ui cargo target')

cpu_family = host_machine.cpu_family()
if cpu_family == 'aarch64'
    rust_target = 'aarch64-linux-android'
elif cpu_family == 'x86_64'
    rust_target = 'x86_64-linux-android'
else
    error('Unsupported Android ABI: ' + cpu_family)
endif

profile_dir = get_option('buildtype') == 'release' ? 'release' : 'debug'

bash = find_program('bash', required: true)
cargo = find_program('cargo', required: true)
cargo_build_script = files('../scripts/cargo-build.sh')
cc = meson.get_compiler('c').cmd_array()[0]
cxx = meson.get_compiler('cpp').cmd_array()[0]

rnote_lib = custom_target(
    'rnote',
    output: 'librnote.so',
    command: [
        bash,
        cargo_build_script,
        repo_root,
        meson.project_build_root(),
        rust_target,
        profile_dir,
        '@OUTPUT@',
        cc,
        cxx,
        cargo,
    ],
    depends: [libgtk, libadwaita],
    build_by_default: true,
    console: true,
    install: true,
    install_dir: libdir,
    install_tag: 'runtime',
)
