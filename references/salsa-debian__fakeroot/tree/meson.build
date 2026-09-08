project(
  'fakeroot',
  'c',
  version: '2.1.4',
  license: 'GPL-3.0-or-later',
  meson_version: '>= 0.61.0',
  default_options: ['warning_level=1'],
)

cc = meson.get_compiler('c')

# =========================================================================
# Platform detection
# =========================================================================
is_darwin = host_machine.system() == 'darwin'

# =========================================================================
# IPC method
# =========================================================================
ipc_opt = get_option('ipc')
if ipc_opt == 'auto'
  if meson.is_cross_build()
    if host_machine.system().startswith('linux')
      use_ipc = 'sysv'
    else
      error(
        'Cross-compiling for ' + host_machine.system() + ': cannot auto-detect IPC. '
        + 'Set -Dipc=sysv or -Dipc=tcp explicitly.',
      )
    endif
  else
    ipc_run = cc.run(
      '''
      #include <stdlib.h>
      #include <sys/types.h>
      #include <sys/ipc.h>
      #include <sys/msg.h>
      #include <time.h>
      #include <unistd.h>
      int main(void) {
        srandom(time(NULL) + getpid() * 33151);
        key_t msg_key = random();
        int mg = msgget(msg_key, IPC_CREAT | 0600);
        if (mg == -1) return 1;
        msgctl(mg, IPC_RMID, NULL);
        return 0;
      }
      ''',
      name: 'SysV IPC message queues work',
    )
    if ipc_run.compiled() and ipc_run.returncode() == 0
      use_ipc = 'sysv'
    else
      use_ipc = 'tcp'
    endif
  endif
else
  use_ipc = ipc_opt
endif
message('IPC method: ' + use_ipc)

# =========================================================================
# Database format
# =========================================================================
use_dbformat = get_option('dbformat')

# =========================================================================
# Shebang shell
# =========================================================================
sh_opt = get_option('sh')
shebang_shell = sh_opt
message('Shebang shell: ' + shebang_shell)

# Program naming
program_prefix = get_option('program_prefix')
program_suffix = get_option('program_suffix')
message('Program prefix: ' + program_prefix)
message('Program suffix: ' + program_suffix)

fakeroot_transformed = program_prefix + 'fakeroot' + program_suffix
faked_transformed = program_prefix + 'faked' + program_suffix

message('Transformed fakeroot name: ' + fakeroot_transformed)
message('Transformed faked name: ' + faked_transformed)
fakeroot_release_name = fakeroot_transformed + '-0'
message('Transformed fakeroot release name: ' + fakeroot_release_name)

# =========================================================================
# Signal used to terminate faked
# =========================================================================
term_signal = (use_ipc == 'tcp') ? 'HUP' : 'TERM'

# =========================================================================
# Platform-specific shared-library / LD_PRELOAD variables
# =========================================================================
if is_darwin
  dlsuffix    = '.dylib'
  ldlibpathvar = 'DYLD_LIBRARY_PATH'
  ldpreloadvar = 'DYLD_INSERT_LIBRARIES'
  ldpreloadabs = '1'
  ldextravar   = ''
else
  dlsuffix    = '.so'
  ldlibpathvar = 'LD_LIBRARY_PATH'
  ldpreloadvar = 'LD_PRELOAD'
  ldpreloadabs = '0'
  ldextravar   = ''
endif

# Path to the C library (used in faked for dlopen fallback)
cpu = host_machine.cpu_family()
os  = host_machine.system()
if cpu in ['alpha', 'ia64'] and os == 'linux'
  libcpath = '/lib/libc.so.6.1'
elif os == 'linux'
  libcpath = '/lib/libc.so.6'
elif os in ['kfreebsd', 'knetbsd']
  libcpath = '/lib/libc.so.0.1'
elif os == 'freebsd'
  libcpath = '/usr/lib/libc.so.4'
elif os == 'netbsd'
  libcpath = '/usr/lib/libc.so.12'
elif os in ['openbsd', 'mirbsd']
  libcpath = '/usr/lib/libc.so'
elif os == 'hpux'
  libcpath = '/usr/lib/hpux32/libc.so.1'
elif os.startswith('osf')
  libcpath = '/shlib/libc.so'
elif os == 'sunos'
  libcpath = '/lib/libc.so.1'
elif os == 'darwin'
  libcpath = '/usr/lib/libSystem.dylib'
else
  warning('Unknown OS ' + os + ': defaulting libcpath to /lib/libc.so')
  libcpath = '/lib/libc.so'
endif

# =========================================================================
# Dependencies
# =========================================================================
libdl       = cc.find_library('dl',      required: false)
libpthread  = []
libsocket   = []
if use_ipc == 'tcp'
  libpthread = cc.find_library('pthread', required: true)
  libsocket  = cc.find_library('socket',  required: false)
endif

# =========================================================================
# configuration_data for config.h
# =========================================================================
conf = configuration_data()

# ── Package metadata ──────────────────────────────────────────────────────
conf.set_quoted('PACKAGE',           'fakeroot')
conf.set_quoted('PACKAGE_NAME',      'fakeroot')
conf.set_quoted('PACKAGE_TARNAME',   'fakeroot')
conf.set_quoted('PACKAGE_VERSION',   meson.project_version())
conf.set_quoted('PACKAGE_STRING',    'fakeroot ' + meson.project_version())
conf.set_quoted('PACKAGE_BUGREPORT', 'clint@debian.org')
conf.set_quoted('VERSION',           meson.project_version())

# ── Headers ───────────────────────────────────────────────────────────────
foreach h : [
  'fcntl.h', 'unistd.h', 'features.h', 'sys/feature_tests.h',
  'pthread.h', 'stdint.h', 'inttypes.h', 'grp.h', 'endian.h',
  'machine/endian.h', 'sys/sysmacros.h', 'sys/socket.h', 'sys/acl.h',
  'sys/capability.h', 'sys/xattr.h', 'fts.h', 'ftw.h',
  'sys/types.h', 'dirent.h', 'sys/stat.h',
  'sys/ipc.h', 'sys/msg.h', 'sys/sem.h',
]
  if cc.has_header(h)
    conf.set('HAVE_' + h.underscorify().to_upper(), 1)
  endif
endforeach

# ── Type aliases (fallback for ancient platforms) ─────────────────────────
foreach t : [['mode_t', 'int'], ['off_t', 'long'], ['size_t', 'unsigned'], ['id_t', 'int']]
  if not cc.has_type(t[0], prefix: '#include <sys/types.h>')
    conf.set(t[0], t[1])
  endif
endforeach

# ── setenv replacement ────────────────────────────────────────────────────
setenv_sources = []
if not cc.has_function('setenv', prefix: '#include <stdlib.h>')
  setenv_sources = ['setenv.c']
endif

# ── Functions ─────────────────────────────────────────────────────────────
foreach f : [
  'fchmodat', 'fchownat', 'fstatat', 'mkdirat', 'mknodat', 'openat',
  'renameat', 'renameat2', 'unlinkat', 'lchmod', 'fgetattrlist',
  'capset', 'listxattr', 'llistxattr', 'flistxattr',
  'getxattr', 'lgetxattr', 'fgetxattr',
  'setxattr', 'lsetxattr', 'fsetxattr',
  'removexattr', 'lremovexattr', 'fremovexattr',
  'statx',
  'strdup', 'strstr',
  'getresuid', 'setresuid', 'getresgid', 'setresgid',
  'setfsuid', 'setfsgid',
  'fts_read', 'fts_children',
  'close_range', 'nftw',
  'setenv', 'unsetenv',
]
  if cc.has_function(f)
    conf.set('HAVE_' + f.to_upper(), 1)
  endif
endforeach

# ── Declarations ──────────────────────────────────────────────────────────
foreach f : ['setenv', 'unsetenv']
  conf.set(
    'HAVE_DECL_' + f.to_upper(),
    cc.has_header_symbol('stdlib.h', f) ? 1 : 0,
  )
endforeach

# ── ACL libraries and type ────────────────────────────────────────────────
libacl = []
_acl_deps = []
_lacl = cc.find_library('acl', required: false)
if _lacl.found()
  _acl_deps += [_lacl]
endif
if cc.has_function(
  'acl_get_fd',
  prefix: '#include <sys/types.h>\n#include <sys/acl.h>',
  dependencies: _acl_deps,
)
  conf.set('HAVE_ACL_GET_FD', 1)
  libacl += _acl_deps
endif
_sec_deps = []
_lsec = cc.find_library('sec', required: false)
if _lsec.found()
  _sec_deps += [_lsec]
endif
if cc.has_function('acl_trivial', dependencies: _sec_deps)
  conf.set('HAVE_ACL_TRIVIAL', 1)
  libacl += _sec_deps
endif
if cc.has_type('acl_t', prefix: '#include <sys/types.h>\n#include <sys/acl.h>')
  conf.set('HAVE_ACL_T', 1)
endif

# ── __builtin_expect ──────────────────────────────────────────────────────
if cc.links(
  'int foo(int a) { a = __builtin_expect(a, 10); return a == 10 ? 0 : 1; }',
  name: '__builtin_expect',
)
  conf.set('HAVE_BUILTIN_EXPECT', 1)
endif

# ── IPC and DB configuration ──────────────────────────────────────────────
if use_ipc == 'tcp'
  conf.set('FAKEROOT_FAKENET', 'TCP')
endif
if use_dbformat == 'path'
  conf.set('FAKEROOT_DB_PATH', 1)
endif

# ── readlink signature ────────────────────────────────────────────────────
_readlink_retval      = 'int'
_readlink_buf_arg     = 'void'
_readlink_bufsize_arg = 'int'
foreach zeroth : ['ssize_t', 'int']
  foreach second : ['void', 'char']
    foreach third : ['size_t', 'int']
      if _readlink_retval == 'int' or _readlink_buf_arg == 'void'
        if cc.compiles(
          '#include <unistd.h>\n#include <stdio.h>\n'
          + zeroth + ' readlink(const char *path, '
          + second + ' *buf, ' + third + ' bufsiz);'
          + '\nint main(void){puts("x");return 0;}',
          name: 'readlink(' + zeroth + ',' + second + '*,' + third + ')',
        )
          _readlink_retval      = zeroth
          _readlink_buf_arg     = second
          _readlink_bufsize_arg = third
        endif
      endif
    endforeach
  endforeach
endforeach
conf.set('READLINK_RETVAL_TYPE',  _readlink_retval)
conf.set('READLINK_BUF_TYPE',     _readlink_buf_arg)
conf.set('READLINK_BUFSIZE_TYPE', _readlink_bufsize_arg)

# ── setgroups first argument type ─────────────────────────────────────────
_setgroups_size = 'int'
if cc.compiles(
  '#define _BSD_SOURCE\n'
  + '#include <sys/types.h>\n#include <unistd.h>\n#include <grp.h>\n'
  + 'int setgroups(size_t size, const gid_t *list);\n'
  + 'int main(void){return 0;}',
  name: 'setgroups(size_t,...)',
)
  _setgroups_size = 'size_t'
endif
conf.set('SETGROUPS_SIZE_TYPE', _setgroups_size)

# ── union semun ───────────────────────────────────────────────────────────
if cc.compiles(
  '#include <sys/types.h>\n#include <sys/ipc.h>\n#include <sys/sem.h>\n'
  + 'int main(void){union semun s;(void)s;return 0;}',
  name: 'union semun',
)
  conf.set('HAVE_SEMUN_DEF', 1)
endif

# ── __xmknod fourth argument ──────────────────────────────────────────────
_xmknod_frth = ''
if cc.has_function('__xmknod')
  if cc.compiles(
    '#include <sys/types.h>\n#include <sys/stat.h>\n'
    + '#include <fcntl.h>\n#include <unistd.h>\n'
    + '#ifndef __GLIBC__\n#error no extra *\n#endif\n'
    + 'int __xmknod(int ver,const char *p,mode_t mode,dev_t *dev);\n'
    + 'int main(void){return 0;}',
    name: '__xmknod dev_t*',
  )
    _xmknod_frth = '*'
  endif
endif
conf.set('XMKNOD_FRTH_ARG', _xmknod_frth)

# ── __xmknodat fifth argument ─────────────────────────────────────────────
_xmknodat_fifth = ''
if cc.has_function('__xmknodat')
  if cc.compiles(
    '#include <sys/types.h>\n#include <sys/stat.h>\n'
    + '#include <fcntl.h>\n#include <unistd.h>\n'
    + '#ifndef __GLIBC__\n#error no extra *\n#endif\n'
    + 'int __xmknodat(int ver,int dirfd,const char *p,mode_t mode,dev_t *dev);\n'
    + 'int main(void){return 0;}',
    name: '__xmknodat dev_t*',
  )
    _xmknodat_fifth = '*'
  endif
endif
conf.set('XMKNODAT_FIFTH_ARG', _xmknodat_fifth)

# ── initgroups second argument ────────────────────────────────────────────
_initgroups_arg = 'int'
if cc.compiles(
  '#include <sys/types.h>\n#include <unistd.h>\n'
  + 'int initgroups(const char *user, gid_t group);\n'
  + 'int main(void){return 0;}',
  name: 'initgroups gid_t',
)
  _initgroups_arg = 'gid_t'
endif
conf.set('INITGROUPS_SECOND_ARG', _initgroups_arg)

# ── setreuid argument type ────────────────────────────────────────────────
# configure.ac sets SETREUID_ARG=gid_t when uid_t signature compiles (historical)
_setreuid_arg = 'int'
if cc.compiles(
  '#include <sys/types.h>\n#include <unistd.h>\n'
  + 'int setreuid(uid_t ruid, uid_t euid);\n'
  + 'int main(void){return 0;}',
  name: 'setreuid uid_t',
)
  _setreuid_arg = 'gid_t'
endif
conf.set('SETREUID_ARG', _setreuid_arg)

# ── setregid argument type ────────────────────────────────────────────────
_setregid_arg = 'int'
if cc.compiles(
  '#include <sys/types.h>\n#include <unistd.h>\n'
  + 'int setreuid(gid_t rgid, gid_t egid);\n'
  + 'int main(void){return 0;}',
  name: 'setregid gid_t',
)
  _setregid_arg = 'gid_t'
endif
conf.set('SETREGID_ARG', _setregid_arg)

# ── stat second argument type ─────────────────────────────────────────────
_stat_second = ''
if cc.compiles(
  '#include <sys/stat.h>\n#include <sys/types.h>\n#include <unistd.h>\n'
  + 'int stat(const char *file_name, struct stat *buf);\n'
  + 'int main(void){return 0;}',
  name: 'stat struct stat*',
)
  _stat_second = 'struct stat *'
elif cc.compiles(
  '#include <sys/stat.h>\n#include <sys/types.h>\n#include <unistd.h>\n'
  + 'int stat(const char *file_name, stat *buf);\n'
  + 'int main(void){return 0;}',
  name: 'stat stat*',
)
  _stat_second = 'stat *'
else
  error('Cannot determine second argument type of stat()')
endif
conf.set('STAT_SECOND_ARG', _stat_second)

# ── struct dirent fields ──────────────────────────────────────────────────
foreach field : ['d_off', 'd_type']
  if cc.has_member(
    'struct dirent', field,
    prefix: '#include <sys/types.h>\n#include <dirent.h>',
  )
    conf.set('STAT_HAS_' + field, 1)
  endif
endforeach

# ── Alpha stat-struct conversion hackery ─────────────────────────────────
if cpu == 'alpha' and os == 'linux'
  if cc.compiles(
    '#include <sys/stat.h>\n'
    + '#if _STAT_VER != _STAT_VER_GLIBC2_3_4\n#error not 3:3\n#endif\n'
    + 'int main(void){return 0;}',
    name: 'alpha stat-struct conversion hack',
  )
    conf.set('STUPID_ALPHA_HACK', 1)
  endif
endif

# ── libcpath ──────────────────────────────────────────────────────────────
conf.set_quoted('LIBCPATH', libcpath)

# =========================================================================
# stat/mknod wrapper function detection → config.h + fakerootconfig.h
#
# Each entry: [func_suffix, prefix]
# Mirrors configure.ac's "for SEARCH in %stat f%stat l%stat ..." loop.
# =========================================================================
# Defines placed in fakerootconfig.h (function-arg macros can't go in config.h)
fakerootconfig_content = ''

# stat_searches: [func, pre]  →  PF = (pre+func).upper()
stat_searches = [
  ['stat',     ''],    # %stat     → STAT
  ['stat',     'f'],   # f%stat    → FSTAT
  ['stat',     'l'],   # l%stat    → LSTAT
  ['statat',   'f'],   # f%statat  → FSTATAT
  ['stat64',   ''],    # %stat64   → STAT64
  ['stat64',   'f'],   # f%stat64  → FSTAT64
  ['stat64',   'l'],   # l%stat64  → LSTAT64
  ['statat64', 'f'],   # f%statat64 → FSTATAT64
  ['mknod',    ''],    # %mknod    → MKNOD
  ['mknodat',  ''],    # %mknodat  → MKNODAT
]

foreach search : stat_searches
  _func = search[0]
  _pre  = search[1]
  _pf   = (_pre + _func).to_upper()

  # Candidate functions in probe order (matches configure.ac)
  _xcand1 = '__' + _pre + 'x' + _func     # e.g. __xstat
  _xcand2 = '_'  + _pre + 'x' + _func     # e.g.  _xstat
  _candidates = [
    _xcand1,
    _xcand2,
    '__' + _pre + _func + '13',            # e.g. __stat13
    _pre + _func,                          # e.g.  stat
    '__' + _pre + _func,                   # e.g. __stat
  ]

  # DEF_END: extra arguments in the macro parameter list
  _def_end = ''
  if _func == 'mknod'
    _def_end = ',d'
  elif _func in ['mknodat', 'statat', 'statat64']
    _def_end = ',d,e'
  endif

  _found = ''
  foreach cand : _candidates
    _ac_check_funcs_probe = (
      '#ifdef ' + cand + '\n'
      + '# undef ' + cand + '\n'
      + '#endif\n'
      + 'char ' + cand + '();\n'
      + 'int main(void){ return ' + cand + '(); }\n'
    )
    if _found == '' and cc.links(_ac_check_funcs_probe, name: 'ac_check_funcs ' + cand)
      _found = cand
    endif
  endforeach

  if _found != ''
    # Simple defines → config.h
    conf.set(_pf.join(['WRAP_', '']),            _found)
    conf.set(_pf.join(['WRAP_', '_RAW']),        _found)
    conf.set_quoted(_pf.join(['WRAP_', '_QUOTE']), _found)
    conf.set(_pf.join(['TMP_',  '']),            'tmp_' + _found)
    conf.set(_pf.join(['NEXT_', '_NOARG']),      'next_' + _found)

    # DEF_BEGIN: "a," only when found is an x-variant (carries version arg)
    _def_begin = (_found == _xcand1 or _found == _xcand2) ? 'a,' : ''

    # Function-arg macros → fakerootconfig.h
    fakerootconfig_content += (
      '#define NEXT_' + _pf + '(a,b,c' + _def_end + ')'
      + ' next_' + _found + '(' + _def_begin + 'b,c' + _def_end + ')\n'
    )
    fakerootconfig_content += (
      '#define ' + _pf + '_ARG(a,b,c' + _def_end + ')'
      + ' (' + _def_begin + 'b,c' + _def_end + ')\n'
    )

    # Flag when no extra wrapper symbol is needed
    if _found == _pre + _func
      conf.set('NO_WRAP_' + _pf + '_SYMBOL', 1)
    endif
  endif
endforeach

# ── time64 hack ───────────────────
time64_hack = false
foreach search : ['stat64_time64', 'fstat64_time64', 'lstat64_time64', 'fstatat64_time64']
  _pf2 = search.to_upper()
  _found2 = ''
  foreach cand : ['__' + search, search]
    _ac_check_funcs_probe = (
      '#ifdef ' + cand + '\n'
      + '# undef ' + cand + '\n'
      + '#endif\n'
      + 'char ' + cand + '();\n'
      + 'int main(void){ return ' + cand + '(); }\n'
    )
    if _found2 == '' and cc.links(_ac_check_funcs_probe, name: 'ac_check_funcs ' + cand)
      _found2 = cand
    endif
  endforeach

  if _found2 != ''
    time64_hack = true
    conf.set('WRAP_' + _pf2,              _found2)
    conf.set('WRAP_' + _pf2 + '_RAW',    _found2)
    conf.set_quoted('WRAP_' + _pf2 + '_QUOTE', _found2)
    conf.set('TMP_'  + _pf2,             'tmp_' + _found2)
    conf.set('NEXT_' + _pf2 + '_NOARG',  'next_' + _found2)

    _def_end2 = search.endswith('statat64_time64') ? ',d,e' : ''
    fakerootconfig_content += (
      '#define NEXT_' + _pf2 + '(a,b,c' + _def_end2 + ')'
      + ' next_' + _found2 + '(b,c' + _def_end2 + ')\n'
    )
    fakerootconfig_content += (
      '#define ' + _pf2 + '_ARG(a,b,c' + _def_end2 + ')'
      + ' (b,c' + _def_end2 + ')\n'
    )
  endif
endforeach

if time64_hack
  conf.set('TIME64_HACK', 1)
endif

# =========================================================================
# Generate fakerootconfig.h
# =========================================================================
fakerootconfig_conf = configuration_data()
fakerootconfig_conf.set('FAKEROOTCONFIG_CONTENT', fakerootconfig_content)
fakerootconfig_h = configure_file(
  input:         'fakerootconfig.h.in',
  output:        'fakerootconfig.h',
  configuration: fakerootconfig_conf,
)

# =========================================================================
# Generate config.h
# =========================================================================
config_h = configure_file(
  input:         'config.meson.h.in',
  output:        'config.h',
  configuration: conf,
)

# =========================================================================
# Generate wrapped headers (Python replacement for wrapawk / wrapawk_macosx)
# =========================================================================
python3       = find_program('python3')
gen_wrapped   = files('tools/gen_wrapped_headers.py')
wrapawk_mode  = is_darwin ? 'macosx' : 'linux'

wrap_headers = custom_target(
  'wrap_headers',
  input:   ['wrapfunc.inp'],
  output:  ['wrapped.h', 'wrapdef.h', 'wrapstruct.h', 'wraptmpf.h'],
  command: [python3, gen_wrapped, '@INPUT@', '@OUTDIR@', wrapawk_mode],
)

# =========================================================================
# Include directories
# =========================================================================
root_inc  = include_directories('.')
build_inc = include_directories('.')   # picks up generated headers from build dir

extra_inc = []
if cpu == 'alpha' and os == 'linux' and conf.get('STUPID_ALPHA_HACK', 0) == 1
  extra_inc = [include_directories('statconv/glibc/linux/alpha')]
endif

all_inc = [root_inc] + extra_inc

# =========================================================================
# libcommunicate  (static, internal)
# =========================================================================
libcommunicate = static_library(
  'communicate',
  ['communicate.c'] + setenv_sources,
  include_directories: all_inc,
  implicit_include_directories: false,
  dependencies: [libdl, libpthread, libsocket],
)
libcommunicate_dep = declare_dependency(
  link_with:           libcommunicate,
  include_directories: all_inc,
)

# =========================================================================
# libmacosx  (static, macOS only)
# =========================================================================
libmacosx_libs = []   # static library objects for link_with
if is_darwin
  libmacosx = static_library(
    'macosx',
    ['libfakeroot_inode64.c', 'libfakeroot_unix2003.c'],
    include_directories: all_inc,
    implicit_include_directories: false,
  )
  libmacosx_libs = [libmacosx]
endif

# =========================================================================
# libfakeroot_time64  (static, internal)
# =========================================================================
libfakeroot_time64 = static_library(
  'fakeroot_time64',
  ['libfakeroot_time64.c', wrap_headers],
  c_args: ['-D_FILE_OFFSET_BITS=64', '-D_TIME_BITS=64'],
  include_directories: all_inc,
  implicit_include_directories: false,
  dependencies: [libdl, libpthread, libsocket],
)
libfakeroot_time64_dep = declare_dependency(link_with: libfakeroot_time64)

# =========================================================================
# lib<fakeroot_transformed>-0  (installed shared library, replaces libfakeroot.la)
#
# libtool -release 0 produces libfakeroot-0.so (dash, not soversion dot).
# We achieve the same with the transformed stem by naming the Meson target
# <fakeroot_transformed>-0.
# =========================================================================
_libfakeroot_link = [libfakeroot_time64, libcommunicate] + libmacosx_libs
_libfakeroot_c_args = []

if not time64_hack
    _libfakeroot_c_args += ['-U_FILE_OFFSET_BITS', '-U_TIME_BITS']
endif

libfakeroot = shared_library(
  fakeroot_release_name,
  ['libfakeroot.c', 'libfakeroot_time64_entry.c', wrap_headers],
  c_args: _libfakeroot_c_args,
  include_directories: all_inc,
  implicit_include_directories: false,
  link_whole: _libfakeroot_link,
  dependencies: [libdl, libpthread, libsocket] + libacl,
  install: true,
  install_dir: get_option('libdir'),
)

# Install the ABI symlink: lib<fakeroot_transformed>.{so,dylib}
# -> lib<fakeroot_transformed>-0.{so,dylib}
install_symlink(
  'lib' + fakeroot_transformed + dlsuffix,
  pointing_to: 'lib' + fakeroot_release_name + dlsuffix,
  install_dir: get_option('libdir'),
)

# =========================================================================
# faked  (the daemon)
# =========================================================================
faked = executable(
  faked_transformed,
  ['faked.c'],
  include_directories: all_inc,
  implicit_include_directories: false,
  dependencies: [libcommunicate_dep, libdl, libpthread, libsocket],
  install: true,
)

# =========================================================================
# simple  (non-installed internal test program)
# =========================================================================
executable(
  'simple',
  ['simple.c'],
  include_directories: all_inc,
  implicit_include_directories: false,
  install: false,
)

# =========================================================================
# Expose build-time variables for subdirectories
# =========================================================================
# These are referenced by scripts/meson.build and test/meson.build.
build_vars = {
  'prefix':        get_option('prefix'),
  'bindir':        get_option('prefix') / get_option('bindir'),
  'libdir':        get_option('prefix') / get_option('libdir'),
  'dlsuffix':      dlsuffix,
  'ldlibpathvar':  ldlibpathvar,
  'ldpreloadvar':  ldpreloadvar,
  'ldpreloadabs':  ldpreloadabs,
  'ldextravar':    ldextravar,
  'signal':        term_signal,
  'shebang_shell': shebang_shell,
  'fakeroot_release_name': fakeroot_release_name,
  'fakeroot_transformed': fakeroot_transformed,
  'faked_transformed': faked_transformed,
  'version':       meson.project_version(),
}

# =========================================================================
# Subdirectories
# =========================================================================
subdir('scripts')
if get_option('docs')
  subdir('doc')
endif
subdir('test')
