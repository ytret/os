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

use core::sync::atomic::Ordering;

use crate::arch::gdt;
use crate::arch::process::{Process, ProcessControlBlock};
use crate::scheduler::{NO_SCHED_COUNTER, SCHEDULER};

extern "C" {
    fn switch_tasks(
        from: *mut ProcessControlBlock,
        to: *const ProcessControlBlock,
        tss: *mut gdt::TaskStateSegment,
    );

    pub fn jump_into_usermode(
        code_seg: u16,
        data_seg: u16,
        jump_to: extern "C" fn() -> !,
    ) -> !;
}

impl crate::scheduler::Scheduler {
    pub fn switch_tasks(
        &self,
        from: *mut ProcessControlBlock,
        to: *const ProcessControlBlock,
    ) {
        // NOTE: call this method with interrupts disabled and enable them after
        // it returns.
        unsafe {
            let tss = &mut gdt::TSS as *mut gdt::TaskStateSegment;
            switch_tasks(from, to, tss);
        }
    }

    pub fn stop_scheduling(&self) {
        unsafe {
            asm!("cli");
        }
        NO_SCHED_COUNTER.fetch_add(1, Ordering::SeqCst);
    }

    pub fn keep_scheduling(&self) {
        NO_SCHED_COUNTER.fetch_sub(1, Ordering::SeqCst);
        if NO_SCHED_COUNTER.load(Ordering::SeqCst) == 0 {
            unsafe {
                asm!("sti");
            }
        }
    }
}

pub fn init() -> ! {
    let mut tss = unsafe { &mut gdt::TSS };
    tss.ss0 = gdt::KERNEL_DATA_SEG;

    // This process has no entry point like an ordinary one, as it is simply
    // the code that is executing now.  The first task switch that happens
    // after enablig the spawner will save the current context as a context
    // of the process with index 0.
    let init_process = Process::new();
    tss.esp0 = init_process.pcb.esp0;

    unsafe {
        // Load the GDT with the new entries.
        gdt::GDT.lock().load();

        // Load the TSS.
        asm!("ltr %ax", in("ax") gdt::TSS_SEG, options(att_syntax));
    }

    unsafe {
        SCHEDULER.add_process(init_process);
    }

    println!("[SCHED] Enabling the spawner.");
    crate::arch::pit::TEMP_SPAWNER_ON
        .store(true, core::sync::atomic::Ordering::SeqCst);

    crate::arch::pic::PIC.set_irq_mask(crate::arch::pit::IRQ, false);
    init_entry_point();
}

fn init_entry_point() -> ! {
    // If this was an ordinary process, here would be an 'sti' instruction.
    // But this is the process with index 0 that is called directly from the
    // kernel and not the scheduler, so the interrupts have been already
    // enabled.

    println!("[INIT] Init process entry point.");
    println!("[INIT] End of init process reached.");
    loop {}
}
