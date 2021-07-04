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

use alloc::alloc::{alloc, Layout};

use crate::scheduler::SCHEDULER;

use crate::arch::gdt;
use crate::thread::Thread;

impl Thread {
    pub fn new(
        process_id: usize,
        thread_id: usize,
        additional_stack: usize,
    ) -> Self {
        unsafe {
            assert!(
                SCHEDULER.process_by_id(process_id).is_some(),
                "no such process",
            );
        }

        let kernel_stack_bottom =
            unsafe { alloc(Layout::from_size_align(65536, 4096).unwrap()) }
                .wrapping_add(65536) as *mut u32;
        let kernel_stack_top =
            kernel_stack_bottom.wrapping_sub(9 + additional_stack);

        let tcb = ThreadControlBlock {
            cr3: crate::arch::vas::KERNEL_VAS.lock().pgdir_phys,
            esp0: kernel_stack_bottom as u32, // FIXME: set to 0
            esp: kernel_stack_top as u32,
            tls: 0,
        };

        Thread {
            id: thread_id,
            process_id,

            tcb,
        }
    }

    pub fn new_with_stack(
        process_id: usize,
        thread_id: usize,
        entry_point: u32,
        additional_stack: usize,
    ) -> Self {
        let thread = Thread::new(process_id, thread_id, additional_stack);

        // Make an initial stack frame that will be popped on a thread switch
        // (see scheduler.s).
        let kernel_stack_top = thread.tcb.esp as *mut u32;
        unsafe {
            *kernel_stack_top.wrapping_add(0) = 0; // edi
            *kernel_stack_top.wrapping_add(1) = 0; // esi
            *kernel_stack_top.wrapping_add(2) = 0; // ebx
            *kernel_stack_top.wrapping_add(3) = 0; // edx
            *kernel_stack_top.wrapping_add(4) = 0; // ecx
            *kernel_stack_top.wrapping_add(5) = 0; // eax
            *kernel_stack_top.wrapping_add(6) = 0x00000000; // ebp
            // ebp = 0x00000000 is a magic value that makes the stack tracer to
            // stop.  It is used here the same way as in boot.s.
            *kernel_stack_top.wrapping_add(7) = entry_point; // eip
            *kernel_stack_top.wrapping_add(8) = 0x00000000;
            // Here 0x00000000 is just some value for the stack tracer to print
            // out as EIP instead of the heap garbage after the stack.  Also it
            // may serve as a return address for default_entry_point().
        }

        thread
    }

    pub fn set_tls(&mut self, value: usize) {
        self.tcb.tls = value as u32;
        self.load_tls();
    }

    pub fn load_tls(&self) {
        gdt::GDT.lock().0[gdt::TLS_IDX].set_base(self.tcb.tls);
        unsafe {
            asm!(
                "movw %ax, %gs",
                in("ax") gdt::TLS_SEG | 3, // usermode TLS segment selector
                options(att_syntax),
            );
        }
    }
}

#[derive(Clone, Copy)]
#[repr(C, packed)]
pub struct ThreadControlBlock {
    // NOTE: if you change the order of these fields, you'll also need to edit
    // switch_threads() in scheduler.s accordingly.
    pub cr3: u32,
    pub esp0: u32,
    pub esp: u32,
    pub tls: u32,
}
