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

/*
 * Passes execution from the current thread to the specified one.  Updates the
 * ESP0 field in the specified Task State Segment.
 * Arguments: 1) from: *mut ThreadControlBlock
 *            2) to: *const ThreadControlBlock
 *            3) tss: *mut TaskStateSegment
 * This function returns when the scheduler decides to run the caller's thread.
 * It returns as if it wasn't ever called (i.e. like a normal function).
 * NOTE: one must disable interrupts before calling this function and enable
 * them after it returns (this applies to both the current and the next thread's
 * code).
 */
.global switch_threads
.type switch_threads, @function
switch_threads:
    pushl %ebp
    movl %esp, %ebp

    pushl %eax
    pushl %ebx
    pushl %ecx
    pushl %esi
    pushl %edi

    movl 8(%ebp), %esi          // esi = from: *mut ThreadControlBlock
    movl 12(%ebp), %edi         // edi = to: *const ThreadControlBlock
    movl 16(%ebp), %eax         // eax = tss: *mut TaskStateSegment

    // Save %esp of the current thread in its ThreadControlBlock.
    movl %esp, 8(%esi)

    // Load the next thread's ThreadControlBlock.
    movl 0*4(%edi), %ebx        // ebx = cr3
    movl 1*4(%edi), %ecx        // ecx = kernel stack bottom
    movl 2*4(%edi), %esp        // esp = kernel stack top

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

    // Load the next thread's %eip from its stack.
    ret
.size switch_threads, . - switch_threads

/*
 * Does a far return with usermode segments to the specified function.
 * Arguments: 1) usermode code segment (not a selector)
 *            2) usermode data segment (not a selector)
 *            3) TLS segment (not a selector)
 *            4) the address to jump to
 *            5) usermode esp value
 * This function does not return.
 */
.global jump_into_usermode
.type jump_into_usermode, @function
jump_into_usermode:
    pushl %ebp
    movl %esp, %ebp

    movl 8(%ebp), %eax          // eax = usermode code segment
    movl 12(%ebp), %ebx         // ebx = usermode data segment
    movl 16(%ebp), %ecx         // ecx = TLS segment
    movl 20(%ebp), %edx         // edx = the address to jump to

    // Set RPL to 3, that is to usermode.
    orl $3, %eax
    orl $3, %ebx
    orl $3, %ecx

    // Set data segments (%ss is set by iret).
    movw %bx, %ds
    movw %bx, %es
    movw %bx, %fs
    movw %cx, %gs

    // Make up the iret stack frame.
    pushl %ebx                  // ss = data segment selector
    pushl 24(%ebp)              // esp
    pushf
    pushl %eax                  // cs = code segment selector
    pushl %edx                  // eip

    iret
1:  ud2
.size jump_into_usermode, . - jump_into_usermode
