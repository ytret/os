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

use crate::memory_region::Region;
use crate::KernelInfo;

pub struct ArchInitInfo {
    // FIXME use usize
    pub kernel_start: u64,
    pub kernel_end: u64,
    pub heap_region: Region<usize>,
}

impl ArchInitInfo {
    pub fn new() -> Self {
        ArchInitInfo {
            kernel_start: 0,
            kernel_end: 0,
            heap_region: Region { start: 0, end: 0 },
        }
    }
}

extern "C" {
    // see the linker.ld script
    static kernel_start: u32;
    static kernel_end: u32;

    // see boot.s
    static stack_bottom: u32;
    static stack_top: u32;
}

pub fn init(kernel_info: &mut KernelInfo) {
    let mut aif = ArchInitInfo::new();

    let kernel_start_addr = unsafe { &kernel_start as *const _ as u64 };
    let kernel_end_addr = unsafe { &kernel_end as *const _ as u64 };
    print!("kernel_start = 0x{:08X}; ", kernel_start_addr);
    println!("kernel_end = 0x{:08X}", kernel_end_addr);
    aif.kernel_start = kernel_start_addr;
    aif.kernel_end = kernel_end_addr;

    unsafe {
        println!(
            "stack_bottom = 0x{:08X}, stack_top = 0x{:08X}",
            &stack_bottom as *const _ as u32, &stack_top as *const _ as u32,
        );
    }

    pic::init();
    interrupts::init();
    paging::init(kernel_end_addr as u32 - kernel_start_addr as u32);
    pmm_stack::init(kernel_info);

    let heap_region = Region {
        start: kernel_end_addr as usize,
        end: kernel_end_addr as usize + crate::allocator::KERNEL_HEAP_SIZE,
    };
    paging::allocate_region(heap_region.start as u32, heap_region.end as u32);
    aif.heap_region = heap_region;

    kernel_info.arch_init_info = aif;
}

pub fn panic() {
    let trace = stack_trace::StackTrace::walk_and_get();
    for (i, addr) in trace.iter().enumerate() {
        println!(" stack item #{}: 0x{:08X}", trace.length - i, addr);
    }
}
