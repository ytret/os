.section .text

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
