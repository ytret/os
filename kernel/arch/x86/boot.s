// ytret's OS - hobby operating system
// Copyright (C) 2020  Yuri Tretyakov (ytretyakov18@gmail.com)
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with this program.  If not, see <https://www.gnu.org/licenses/>.

.set MAGIC,      0xE85250D6
.set ARCH,       0 // i386
.set HEADER_LEN, header_end - header_start
.set CHECKSUM,   -(MAGIC + ARCH + HEADER_LEN)

.section .multiboot
.align 8
header_start:
.long MAGIC
.long ARCH
.long HEADER_LEN
.long CHECKSUM

// Tag: Multiboot2 information request
.align 8
.word 1         // type
.word 0         // flags
.long 12        // size
.long 6         // memory map

// End of tags
.align 8
.word 0
.word 0
.long 8
header_end:

.section .bss
stack_bottom:
.skip 131072
stack_top:

.section .text
.global _entry
.type _entry, @function
_entry:
    cli
    movl $stack_top, %esp

    pushl %eax
    call load_gdt
    popl %eax

    pushl %ebx
    pushl %eax
    xorl %ebp, %ebp
    call main

    // Hang if main() returns.
    jmp halt
.size _entry, . - _entry

.global halt
.type halt, @function
halt:
    cli
1:  hlt
    jmp 1b
.size halt, . - halt

.local load_gdt
.type load_gdt, @function
load_gdt:
    lgdt gdt
    ljmp $0x08, $1f
1:  movw $0x10, %ax
    movw %ax, %ds
    movw %ax, %es
    movw %ax, %fs
    movw %ax, %gs
    movw %ax, %ss
    ret
.size load_gdt, . - load_gdt

gdt:
// Null segment (GDT descriptor)
.word end_gdt - gdt // size
.long gdt           // offset
.word 0x0000

// Code segment
.word 0xFFFF        // limit 0:15
.word 0x0000        // base 0:15
.byte 0x00          // base 16:23
.byte 0x9A          // access byte
.byte 0xCF          // (flags < 4) | limit 16:19
.byte 0x00          // base 24:31

// Data segment
.word 0xFFFF        // limit 0:15
.word 0x0000        // base 0:15
.byte 0x00          // base 16:23
.byte 0x92          // access byte
.byte 0xCF          // (flags < 4) | limit 16:19
.byte 0x00          // base 24:31
end_gdt:
