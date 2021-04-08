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
    movl $0, %ebp

    call open_console

    // Any errors?
    cmpl $0, %eax
    jl 1f

    // Save the file descriptor.
    movl %eax, (console_fd)

0:  // Print the prompt.
    PRINT $entry_hello (entry_hello_len)
    PRINT $entry_list (entry_list_len)
    PRINT $entry_prompt (entry_prompt_len)

    // Read the answer.
    READ $entry_buf $1
    cmpl $0, %eax
    jl 1f

    cmpb $0x31, (entry_buf)     // 1
    je 2f
    cmpb $0x32, (entry_buf)     // 2
    je 3f
    cmpb $0x33, (entry_buf)     // 3
    je 4f
    cmpb $0x34, (entry_buf)     // 4
    je 5f

    jmp 0b

2:  call test_console
    jmp 0b

3:  call test_mem_map
    jmp 0b

4:  call test_exit
    jmp 0b

5:  call test_read_many
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

.type test_exit, @function
test_exit:
    movl $10, %eax
    movl $-1, %ebx
    int $0x88
    ud2
.size test_exit, . - test_exit

.type test_read_many, @function
test_read_many:
    pushl %ebp
    movl %esp, %ebp

    movl $2, %eax
    movl (console_fd), %ebx
    movl $test_read_many_buffer, %ecx
    movl $2, %edx
    int $0x88

    movl %eax, %edx
    movl $1, %eax
    int $0x88

    movl $1, %eax
    movl $test_console_newline_char, %ecx
    movl $1, %edx
    int $0x88

    popl %ebp
    ret
.size test_read_many, . - test_read_many

.section .data

entry_hello:                .ascii "Choose a test to run:\n"
entry_hello_len:            .long 22
entry_list:                 .ascii "1. console\n2. mem_map\n3. exit\n4. read_many\n"
entry_list_len:             .long 43
entry_prompt:               .ascii "> "
entry_prompt_len:           .long 2
entry_buf:                  .skip 1

console_fd:                 .skip 4
console_pathname:           .ascii "/dev/chr0"

test_console_hello:         .ascii "Hello from test-console. Press ] to exit.\n"
test_console_hello_len:     .long 42
test_console_buffer:        .skip 1
test_console_newline_char:  .ascii "\n"
test_console_exit_char:     .ascii "]"

test_read_many_buffer:      .skip 128
test_read_many_buffer_len:  .long 128
