# test/meson.build — fakeroot test suite

# The tests are shell scripts that source ./defs (generated from defs.in).
# They need the built libfakeroot and faked to be present.

# Generate test/defs from test/defs.in using the same substitution variables
# as configure.ac generates for AC_CONFIG_FILES([test/defs]).
defs_conf = configuration_data()
# LDPRELOADVAR is the env var used to inject the preload library (LD_PRELOAD on Linux,
# DYLD_INSERT_LIBRARIES on macOS).
defs_conf.set('LDPRELOADVAR', build_vars['ldpreloadvar'])
# Autotools puts shared libs in @LT_OBJDIR@; Meson puts them directly in the build root.
defs_conf.set('LT_OBJDIR', '')
defs_conf.set('FAKEROOT_SCRIPT', build_vars['fakeroot_transformed'])
defs_conf.set('FAKED_BIN',       build_vars['faked_transformed'])

defs_file = configure_file(
  input:         'defs.in',
  output:        'defs',
  configuration: defs_conf,
)

# Environment shared by all tests.
# libfakeroot: the basename of the shared library (no path, no suffix)
# so that test/defs can find it in the build directory.
_libfakeroot_name = 'lib' + build_vars['fakeroot_release_name'] + build_vars['dlsuffix']

test_env = environment()
test_env.set('libfakeroot',  _libfakeroot_name)
test_env.set('srcdir',       meson.current_source_dir())
test_env.set('suffix',       '')
test_env.set('posixshell',   find_program('sh').full_path())
test_env.set('FAKED_BINDIR', meson.project_build_root())
test_env.set('FAKEDOPTS',    '')

# Prepend the build root and the test build dir to PATH so that the freshly
# built faked and fakeroot script are found before any system-installed ones.
test_env.prepend('PATH', meson.project_build_root(), meson.project_build_root() / 'scripts')

# All test scripts
_tests = [
  't.chmod_dev',
  't.chmod_symlink',
  't.chown',
  't.cp-a',
  't.echoarg',
  't.falsereturn',
  't.mknod',
  't.no_ld_preload',
  't.no_ld_preload_link',
  't.no_ld_preload_typeswitch',
  't.option',
  't.perlstat',
  't.tar',
  't.touchinstall',
  't.truereturn',
  't.xattr',
  't.xattr2',
]

foreach t : _tests
  test(
    t,
    find_program(t),
    env:          test_env,
    workdir:      meson.current_build_dir(),
    depends:      [libfakeroot, faked],
    timeout:      60,
  )
endforeach
