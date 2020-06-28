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

.macro CALL_HANDLER int_num err_code frame_ptr
	pushl \frame_ptr
	pushl \err_code                 // error code
    pushl \int_num                  // interrupt number
    cld
    call dummy_exception_handler
    addl $12, %esp
.endm

.macro DUMMY_EXCEPTION_ISR num
.global dummy_isr_\num
.type dummy_isr_\num, @function
dummy_isr_\num:
	cli
    pusha
    movl %esp, %ebx
    addl $32, %ebx                  // interrupt stack frame pointer
    CALL_HANDLER $\num $0 %ebx
    popa
    iret
.size dummy_isr_\num, . - dummy_isr_\num
.endm

.macro DUMMY_EXCEPTION_ISR_EC num
.global dummy_isr_\num
.type dummy_isr_\num, @function
dummy_isr_\num:
	cli
    pusha
    movl %esp, %ebx
    addl $32, %ebx                  // interrupt stack frame pointer
    movl 32(%esp), %eax             // error code
    CALL_HANDLER $\num %eax %ebx
    popa
    addl $4, %esp                   // the error code must be consumed
    iret
.size dummy_isr_\num, . - dummy_isr_\num
.endm

DUMMY_EXCEPTION_ISR 0       // divide error
DUMMY_EXCEPTION_ISR 1       // debug
DUMMY_EXCEPTION_ISR 2       // non-maskable interrupt
DUMMY_EXCEPTION_ISR 3       // breakpoint
DUMMY_EXCEPTION_ISR 4       // overflow
DUMMY_EXCEPTION_ISR 5       // bound range exceeded
DUMMY_EXCEPTION_ISR 6       // invalid opcode
DUMMY_EXCEPTION_ISR 7       // device not available
DUMMY_EXCEPTION_ISR_EC 8    // double fault
DUMMY_EXCEPTION_ISR 9       // coprocessor segment overrun (old)
DUMMY_EXCEPTION_ISR_EC 10   // invalid TSS
DUMMY_EXCEPTION_ISR_EC 11   // segment not present
DUMMY_EXCEPTION_ISR_EC 12   // stack fault
DUMMY_EXCEPTION_ISR_EC 13   // general protection
DUMMY_EXCEPTION_ISR_EC 14   // page fault
DUMMY_EXCEPTION_ISR 15      // reserved
DUMMY_EXCEPTION_ISR 16      // x87 FPU floating-point error
DUMMY_EXCEPTION_ISR_EC 17   // alignment check
DUMMY_EXCEPTION_ISR 18      // machine check
DUMMY_EXCEPTION_ISR 19      // SIMD floating-point
DUMMY_EXCEPTION_ISR 20      // virtualization
DUMMY_EXCEPTION_ISR_EC 21   // control protection
DUMMY_EXCEPTION_ISR 22      // 22-31 reserved
DUMMY_EXCEPTION_ISR 23
DUMMY_EXCEPTION_ISR 24
DUMMY_EXCEPTION_ISR 25
DUMMY_EXCEPTION_ISR 26
DUMMY_EXCEPTION_ISR 27
DUMMY_EXCEPTION_ISR 28
DUMMY_EXCEPTION_ISR 29
DUMMY_EXCEPTION_ISR 30
DUMMY_EXCEPTION_ISR 31

.global common_isr
.type common_isr, @function
common_isr:
    cli
    pusha
    movl %esp, %eax
    addl $32, %eax
    cld
    pushl %eax
    call common_interrupt_handler
    addl $4, %esp
    popa
    iret
.size common_isr, . - common_isr

.global common_isr_ec
.type common_isr_ec, @function
common_isr_ec:
    cli
    pusha
    movl %esp, %eax
    addl $32, %eax
    cld
    pushl %eax
    call common_interrupt_handler
    addl $4, %esp
    popa
    addl $4, %esp
    iret
.size common_isr, . - common_isr
