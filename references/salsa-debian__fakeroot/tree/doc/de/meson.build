# Translated man pages for locale _po4a_lang (set by parent doc/meson.build).
# Outputs are flat here; Meson places them in builddir/doc/<lang>/.
# po4a --no-update regenerates translations without touching PO/POT files.
# po4a writes to a temporary directory; outputs are then copied to transformed names.
custom_target(
  'translate-' + _po4a_lang,
  input : [
    meson.project_source_root() / 'doc' / 'po4a' / 'po4a.cfg',
    meson.project_source_root() / 'doc' / 'po4a' / 'po' / _po4a_lang + '.po',
    meson.project_source_root() / 'doc' / 'po4a' / 'addendum.1.' + _po4a_lang,
    meson.project_source_root() / 'doc' / 'faked.1',
    meson.project_source_root() / 'doc' / 'fakeroot.1',
  ],
  output : _manpages_transformed,
  command : [
    'sh',
    '-ec',
    'tmp="@OUTDIR@/po4a-output"; '
    + 'rm -rf "$tmp"; mkdir -p "$tmp"; '
    + '"$1" --no-update -k 0 --target-lang "$2" --variable "$3" --destdir "$tmp" "$4"; '
    + 'cp "$tmp/$2/faked.1" "@OUTPUT0@"; '
    + 'cp "$tmp/$2/fakeroot.1" "@OUTPUT1@"',
    'sh',
    po4a,
    _po4a_lang,
    'srcdir=' + meson.project_source_root() / 'doc' / '',
    '@INPUT0@',
  ],
  build_by_default : true,
  install : true,
  install_dir : get_option('mandir') / _po4a_lang / 'man1',
)
