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

.global usermode_part
.type usermode_part, @function
usermode_part:
    // Set up a stack.
    movl $.stack_top, %esp
    xorl %ebp, %ebp

    // Open /dev/chr0.
    movl $0, %eax
    movl $.pathname, %ebx
    movl $9, %ecx
    int $0x88

    // Save the file descriptor in %ebx.
    movl %eax, %ebx

    // Any errors?
    cmpl $0, %ebx
    jne 2f

    // Read one ASCII character from the console.
1:  movl $2, %eax
    movl $.buffer, %ecx
    movl $1, %edx
    int $0x88
    cmpl $0, %eax
    jl 2f

    // Write that character to the console.
    movl $1, %eax
    movl $.buffer, %ecx
    movl $1, %edx
    int $0x88

    // Write a newline.
    // movl $1, %eax
    // movl $.newline, %ecx
    // int $0x88

    jmp 1b

2:  jmp 2b
.size usermode_part, . - usermode_part

.section .data
.pathname:  .ascii "/dev/chr0"
.buffer:    .skip 1, 0
.newline:   .ascii "\n"

.section .bss
.stack_bottom:
.skip 131072
.stack_top:
