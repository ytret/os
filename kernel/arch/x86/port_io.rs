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

pub unsafe fn outb(port: u16, data: u8) {
    asm!(
        "outb %al, %dx",
        in("al") data,
        in("dx") port,
        options(att_syntax),
    );
}

pub unsafe fn outw(port: u16, data: u16) {
    asm!(
        "outw %ax, %dx",
        in("ax") data,
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
    let mut data: u8;
    asm!(
        "inb %dx, %al",
        out("al") data,
        in("dx") port,
        options(att_syntax),
    );
    data
}

pub unsafe fn inw(port: u16) -> u16 {
    let mut data: u16;
    asm!(
        "inw %dx, %ax",
        out("ax") data,
        in("dx") port,
        options(att_syntax),
    );
    data
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
