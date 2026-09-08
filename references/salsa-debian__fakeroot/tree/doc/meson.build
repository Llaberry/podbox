# doc/meson.build — man page installation

# Install English man pages using transformed program names so
# -Dprogram_prefix/-Dprogram_suffix apply consistently.
_manpages_source = ['faked.1', 'fakeroot.1']
_manpages_transformed = [
  build_vars['faked_transformed'] + '.1',
  build_vars['fakeroot_transformed'] + '.1',
]
install_data(
  _manpages_source,
  rename : _manpages_transformed,
  install_dir : get_option('mandir') / 'man1',
)

# =========================================================================
# Translated man pages via po4a-translate
# =========================================================================
po4a = find_program('po4a', required: false)

if po4a.found()
  foreach _po4a_lang : ['de', 'es', 'fr', 'nl', 'pt', 'ro', 'sv']
    # subdir() inherits parent variables (_po4a_lang, po4a).
    # Inside, custom_target outputs are flat and land in builddir/doc/$lang/.
    subdir(_po4a_lang)
  endforeach
else
  warning('po4a not found; installing English man pages only')
endif
