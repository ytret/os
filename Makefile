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
# along with this program.  If not, see <https://www.gnu.org/licenses/>.

ARCH ?= x86
ARCHDIR := $(PWD)/kernel/arch/$(ARCH)
include $(ARCHDIR)/Makefile.inc

LIBDIR ?= $(PWD)/lib
ISODIR ?= $(PWD)/isodir

AS := i686-elf-as
LD := i686-elf-ld
RUST := rustc
RUSTFMT := rustfmt
RUSTDOC := rustdoc

RUSTFLAGS := --target $(ARCHDIR)/target.json -L $(LIBDIR)
RUSTFMTFLAGS := --check --edition 2018 \
	--config max_width=80,reorder_modules=false

# kernel/main.rs must be first (e.g. see the $(LIBKERNEL) rule)
SOURCES := \
	kernel/main.rs \
	kernel/bitflags.rs \
	kernel/kernel_static.rs \
	kernel/port.rs \
	kernel/vga.rs \
	kernel/mbi.rs \
	kernel/heap.rs \
	kernel/scheduler.rs \
	kernel/disk/mod.rs \
	kernel/disk/ata.rs \
	kernel/fs/mod.rs \
	kernel/fs/ext2.rs \
	kernel/elf.rs \
	$(ARCH_SOURCES)

OBJECTS := \
    $(ARCH_OBJECTS)

LIBKERNEL := $(LIBDIR)/libkernel.a
LIBCORE := $(LIBDIR)/libcore.rlib
LIBCOMP := $(LIBDIR)/libcompiler_builtins.rlib
LIBALLOC := $(LIBDIR)/liballoc.rlib
RUSTLIBS := $$(rustc --print sysroot)/lib/rustlib/src/rust/library

LINKLIST := \
	$(OBJECTS) \
	$(LIBKERNEL)

OUTPUT := kernel.bin
ISOFILE := kernel.iso
HDIMG := hd.img

.PHONY: all get-libs doc iso hd clean clean-all run check-fmt

all: $(OUTPUT)

$(OUTPUT): $(LINKLIST) $(ARCHDIR)/linker.ld
	$(LD) -T $(ARCHDIR)/linker.ld $(LINKLIST) -o $@

%.o: %.s
	$(AS) -c $< -o $@

$(LIBKERNEL): $(SOURCES) $(LIBCORE) $(LIBCOMP) $(LIBALLOC)
	$(RUST) $(RUSTFLAGS) --edition 2018 --out-dir $(LIBDIR) \
	--crate-name kernel --crate-type staticlib $<

$(LIBCORE):
	$(RUST) -O $(RUSTFLAGS) --edition 2018 --out-dir $(LIBDIR) \
	--crate-name core --crate-type rlib $(LIBDIR)/core/src/lib.rs

# Note: --edition 2018 causes build failures for this library.
$(LIBCOMP): $(LIBCORE)
	$(RUST) -O $(RUSTFLAGS) --out-dir $(LIBDIR) \
	--cfg feature=\"compiler-builtins\" --cfg feature=\"mem\" \
	--crate-name compiler_builtins --crate-type rlib \
	$(LIBDIR)/compiler-builtins/src/lib.rs

$(LIBALLOC): $(LIBCORE) $(LIBCOMP)
	$(RUST) -O $(RUSTFLAGS) --edition 2018 --out-dir $(LIBDIR) \
	--crate-name alloc --crate-type rlib $(LIBDIR)/alloc/src/lib.rs

doc: $(SOURCES)
	$(RUSTDOC) $(RUSTFLAGS) --edition 2018 --crate-name kernel \
	--crate-type staticlib $<

get-libs:
	mkdir -p $(LIBDIR)
	cp -R $(RUSTLIBS)/core $(RUSTLIBS)/stdarch $(RUSTLIBS)/alloc $(LIBDIR)/
	cd $(LIBDIR) && git clone "https://github.com/rust-lang/compiler-builtins"

iso: $(ISOFILE)

$(ISOFILE): $(OUTPUT) grub.cfg
	mkdir -p $(ISODIR)/boot/grub
	cp grub.cfg $(ISODIR)/boot/grub/
	cp $(OUTPUT) $(ISODIR)/boot
	grub-mkrescue -o $(ISOFILE) $(ISODIR)

hd:
	rm -rf hd.img
	bximage -q -mode=create -hd=19M -imgmode=flat $(HDIMG)

clean:
	rm -rf $(ISOFILE) $(ISODIR) $(LINKLIST) $(OUTPUT)

clean-all: clean
	rm -rf $(LIBDIR)

run:
	qemu-system-i386 -m 32 \
	                 -drive if=ide,index=0,media=cdrom,file=$(ISOFILE) \
	                 -drive if=ide,index=1,media=disk,file=hd.img,format=raw \
	                 -serial stdio

check-fmt: $(SOURCES)
	$(RUSTFMT) $(RUSTFMTFLAGS) $<
