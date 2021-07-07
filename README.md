[![CI](https://github.com/ytret/os/actions/workflows/main.yml/badge.svg)](https://github.com/ytret/os/actions/workflows/main.yml)

## Building

### Dependencies

* `i686-elf-as`, `i686-elf-ld`

* `libcore`, `liballoc`

      $ rustup toolchain install nightly
      $ rustup component add rust-src

* [`libcompiler_builtins`](https://github.com/rust-lang/compiler-builtins)

To copy these into `lib/`, run:

    $ make get-libs

These libraries will be built the first time you run `make kernel`.

### Kernel

    $ make kernel  # or just `make'

### Userland

To build and run userspace C programs, one has to have a C standard library and
a GCC cross-compiler using that library.  For instructions on building and
installing the latter, see [toolchain/README.md](toolchain/README.md).  myos
userspace programs use a statically linked [port of
mlibc](https://github.com/ytret/mlibc).  Prepare a system root:

    $ make sysroot

And install the static libraries there after building them.

To build the userspace programs and install them into the system root, run:

    $ make userland

## Running

### Hard disk image

A hard disk image is created using `bximage` from Bochs:

    $ make hd

It creates a raw image with an ext2 file system inside and copies there
everything from the system root.  To sort of synchronize the contents of the
image with the system root, run:

    $ make sync

It copies new files to the disk image, but also removes files that are not
present in the system root from it.

### ISO

`kernel.iso` is created with `grub-mkrescue`, so you must have that tool
installed.

    $ make iso

### QEMU

    $ make run

### Bochs

    $ bochs -q

See `.bochsrc` for configuration options.
