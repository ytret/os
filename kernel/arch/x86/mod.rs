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

pub mod interrupts;

use crate::ArchInitInfo;

extern "C" {
    // see the linker.ld script
    static text_start: u32;
    static kernel_end: u32;
}

pub fn init() -> ArchInitInfo {
    //interrupts::init();

    let text_start_addr = unsafe { &text_start as *const _ as u32 };
    let kernel_end_addr = unsafe { &kernel_end as *const _ as u32 };
    print!("text_start = 0x{:08X}; ", text_start_addr);
    println!("kernel_end = 0x{:08X}", kernel_end_addr);

    ArchInitInfo {
        kernel_size: (kernel_end_addr - text_start_addr) / 1024,
    }
}
