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

AS := i686-elf-as
LD := i686-elf-ld

OUTPUT := main
SYSROOT := $(shell pwd)/../../sysroot

.PHONY: all install clean

all: $(OUTPUT)

$(OUTPUT): main.o
	$(LD) -T linker.ld $^ -o $@

%.o: %.s
	$(AS) -c $< -o $@

install:
	cp main $(SYSROOT)/test

clean:
	rm -rf $(OUTPUT) main.o $(SYSROOT)/test
