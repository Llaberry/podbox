AUTOMAKE_OPTIONS=foreign

TESTS =						\
  t.chmod_dev					\
  t.chmod_symlink				\
  t.chown					\
  t.cp-a					\
  t.echoarg					\
  t.falsereturn					\
  t.mknod					\
  t.no_ld_preload				\
  t.no_ld_preload_link				\
  t.no_ld_preload_typeswitch			\
  t.option					\
  t.perlstat					\
  t.tar						\
  t.touchinstall				\
  t.truereturn					\
  t.xattr					\
  t.xattr2

suffix =

TESTS_ENVIRONMENT =				\
  libfakeroot=libfakeroot-0$(DLSUFFIX)		\
  srcdir=$(srcdir)				\
  suffix=$(suffix)				\
  posixshell=$(SHELL)

CLEANFILES = tmp* tartest.tar.gz hda3

check_SCRIPTS = $(TESTS) compare-tar cp-atest defs echo_arg tartest perlstattest
EXTRA_DIST = $(check_SCRIPTS) tartest.tar.gz.uue
