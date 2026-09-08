# Installing fakeroot from source

## Introduction

As an experiment, fakeroot currently supports two build systems,
with a primary goal of no longer shipping autotools-generated
files (and po4a-generated files) in release tarballs.  One of
them will probably go away in the future.

You will need po4a installed if you want man page translations.

The following examples show how to build and install the TCP
version of fakeroot, rather than with SYSV IPC (the default).

## autotools

Ensure that you have autoconf, automake, and libtool installed.

1. autoreconf -fi
2. ./configure --with-ipc=tcp
3. make
4. make check
5. sudo make install

## meson

Ensure that you have meson or muon and ninja installed.

### with meson

1. meson setup -D ipc=tcp build
2. meson compile -C build
3. meson test -C build
4. sudo meson install -C build

### with muon

1. muon setup -D ipc=tcp build
2. ninja -C build
3. cd build
4. muon test
5. sudo muon install

