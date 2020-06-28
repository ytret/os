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

ARCH_SOURCES := \
	$(ARCHDIR)/mod.rs \
	$(ARCHDIR)/port_io.rs \
	$(ARCHDIR)/gdt.rs \
	$(ARCHDIR)/pic.rs \
	$(ARCHDIR)/pit.rs \
	$(ARCHDIR)/interrupts.rs \
	$(ARCHDIR)/paging.rs \
	$(ARCHDIR)/pmm_stack.rs \
	$(ARCHDIR)/process.rs \
	$(ARCHDIR)/scheduler.rs

ARCH_OBJECTS := \
	$(ARCHDIR)/boot.o \
	$(ARCHDIR)/stack_trace.o \
	$(ARCHDIR)/gdt.o \
	$(ARCHDIR)/interrupts.o \
	$(ARCHDIR)/pit.o \
	$(ARCHDIR)/scheduler.o
