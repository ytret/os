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

/*
 * Does a far return with usermode segments to the specified function.
 * Arguments: 1) usermode code segment (not a selector)
 *            2) usermode data segment (not a selector)
 *            3) the address to jump to
 * This function does not return.
 */
.global jump_into_usermode
.type jump_into_usermode, @function
jump_into_usermode:
    xchg %bx, %bx
    pushl %ebp
    movl %esp, %ebp

    movl 8(%ebp), %eax      // eax = usermode code segment
    movl 12(%ebp), %ebx     // ebx = usermode data segment
    movl 16(%ebp), %ecx     // ecx = the address to jump to

    // Set RPL to 3, that is to usermode.
    orl $3, %eax
    orl $3, %ebx

    // Set data segments (%ss is set by iret).
    movw %bx, %ds
    movw %bx, %es
    movw %bx, %fs
    movw %bx, %gs

    // Make up the iret stack frame.
    movl %esp, %edx
    pushl %ebx              // ss = data segment selector
    pushl %edx              // esp
    pushf
    pushl %eax              // cs = code segment selector
    pushl %ecx              // eip

    iret
.size jump_into_usermode, . - jump_into_usermode
