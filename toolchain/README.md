## Hosted GCC Cross-Compiler

This document describes building GNU Binutils and GCC which target myos.

1. Set environment variables:

```sh
$ export TARGET=i686-myos
$ export SYSROOT=/path/to/os/sysroot
$ export PREFIX=$SYSROOT
```

2. Patch GNU Binutils 2.36.1 (`autoreconf` may require a specific version of
autoconf):

```sh
$ cd binutils-2.36.1
$ patch -p1 < ../binutils-2.36.1.patch
$ cd ld && autoreconf
```

3. Patch GCC 10.2.0:

```sh
$ cd gcc-10.2.0
$ patch -p1 < ../gcc-10.2.0.patch
$ cd libstdc++-v3 && autoreconf
```

4. Install the [C standard library][ytret_mlibc] headers:

```sh
$ cd /path/to/ytret/mlibc
$ $EDITOR meson_options.txt  # set `headers_only' to `true'
$ meson builddir --cross-file ci/myos.cross-file
$ cd builddir && DESTDIR=$SYSROOT ninja install
```

5. Configure, build and install Binutils 2.36.1:

```sh
$ mkdir build-binutils-2.36.1 && cd build-binutils-2.36.1
$ ../binutils-2.36.1/configure --target=$TARGET --prefix=$PREFIX --with-sysroot=$SYSROOT --disable-werror --enable-shared
$ make
$ make install
```

6. Configure, build and install GCC 10.2.0:

```sh
$ mkdir build-gcc-10.2.0 && cd build-gcc-10.2.0
$ ../gcc-10.2.0/configure --target=$TARGET --prefix=$PREFIX --with-sysroot=$SYSROOT --enable-languages=c,c++ --enable-shared
$ make all-gcc all-target-libgcc
$ make install-gcc install-target-libgcc
```

7. Build and install the [C standard library][ytret_mlibc]:

```sh
$ cd /path/to/ytret/mlibc
$ $EDITOR meson_options.txt  # set `headers_only` to `false', `static' to `true'
$ meson builddir --cross-file ci/myos.cross-file
$ cd builddir && DESTDIR=$SYSROOT ninja install
```

8. Build and install the C++ standard library:

```sh
$ cd build-gcc-10.2.0
$ make all-target-libstdc++-v3
$ make install-target-libstdc++-v3
```

[ytret_mlibc]: https://github.com/ytret/mlibc
