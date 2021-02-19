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

use crate::arch::gdt;
use crate::arch::vas::VirtAddrSpace;
use crate::scheduler::SCHEDULER;

#[derive(Clone, Copy)]
#[repr(C, packed)]
pub struct Process {
    // NOTE: when changing the order of these fields, also edit switch_tasks()
    // in scheduler.s.
    pub cr3: u32,
    pub esp0: u32,
    pub esp: u32,
}

impl Process {
    pub fn new() -> Self {
        let kernel_stack_bottom =
            unsafe { alloc(Layout::from_size_align(1024, 4096).unwrap()) }
                .wrapping_offset(1024) as *mut u32;

        // Make an initial stack frame that will be popped on a task switch (see
        // scheduler.s).
        let kernel_stack_top = kernel_stack_bottom.wrapping_sub(8);
        unsafe {
            *kernel_stack_top.wrapping_add(0) = 0; // edi
            *kernel_stack_top.wrapping_add(1) = 0; // esi
            *kernel_stack_top.wrapping_add(2) = 0; // ecx
            *kernel_stack_top.wrapping_add(3) = 0; // ebx
            *kernel_stack_top.wrapping_add(4) = 0; // eax
            *kernel_stack_top.wrapping_add(5) = 0x00000000;
            // ebp = 0x00000000 is a magic value making the stack tracer to
            // stop.  It is used here the same way as in boot.s.
            *kernel_stack_top.wrapping_add(6) =
                default_entry_point as *const () as u32; // eip
            *kernel_stack_top.wrapping_add(7) = 0x00000000;
            // Here 0x00000000 is just some value for the stack tracer to print
            // out as EIP instead of the heap garbage after the the stack.  Also
            // it may serve as a return address for default_entry_point().
        }

        Process {
            cr3: crate::arch::vas::KERNEL_VAS.lock().pgdir_phys,
            esp0: kernel_stack_bottom as u32,
            esp: kernel_stack_top as u32,
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
        SCHEDULER.stop_scheduling();
        println!("[PROC] Creating a new VAS for the process.");
        let vas = VirtAddrSpace::kvas_copy_on_heap();
        println!("[PROC] Loading the VAS.");
        vas.load();
        SCHEDULER.keep_scheduling();
    }

    // unsafe {
    //     println!("[PROC] Entering usermode.");
    //     scheduler::jump_into_usermode(
    //         gdt::USERMODE_CODE_SEG,
    //         gdt::USERMODE_DATA_SEG,
    //         usermode_part,
    //     );
    // }

    println!("[PROC] End of default process entry.");
    loop {}
}
