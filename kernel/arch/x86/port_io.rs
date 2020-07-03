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

pub unsafe fn outb(port: u16, data: u8) {
    asm!(
        "outb %al, %dx",
        in("eax") data as u32,
        in("dx") port,
        options(att_syntax),
    );
}

pub unsafe fn outl(port: u16, data: u32) {
    asm!(
        "outl %eax, %dx",
        in("eax") data,
        in("dx") port,
        options(att_syntax),
    );
}

pub unsafe fn inb(port: u16) -> u8 {
    let mut data: u32;
    asm!(
        "xorl %eax, %eax
         inb %dx, %al",
        out("eax") data,
        in("dx") port,
        options(att_syntax),
    );
    data as u8
}

pub unsafe fn inl(port: u16) -> u32 {
    let mut data: u32;
    asm!(
        "inl %dx, %eax",
        out("eax") data,
        in("dx") port,
        options(att_syntax),
    );
    data
}
