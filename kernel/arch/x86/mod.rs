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
mod paging;
mod pic;
mod pmm_stack;
pub mod port_io;
mod stack_trace;

use crate::KernelInfo;

pub struct ArchInitInfo {
    pub kernel_start: u64,
    pub kernel_end: u64,
}

impl ArchInitInfo {
    pub fn new() -> Self {
        ArchInitInfo {
            kernel_start: 0,
            kernel_end: 0,
        }
    }
}

extern "C" {
    // see the linker.ld script
    static kernel_start: u32;
    static kernel_end: u32;
}

pub fn init(kernel_info: &mut KernelInfo) {
    let mut aif = ArchInitInfo::new();

    let kernel_start_addr = unsafe { &kernel_start as *const _ as u64 };
    let kernel_end_addr = unsafe { &kernel_end as *const _ as u64 };
    print!("kernel_start = 0x{:08X}; ", kernel_start_addr);
    println!("kernel_end = 0x{:08X}", kernel_end_addr);
    aif.kernel_start = kernel_start_addr;
    aif.kernel_end = kernel_end_addr;

    pic::init();
    interrupts::init();
    paging::init(kernel_end_addr as u32 - kernel_start_addr as u32);
    pmm_stack::init(kernel_info);

    kernel_info.arch_init_info = aif;

    unsafe {
        let val: u32 = 12;
        let val_ptr = &val as *const u32 as u32;
        let val_page = val_ptr & !0xFFF;
        let val_offset = val_ptr & 0xFFF;
        println!("value is {:2} at 0x{:08X}", val, val_ptr);

        // A page table already exists.
        let virt1_page: u32 = 0x7FF000;
        paging::map_page(virt1_page, val_page);
        let virt1_ptr = (virt1_page + val_offset) as *const u32;
        let virt1: u32 = *virt1_ptr;
        println!("virt1 is {:2} at 0x{:08X}", virt1, virt1_ptr as u32);

        // A new page table is allocated.
        let virt2_page: u32 = 0xABCDE000;
        paging::map_page(virt2_page, val_page);
        let virt2_ptr = (virt2_page + val_offset) as *const u32;
        let virt2: u32 = *virt2_ptr;
        println!("virt2 is {:2} at 0x{:08X}", virt2, virt2_ptr as u32);

        loop {}
    }
}

pub fn panic() {
    let trace = stack_trace::StackTrace::walk_and_get();
    for (i, addr) in trace.iter().enumerate() {
        println!(" stack item #{}: 0x{:08X}", trace.length - i, addr);
    }
}
