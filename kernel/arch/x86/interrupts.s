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

.macro CALL_HANDLER int_num err_code
	pushl \err_code                 // error code
    pushl \int_num                  // interrupt number
    cld
    call dummy_interrupt_handler
    addl $8, %esp
.endm

.macro DEFINE_DUMMY_ISR num
.global dummy_isr_\num
.type dummy_isr_\num, @function
dummy_isr_\num:
	cli
    pusha
    CALL_HANDLER $\num $0
    popa
    iret
.size dummy_isr_\num, . - dummy_isr_\num
.endm

.macro DEFINE_DUMMY_ISR_EC num
.global dummy_isr_\num
.type dummy_isr_\num, @function
dummy_isr_\num:
	cli
    pusha
    movl 36(%esp), %eax     // error code
    CALL_HANDLER $\num %eax
    popa
    addl $4, %esp           // the error code must be consumed
    iret
.size dummy_isr_\num, . - dummy_isr_\num
.endm

DEFINE_DUMMY_ISR 0          // divide error
DEFINE_DUMMY_ISR 1          // debug
DEFINE_DUMMY_ISR 2          // non-maskable interrupt
DEFINE_DUMMY_ISR 3          // breakpoint
DEFINE_DUMMY_ISR 4          // overflow
DEFINE_DUMMY_ISR 5          // bound range exceeded
DEFINE_DUMMY_ISR 6          // invalid opcode
DEFINE_DUMMY_ISR 7          // device not available
DEFINE_DUMMY_ISR_EC 8       // double fault
DEFINE_DUMMY_ISR 9          // coprocessor segment overrun (old)
DEFINE_DUMMY_ISR_EC 10      // invalid TSS
DEFINE_DUMMY_ISR_EC 11      // segment not present
DEFINE_DUMMY_ISR_EC 12      // stack fault
DEFINE_DUMMY_ISR_EC 13      // general protection
DEFINE_DUMMY_ISR_EC 14      // page fault
DEFINE_DUMMY_ISR 15         // reserved
DEFINE_DUMMY_ISR 16         // x87 FPU floating-point error
DEFINE_DUMMY_ISR_EC 17      // alignment check
DEFINE_DUMMY_ISR 18         // machine check
DEFINE_DUMMY_ISR 19         // SIMD floating-point
DEFINE_DUMMY_ISR 20         // virtualization
DEFINE_DUMMY_ISR_EC 21      // control protection
DEFINE_DUMMY_ISR 22         // 22-31 reserved
DEFINE_DUMMY_ISR 23
DEFINE_DUMMY_ISR 24
DEFINE_DUMMY_ISR 25
DEFINE_DUMMY_ISR 26
DEFINE_DUMMY_ISR 27
DEFINE_DUMMY_ISR 28
DEFINE_DUMMY_ISR 29
DEFINE_DUMMY_ISR 30
DEFINE_DUMMY_ISR 31

DEFINE_DUMMY_ISR 256        // generic ISR
DEFINE_DUMMY_ISR_EC 257     // generic ISR with an error code
