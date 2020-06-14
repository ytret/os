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
