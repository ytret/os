# ytret's OS - hobby operating system
# Copyright (C) 2020, 2021  Yuri Tretyakov (ytretyakov18@gmail.com)
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

AS := i686-elf-as
LD := i686-elf-ld
RUST := rustc
RUSTFMT := rustfmt
RUSTDOC := rustdoc
SHELL := /bin/bash

BUILDDIR ?= $(PWD)/build
LIBDIR ?= $(PWD)/lib
ISODIR ?= $(PWD)/isodir
USERDIR ?= $(PWD)/userland

RUSTFLAGS := --target $(ARCHDIR)/target.json -L $(LIBDIR) \
             -C incremental=$(BUILDDIR)
RUSTFMTFLAGS := --check --edition 2018 \
	--config max_width=80,reorder_modules=false

# kernel/main.rs must be first (see the $(LIBKERNEL) rule)
SOURCES := \
	kernel/main.rs \
	kernel/bitflags.rs \
	kernel/kernel_static.rs \
	kernel/memory_region.rs \
	kernel/port.rs \
	kernel/dev/vga.rs \
	kernel/dev/block_device.rs \
	kernel/dev/disk/mod.rs \
	kernel/dev/disk/ata.rs \
	kernel/dev/char_device.rs \
	kernel/dev/console.rs \
	kernel/multiboot.rs \
	kernel/heap.rs \
	kernel/process.rs \
	kernel/thread.rs \
	kernel/scheduler.rs \
	kernel/syscall.rs \
	kernel/fs/mod.rs \
	kernel/fs/devfs.rs \
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
SYSROOT := sysroot

USERPROGS ?= syscalls hello-world user-input

.DEFAULT_GOAL := kernel
.PHONY: all kernel userland \
	get-libs \
        iso sysroot hd sync run \
	clean-all clean-libdir clean-kernel clean-userland \
	check-fmt doc

all: kernel userland

get-libs:
	mkdir $(LIBDIR)
	cp -R $(RUSTLIBS)/core $(RUSTLIBS)/stdarch $(RUSTLIBS)/alloc $(LIBDIR)/
	cd $(LIBDIR) && git clone "https://github.com/rust-lang/compiler-builtins"

kernel: $(OUTPUT)

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

userland:
	userprogs=($(USERPROGS));					\
	for userprog in $${userprogs[@]}; do				\
		make -C $(USERDIR)/$$userprog all install || exit 1;	\
	done

iso: $(ISOFILE)

$(ISOFILE): $(OUTPUT) grub.cfg
	mkdir -p $(ISODIR)/boot/grub
	cp grub.cfg $(ISODIR)/boot/grub/
	cp $(OUTPUT) $(ISODIR)/boot
	grub-mkrescue -o $(ISOFILE) $(ISODIR)

sysroot:
	test ! -d $(SYSROOT)
	mkdir -p $(SYSROOT)/dev $(SYSROOT)/bin
	mkdir -p $(SYSROOT)/usr/local/lib $(SYSROOT)/usr/local/include
	ln -s local/lib $(SYSROOT)/usr/lib
	ln -s local/include $(SYSROOT)/usr/include

hd:
	bximage -q -func=create -hd=2048M -imgmode=flat $(HDIMG)
	mkfs.ext2 $(HDIMG) -d $(SYSROOT)

# Sync the contents of the hard disk with the $(SYSROOT) directory.
sync:
	mkdir mnt
	sudo mount $(HDIMG) mnt
	sudo rsync -avu --delete $(SYSROOT)/ mnt/
	sudo umount mnt
	rmdir mnt

run:
	qemu-system-i386 -m 32                                                         \
	                 -drive if=ide,index=0,media=cdrom,file=$(ISOFILE)             \
	                 -drive if=ide,index=1,media=disk,file=hd.img,format=raw       \
	                 -serial stdio

clean-all: clean-libdir clean-kernel clean-userland

clean-libdir:
	rm -rf $(LIBDIR)

clean-kernel:
	rm -rf $(ISOFILE) $(ISODIR) $(LINKLIST) $(OUTPUT)

clean-userland:
	userprogs=($(USERPROGS));				\
	for userprog in $${userprogs[@]}; do			\
		make -C $(USERDIR)/$$userprog clean || exit 1;	\
	done

check-fmt: $(SOURCES)
	$(RUSTFMT) $(RUSTFMTFLAGS) $<

doc: $(SOURCES)
	$(RUSTDOC) $(RUSTFLAGS) --edition 2018 --crate-name kernel \
	--crate-type staticlib $<
