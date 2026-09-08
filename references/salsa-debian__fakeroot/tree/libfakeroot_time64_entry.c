/*
  Copyright © 1997, 1998, 1999, 2000, 2001  joost witteveen
  Copyright © 2002-2026  Clint Adams
  Copyright © 2012 Mikhail Gusarov
  Copyright © 2024 Chris Hofstaedtler

    This program is free software: you can redistribute it and/or modify
    it under the terms of the GNU General Public License as published by
    the Free Software Foundation, either version 3 of the License, or
    (at your option) any later version.

    This program is distributed in the hope that it will be useful,
    but WITHOUT ANY WARRANTY; without even the implied warranty of
    MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
    GNU General Public License for more details.

*/

#include "config.h"
#include "fakerootconfig.h"
#include "libfakeroot-internal.h"

/*
 glibc headers rename the asm symbol for lstat64, stat64, fstat64, fstatat64
 on 32bit time_t platforms, _if_ -D_TIME_BITS=64 is set.
 Unfortunately us redefining the functions then results in duplicate symbols
 when assembling.

 Solution: put these functions into its own .c, which does not include
 <sys/stat.h>.
*/
#ifndef NO_WRAP_LSTAT_SYMBOL
  extern int WRAP_LSTAT LSTAT_ARG(int ver, const char *file_name, void *statbuf);

  /* glibc exports both lstat and __xstat */
  int lstat(const char *file_name, void *statbuf) {
     return WRAP_LSTAT LSTAT_ARG(_STAT_VER, file_name, statbuf);
  }
#endif

#ifndef NO_WRAP_STAT_SYMBOL
  extern int WRAP_STAT STAT_ARG(int ver, const char *file_name, void *st);

  /* glibc exports both stat and __xstat */
  int stat(const char *file_name, void *st) {
     return WRAP_STAT STAT_ARG(_STAT_VER, file_name, st);
  }
#endif
#ifndef NO_WRAP_FSTAT_SYMBOL
  extern int WRAP_FSTAT FSTAT_ARG(int ver, int fd, void *st);

  /* glibc exports both fstat and __fxstat */
  int fstat(int fd, void *st) {
     return WRAP_FSTAT FSTAT_ARG(_STAT_VER, fd, st);
  }
#endif

  #if defined(HAVE_FSTATAT) && !defined(NO_WRAP_FSTATAT_SYMBOL)
    extern int WRAP_FSTATAT FSTATAT_ARG(int ver, int dir_fd, const char *path, void *st, int flags);

    /* glibc exports both fstatat and __fxstatat */
    int fstatat(int dir_fd, const char *path, void *st, int flags) {
       return WRAP_FSTATAT FSTATAT_ARG(_STAT_VER, dir_fd, path, st, flags);
    }
  #endif

    #ifndef NO_WRAP_LSTAT64_SYMBOL
    extern int WRAP_LSTAT64 LSTAT64_ARG (int ver, const char *file_name, void *st);

    /* glibc exports both lstat64 and __xstat64 */
    int lstat64(const char *file_name, void *st) {
       return WRAP_LSTAT64 LSTAT64_ARG(_STAT_VER, file_name, st);
    }
    #endif
    #ifndef NO_WRAP_STAT64_SYMBOL
    extern int WRAP_STAT64 STAT64_ARG(int ver, const char *file_name, void *st);

    /* glibc exports both stat64 and __xstat64 */
    int stat64(const char *file_name, void *st) {
       return WRAP_STAT64 STAT64_ARG(_STAT_VER, file_name, st);
    }
    #endif
    #ifndef NO_WRAP_FSTAT64_SYMBOL
    extern int WRAP_FSTAT64 FSTAT64_ARG(int ver, int fd, void *st);

    /* glibc exports both fstat64 and __fxstat64 */
    int fstat64(int fd, void *st) {
       return WRAP_FSTAT64 FSTAT64_ARG(_STAT_VER, fd, st);
    }
    #endif

    #if defined(HAVE_FSTATAT) && !defined(NO_WRAP_FSTATAT64_SYMBOL)
    extern int WRAP_FSTATAT64 FSTATAT64_ARG(int ver, int dir_fd, const char *path, void *st, int flags);

    /* glibc exports both fstatat64 and __fxstatat64 */
      int fstatat64 (int dir_fd, const char *path, void *st, int flags) {
	 return WRAP_FSTATAT64 FSTATAT64_ARG(_STAT_VER, dir_fd, path, st, flags);
      }
    #endif
