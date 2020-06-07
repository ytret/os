.set MAGIC,      0xE85250D6
.set ARCH,       0  // i386
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
.skip 16384
stack_top:

.section .text
.global _entry
.type _entry, @function
_entry:
    cli
    movl $stack_top, %esp

    pushl %eax
    pushl %ebx
    call load_gdt
    popl %ebx
    popl %eax

    pushl %ebp
    movl %esp, %ebp
    pushl %ebx
    pushl %eax
    call main
    addl $8, %esp
    popl %ebp

    addl $4, %esp
    popl %ebp

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

.global memset
.type memset, @function
memset:
    pushl %ebp
    movl %esp, %ebp
    pushl %edi

    movl 8(%ebp), %edi
    movl 12(%ebp), %eax
    movl 16(%ebp), %ecx
    rep stosb

    popl %edi
    popl %ebp
    ret
.size memset, . - memset

.global memcpy
.type memcpy, @function
memcpy:
    pushl %ebp
    movl %esp, %ebp
    pushl %edi
    pushl %esi

    movl 8(%ebp), %edi
    movl 12(%ebp), %esi
    movl 16(%ebp), %ecx
    rep movsb

    popl %esi
    popl %edi
    popl %ebp
    ret
.size memcpy, . - memcpy

.global memcmp
.type memcmp, @function
memcmp:
    pushl %ebp
    pushl %edi
    pushl %esi

    movl 8(%ebp), %esi
    movl 12(%ebp), %edi
    movl 16(%ebp), %ecx
    repe cmpsb

    movl $0, %eax
    jz 1f

    movl $1, %eax
    ja 1f
    negl %eax

1:  popl %esi
    popl %edi
    popl %ebp
    ret
.size memcmp, . - memcmp
