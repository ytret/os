.PHONY: all clean install

ARCH ?= x86
ARCHDIR := $(PWD)/kernel/arch/$(ARCH)
include $(ARCHDIR)/Makefile.inc

LIBDIR ?= $(PWD)/lib
ISODIR ?= $(PWD)/isodir

AS := i686-elf-as
LD := i686-elf-ld
RUST := rustc
CARGO := cargo

RUSTFLAGS := --target $(ARCHDIR)/target.json
# --edition 2018 is passed only in some of the rules

# kernel/main.rs must be first (see the $(LIBKERNEL) rule)
SOURCES := \
	kernel/main.rs \
	kernel/vga.rs

OBJECTS := \
    $(ARCH_OBJECTS)

LIBKERNEL := $(LIBDIR)/libkernel.a
LIBCORE := $(LIBDIR)/libcore.rlib
LIBCOMP := $(LIBDIR)/libcompiler_builtins.rlib

LINKLIST := \
	$(OBJECTS) \
	$(LIBKERNEL)

OUTPUT := kernel.bin
ISOFILE := kernel.iso

.PHONY: all iso clean run

all: $(OUTPUT)

$(OUTPUT): $(LINKLIST)
	$(LD) -T $(ARCHDIR)/linker.ld $^ -o $@

%.o: %.s
	$(AS) -c $< -o $@

$(LIBKERNEL): $(SOURCES) $(LIBCORE) $(LIBCOMP)
	$(RUST) $(RUSTFLAGS) --edition 2018 --out-dir $(LIBDIR) \
	--crate-name kernel --crate-type staticlib $< \
	--extern core=$(LIBCORE) --extern compiler_builtins=$(LIBCOMP)

$(LIBCORE):
	$(RUST) -O $(RUSTFLAGS) --edition 2018 --out-dir $(LIBDIR) \
	--crate-name core --crate-type lib $(LIBDIR)/libcore/lib.rs

# Note: --edition 2018 causes weird build failures for this library.
$(LIBCOMP): $(LIBCORE)
	cd $(LIBDIR)/compiler-builtins && \
	cargo rustc --release -- $(RUSTFLAGS) --extern core=$(LIBCORE) && \
	mv target/release/libcompiler_builtins.rlib ..

setup-libs:
	cp -R $$(rustc --print sysroot)/lib/rustlib/src/rust/src $(LIBDIR)
	cd $(LIBDIR) && git clone "https://github.com/rust-lang/compiler-builtins"

iso:
	mkdir -p $(ISODIR)/boot/grub
	cp grub.cfg $(ISODIR)/boot/grub/
	cp $(OUTPUT) $(ISODIR)/boot
	grub-mkrescue -o $(ISOFILE) $(ISODIR)

clean:
	rm -rf $(ISOFILE) $(ISODIR) $(LINKLIST) $(OUTPUT)

clean-all: clean
	rm -rf $(LIBDIR)

run:
	qemu-system-i386 -m 32 -cdrom $(ISOFILE)
