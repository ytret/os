// ytret's OS - hobby operating system
// Copyright (C) 2020, 2021  Yuri Tretyakov (ytretyakov18@gmail.com)
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

.macro PRINT buf len
    movl $1, %eax
    movl (console_fd), %ebx
    movl \buf, %ecx
    movl \len, %edx
    int $0x88
.endm

.macro READ buf len
    movl $2, %eax
    movl (console_fd), %ebx
    movl \buf, %ecx
    movl \len, %edx
    int $0x88
.endm

.global _entry
.type _entry, @function
_entry:
    movl %esp, %ebp

    call open_console

    // Any errors?
    cmpl $0, %eax
    jl 1f

    // Save the file descriptor.
    movl %eax, (console_fd)

0:  PRINT $entry_hello (entry_hello_len)
    PRINT $entry_list (entry_list_len)

    READ $entry_buf $1
    cmpl $0, %eax
    jl 1f

    cmpb $0x31, (entry_buf)     // 1
    je 2f
    cmpb $0x32, (entry_buf)     // 2
    je 3f

    jmp 1f

2:  call test_console
    jmp 0b

3:  call test_mem_map
    jmp 0b

1:  ud2
.size _entry, . - _entry

.type open_console, @function
open_console:
    pushl %ebp
    movl %esp, %ebp

    movl $0, %eax
    movl $console_pathname, %ebx
    movl $9, %ecx
    int $0x88

    popl %ebp
    ret
.size open_console, . - open_console

.type test_console, @function
test_console:
    pushl %ebp
    movl %esp, %ebp

    PRINT $test_console_hello (test_console_hello_len)

    // Read one ASCII character from the console.
1:  READ $test_console_buffer $1
    cmpl $0, %eax
    jl 2f

    // Write that character to the console.
    PRINT $test_console_buffer $1

    xchg %bx, %bx
    movb (test_console_exit_char), %al
    cmpb %al, (test_console_buffer)
    je 2f

    jmp 1b

2:  // Write a newline.
    PRINT $test_console_newline_char $1

    popl %ebp
    ret
.size test_console, . - test_console

.type test_mem_map, @function
test_mem_map:
    pushl $6
    pushl $5
    pushl $4
    pushl $3
    pushl $2
    pushl $1
    movl $5, %eax
    movl %esp, %ebx
    int $0x88
    addl $24, %esp
    ud2
.size test_mem_map, . - test_mem_map

.section .data

entry_hello:                .ascii "Choose a test to run:\n"
entry_hello_len:            .long 22
entry_list:                 .ascii "1. console\n2. mem_map\n"
entry_list_len:             .long 22
entry_buf:                  .skip 1

console_fd:                 .skip 4
console_pathname:           .ascii "/dev/chr0"

test_console_hello:         .ascii "Hello from test-console. Press ] to exit.\n"
test_console_hello_len:     .long 42
test_console_buffer:        .skip 1
test_console_newline_char:  .ascii "\n"
test_console_exit_char:     .ascii "]"
