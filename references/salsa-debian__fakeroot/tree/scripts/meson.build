# scripts/meson.build — generate the fakeroot shell script from fakeroot.in

script_conf = configuration_data()
script_conf.set('prefix',                build_vars['prefix'])
script_conf.set('bindir',                build_vars['bindir'])
script_conf.set('libdir',                build_vars['libdir'])
script_conf.set('fakeroot_transformed',  build_vars['fakeroot_transformed'])
script_conf.set('faked_transformed',     build_vars['faked_transformed'])
script_conf.set('signal',                build_vars['signal'])
script_conf.set('SHEBANG_SHELL',         build_vars['shebang_shell'])
script_conf.set('VERSION',               build_vars['version'])
script_conf.set('DLSUFFIX',              build_vars['dlsuffix'])
script_conf.set('LDLIBPATHVAR',          build_vars['ldlibpathvar'])
script_conf.set('LDPRELOADVAR',          build_vars['ldpreloadvar'])
script_conf.set('LDPRELOADABS',          build_vars['ldpreloadabs'])
script_conf.set('LDEXTRAVAR',            build_vars['ldextravar'])

# On macOS, MACOSX_TRUE is empty (the block is active) and MACOSX_FALSE is
# commented out, matching automake's MACOSX conditional.  On other systems,
# the roles are reversed.
if is_darwin
  script_conf.set('MACOSX_TRUE',  '')
  script_conf.set('MACOSX_FALSE', '#')
else
  script_conf.set('MACOSX_TRUE',  '#')
  script_conf.set('MACOSX_FALSE', '')
endif

fakeroot_script = configure_file(
  input:         'fakeroot.in',
  output:        build_vars['fakeroot_transformed'],
  configuration: script_conf,
  install:       true,
  install_dir:   get_option('bindir'),
  install_mode:  'rwxr-xr-x',
)
