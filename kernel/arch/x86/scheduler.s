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
    // If we don't push %ebp here, StackTrace::walk_and_get() will go mad.
    pushl %ebp
    movl %esp, %ebp

    movl 8(%ebp), %eax          // eax = usermode code segment
    movl 12(%ebp), %ebx          // ebx = usermode data segment
    movl 16(%ebp), %ecx         // ecx = the address to jump to

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
    pushl %ebx                  // ss = data segment selector
    pushl %edx                  // esp
    pushf
    pushl %eax                  // cs = code segment selector
    pushl %ecx                  // eip

    iret
.size jump_into_usermode, . - jump_into_usermode

/*
 * Passes execution from the current task to the specified one.  Updates the
 * ESP0 field in the specified Task State Segment.
 * Arguments: 1) from: *mut Process
 *            2) to: *const Process
 *            3) tss: *mut TaskStateSegment
 * This function returns when the scheduler decides to run the caller's task.
 * It returns as if it wasn't ever called.
 * NOTE: the caller must disable interrupts before calling this function and
 * enable them after it returns.
 */
.global switch_tasks
.type switch_tasks, @function
switch_tasks:
    pushl %ebp
    movl %esp, %ebp

    pushl %eax
    pushl %ebx
    pushl %ecx
    pushl %esi
    pushl %edi

    movl 8(%ebp), %esi          // esi = from: *mut Process
    movl 12(%ebp), %edi         // edi = to: *const Process
    movl 16(%ebp), %eax         // eax = tss: *mut TaskStateSegment

    // Save %esp of the current task in its Process struct.
    movl %esp, 8(%esi)

    // Load the next task's Process struct.
    movl 0*4(%edi), %ebx        // ebx = cr3
    movl 1*4(%edi), %ecx        // kernel stack top
    movl 2*4(%edi), %esp        // kernel stack ptr

    // Update the ESP0 field in the TSS.
    movl %ecx, 4(%eax)

    // Change the virtual address space if needed.
    movl %cr3, %eax
    cmpl %ebx, %eax
    je 1f
    movl %ebx, %cr3

1:  popl %edi
    popl %esi
    popl %ecx
    popl %ebx
    popl %eax
    popl %ebp
    ret
.size switch_tasks, . - switch_tasks
