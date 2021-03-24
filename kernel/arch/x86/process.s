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

.global usermode_part
.type usermode_part, @function
usermode_part:
    pushl %ebp
    movl %esp, %ebp

    movl $0, %eax
    movl $.pathname, %ebx
    movl $9, %ecx
    int $0x88

    pushl %eax
    popl %ebx

    movl $1, %eax
    movl $.greeting, %ecx
    movl $14, %edx
    int $0x88

1:  jmp 1b
.size usermode_part, . - usermode_part

.pathname:  .ascii "/dev/chr0"
.greeting:  .ascii "Hello, World!\n"