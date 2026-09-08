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

/*
  This file needs to work without any glibc headers.
*/

#ifndef _STAT_VER
 #if defined __linux__
  #if defined (__aarch64__)
   #define _STAT_VER 0
  #elif defined (__ia64__)
   #define _STAT_VER 1
  #elif defined (__powerpc__) && __WORDSIZE == 64
   #define _STAT_VER 1
  #elif defined (__riscv) && __riscv_xlen==64
   #define _STAT_VER 0
  #elif defined (__s390x__)
   #define _STAT_VER 1
  #elif defined (__x86_64__)
   #define _STAT_VER 1
  #else
   #define _STAT_VER 3
  #endif
 #elif defined __GNU__
   #define _STAT_VER 0
 #endif
#endif


