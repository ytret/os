## Hosted GCC Cross-Compiler

This document describes building GNU Binutils and GCC which target myos.

    $ export TARGET=i686-myos
    $ export SYSROOT=/path/to/os/sysroot
    $ export PREFIX=$SYSROOT

Patch GNU Binutils 2.36.1 (`autoreconf` may require a specific version of
autoconf):

    $ cd binutils-2.36.1
    $ patch -p1 < ../binutils-2.36.1.patch
    $ cd ld && autoreconf

Patch GCC 10.2.0:

    $ cd gcc-10.2.0
    $ patch -p1 < ../gcc-10.2.0.patch
    $ cd libstdc++-v3 && autoreconf

Install the [C standard library](https://github.com/ytret/mlibc) headers:

    $ cd /path/to/ytret/mlibc
    $ $EDITOR meson_options.txt  # set `headers_only' to `true'
    $ meson builddir --cross-file cross.txt
    $ cd builddir && DESTDIR=$SYSROOT ninja install

Configure, build and install Binutils 2.36.1:

    $ mkdir build-binutils-2.36.1 && cd build-binutils-2.36.1
    $ ../binutils-2.36.1/configure --target=$TARGET --prefix=$PREFIX --with-sysroot=$SYSROOT --disable-werror --enable-shared
    $ make
    $ make install

Configure, build and install GCC 10.2.0:

    $ mkdir build-gcc-10.2.0 && cd build-gcc-10.2.0
    $ ../gcc-10.2.0/configure --target=$TARGET --prefix=$PREFIX --with-sysroot=$SYSROOT --enable-languages=c,c++ --enable-shared
    $ make all-gcc all-target-libgcc
    $ make install-gcc install-target-libgcc

Build and install the C++ standard library:

    $ cd build-gcc-10.2.0
    $ make all-target-libstdc++-v3
    $ make install-target-libstdc++-v3

[ytret_mlibc]: https://github.com/ytret/mlibc
