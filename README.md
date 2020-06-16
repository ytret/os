## Compilation

_Assuming you have installed [the cross-compiled GNU binutils][gcc_cross_comp]._

If you are building the kernel for the first time, you have to compile `libcore`
and its prerequisite `libcompiler_builtins` for the selected target (set in
`Makefile`).  The former is available in your `rustc` sysroot directory (see
`rustc --print sysroot`) after adding the `rust-src` component, and the latter
is fetched from [rust-lang/compiler-builtins][libcomp_github].  To copy these
two into the `lib/` subdirectory, simply run:

    $ make get-libs

Once you have the source code for `libcore` and `libcompiler_builtins`, you can
compile these crates and the kernel and link everything together:

    $ make

The final ELF binary is called `kernel.bin`.  If you ever need to recompile the
two dependencies, you can safely delete the `lib/` directory and run the
`get-libs` recipe again.

[gcc_cross_comp]: https://wiki.osdev.org/GCC_Cross-Compiler
[libcomp_github]: https://github.com/rust-lang/compiler-builtins

## Running

The kernel is booted using [the Multiboot2 specification][multiboot2_spec] by
GRUB2.  The disk image `kernel.iso` is created with `grub-mkrescue`, so you need
to have this tool installed.  To create the ISO and jump into the QEMU emulator,
run:

    $ make iso run

Alternatively, you can run the kernel in Bochs:

    $ make iso && bochs

See `.bochsrc` for configuration options.

[multiboot2_spec]: https://www.gnu.org/software/grub/manual/multiboot2/multiboot.html
