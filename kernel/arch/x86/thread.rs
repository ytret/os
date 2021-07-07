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

use alloc::alloc::Layout;

use crate::scheduler::SCHEDULER;

use crate::arch::gdt;
use crate::stack::Stack;
use crate::thread::Thread;

impl Thread {
    pub fn new(process_id: usize, thread_id: usize) -> Self {
        unsafe {
            assert!(
                SCHEDULER.process_by_id(process_id).is_some(),
                "no such process",
            );
        }

        let kernel_stack_layout = Layout::from_size_align(65536, 4096).unwrap();
        let kernel_stack = Stack::new(kernel_stack_layout);

        let tcb = ThreadControlBlock {
            cr3: crate::arch::vas::KERNEL_VAS.lock().pgdir_phys,
            esp0: kernel_stack.bottom as u32,
            esp: kernel_stack.top as u32,
            tls: 0,
        };

        Thread {
            id: thread_id,
            process_id,

            kernel_stack,
            tcb,
        }
    }

    pub fn new_with_stack(
        process_id: usize,
        thread_id: usize,
        entry: u32,
        entry_args: &[u32],
    ) -> Self {
        let mut thread = Thread::new(process_id, thread_id);

        // Make an initial stack frame that will be popped on a thread switch
        // (see scheduler.s).
        for arg in entry_args.iter().rev() {
            thread.kernel_stack.push(arg.clone()).unwrap();
        }
        thread.kernel_stack.push(0x00000000).unwrap();
        // Here 0x00000000 is just some value for the stack tracer to print
        // out as EIP instead of the heap garbage after the stack.  Also it
        // may serve as a return address for default_entry_point().
        thread.kernel_stack.push(entry).unwrap(); // eip
        thread.kernel_stack.push(0x00000000).unwrap();
        // ebp = 0x00000000 is a magic value that makes the stack tracer to
        // stop.  It is used here the same way as in boot.s.
        thread.kernel_stack.push(0).unwrap(); // eax
        thread.kernel_stack.push(0).unwrap(); // ecx
        thread.kernel_stack.push(0).unwrap(); // edx
        thread.kernel_stack.push(0).unwrap(); // ebx
        thread.kernel_stack.push(0).unwrap(); // esi
        thread.kernel_stack.push(0).unwrap(); // edi

        thread.tcb.esp = thread.kernel_stack.top as u32;
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
