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

.section .text

/*
 * Walks backwards through the call stack and builds an array of return
 * addresses.
 * Arguments: 1) array of 32-bit addresses (pointer to the first element)
 *            2) max. number of elements in the array
 * Return value: length of the constructed array.
 */
.global walk_stack
.type walk_stack, @function
walk_stack:
    // Create stack frame and save caller's %edi and %ebx.
    pushl %ebp
    movl %esp, %ebp
    subl $8, %esp
    movl %edi, -4(%ebp)
    movl %ebx, -8(%ebp)

    // Set up local registers.
    xorl %eax, %eax         // eax = return value
    movl 8(%esp), %ebx      // old ebp => ebx
    movl 16(%esp), %edi     // destination array pointer => edi
    movl 20(%esp), %ecx     // max. array size => ecx

1:  // Walk backwards through the %ebp-linked list, storing return addresses in
    // the %edi-array.
    testl %ebx, %ebx
    jz 2f                   // ebp is set to 0 in boot.s and Process::new()
    movl 4(%ebx), %edx      // previous stack frame's eip => edx
    movl 0(%ebx), %ebx      // previous stack frame's ebp => ebx
    movl %edx, (%edi)       // push eip to the array
    addl $4, %edi
    incl %eax
    loop 1b

2:  // Restore the caller's %edi and %ebx and leave the stack frame.  %eax holds
    // the return value.
    movl -4(%ebp), %edi
    movl -8(%ebp), %ebx
    leave
    ret
.size walk_stack, . - walk_stack
