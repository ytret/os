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

mod gdt;
pub mod interrupts;
pub mod vas;
pub mod pic;
mod pit;
pub mod pmm_stack;
pub mod port_io;
mod stack_trace;

pub mod process;
pub mod scheduler;

pub mod pci;

pub mod syscalls;

pub mod keyboard;

use core::ptr;

use crate::memory_region::Region;
use crate::KERNEL_INFO;

pub struct ArchInitInfo {
    pub kernel_region: Region<usize>,
    pub heap_region: Region<usize>,
}

impl ArchInitInfo {
    pub const fn new() -> Self {
        ArchInitInfo {
            kernel_region: Region { start: 0, end: 0 },
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

pub fn init() {
    let mut aif = ArchInitInfo::new();

    gdt::init();

    aif.kernel_region = Region {
        start: unsafe { &kernel_start as *const _ as usize },
        end: unsafe { &kernel_end as *const _ as usize },
    };
    println!("Kernel region: {:?}", aif.kernel_region);

    unsafe {
        println!(
            "stack_bottom = 0x{:08X}, stack_top = 0x{:08X}",
            &stack_bottom as *const _ as u32, &stack_top as *const _ as u32,
        );
    }

    pic::init();
    interrupts::init();
    pit::init();

    // Enable paging.
    unsafe {
        vas::KERNEL_VAS.lock().load();
        asm!("movl %cr0, %eax
              orl $0x80000001, %eax
              movl %eax, %cr0",
             out("eax") _,
             options(att_syntax));
    }

    pmm_stack::init();

    aif.heap_region = Region {
        start: (aif.kernel_region.end + 0x400_000 - 1) & !(0x400_000 - 1),
        end: ((aif.kernel_region.end + 0x400_000 - 1) & !(0x400_000 - 1))
            + crate::heap::KERNEL_HEAP_SIZE,
    };
    println!("Heap region: {:?}", aif.heap_region);

    // Map the heap.
    unsafe {
        let kvas = vas::KERNEL_VAS.lock();
        let heap_pgtbl_virt =
            &mut *vas::KERNEL_HEAP_PGTBL.lock() as *mut vas::Table;
        kvas.set_pde_addr(aif.heap_region.start >> 22, heap_pgtbl_virt);
        ptr::write_bytes(heap_pgtbl_virt as *mut u8, 0, 4096);
        kvas.allocate_pages_from_stack(
            aif.heap_region.start as u32,
            aif.heap_region.end as u32,
        );
    }

    unsafe {
        KERNEL_INFO.arch_init_info = aif;
    }
}

#[inline(always)]
pub fn panic() {
    unsafe {
        asm!("cli");
    }
    let trace = stack_trace::StackTrace::walk_and_get();
    println!(" stack trace:");
    for (i, addr) in trace.iter().enumerate() {
        print!(" #{:02}: 0x{:08X}    ", trace.length - i, addr);
    }
}
