# ytret's OS - hobby operating system
# Copyright (C) 2020  Yuri Tretyakov (ytretyakov18@gmail.com)
#
# This program is free software: you can redistribute it and/or modify
# it under the terms of the GNU General Public License as published by
# the Free Software Foundation, either version 3 of the License, or
# (at your option) any later version.
#
# This program is distributed in the hope that it will be useful,
# but WITHOUT ANY WARRANTY; without even the implied warranty of
# MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
# GNU General Public License for more details.
#
# You should have received a copy of the GNU General Public License
# along with this program.  If not, see <https:#www.gnu.org/licenses/>.

.PHONY: all clean install

ARCH ?= x86
ARCHDIR := $(PWD)/kernel/arch/$(ARCH)
include $(ARCHDIR)/Makefile.inc

LIBDIR ?= $(PWD)/lib
ISODIR ?= $(PWD)/isodir

AS := i686-elf-as
LD := i686-elf-ld
RUST := rustc
CARGO := cargo # used only for $(LIBCOMP)
RUSTFMT := rustfmt

RUSTFLAGS := --target $(ARCHDIR)/target.json
RUSTFMTFLAGS := --check --edition 2018 --config max_width=80

# kernel/main.rs must be first (e.g. see the $(LIBKERNEL) rule)
SOURCES := \
	kernel/main.rs \
	kernel/kernel_static.rs \
	kernel/vga.rs \
	$(ARCH_SOURCES)

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

.PHONY: all iso clean run check-fmt

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
	$(CARGO) rustc --release -- $(RUSTFLAGS) --extern core=$(LIBCORE) && \
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

check-fmt: $(SOURCES)
	$(RUSTFMT) $(RUSTFMTFLAGS) $<
