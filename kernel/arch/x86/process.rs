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

use alloc::alloc::{alloc, Layout};

use crate::arch::vas::VirtAddrSpace;

#[derive(Clone, Copy)]
#[repr(C, packed)]
pub struct Process {
    // NOTE: when changing the order of these fields, also edit switch_tasks()
    // in scheduler.s.
    pub cr3: u32,
    pub esp0: u32, // kernel stack bottom
    pub esp: u32,
}

impl Process {
    pub fn new() -> Self {
        let kernel_stack_bottom =
            unsafe { alloc(Layout::from_size_align(1024, 4096).unwrap()) }
                .wrapping_offset(1024) as *mut u32;
        let ordinary_stack_bottom =
            unsafe { alloc(Layout::from_size_align(1024, 4096).unwrap()) }
                .wrapping_offset(1024) as *mut u32;

        // Make an initial stack frame that will be popped on a task switch (see
        // scheduler.s).
        let ordinary_stack_top = ordinary_stack_bottom.wrapping_sub(7);
        unsafe {
            *ordinary_stack_top.wrapping_add(0) = 0; // edi
            *ordinary_stack_top.wrapping_add(1) = 0; // esi
            *ordinary_stack_top.wrapping_add(2) = 0; // ecx
            *ordinary_stack_top.wrapping_add(3) = 0; // ebx
            *ordinary_stack_top.wrapping_add(4) = 0; // eax
            *ordinary_stack_top.wrapping_add(5) = 0; // ebp
            *ordinary_stack_top.wrapping_add(6) =
                default_entry_point as *const () as u32; // eip
        }

        Process {
            cr3: crate::arch::vas::KERNEL_VAS.lock().pgdir_phys,
            esp0: kernel_stack_bottom as u32,
            esp: ordinary_stack_top as u32,
        }
    }
}

fn default_entry_point() -> ! {
    // This function must always be a result of ret from switch_tasks (see
    // scheduler.s) which requires that interrupts be enabled after it returns
    // so that task switching remains possible.
    unsafe {
        asm!("sti");
    }

    println!("[PROC] Default process entry. Starting initialization.");

    unsafe {
        println!("[PROC] Creating a new VAS for the process.");
        let vas = VirtAddrSpace::kvas_copy_on_heap();
        println!("[PROC] Loading the VAS.");
        vas.load();
    }

    println!("[PROC] End of default process entry.");

    loop {}
}
