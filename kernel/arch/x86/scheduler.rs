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

use core::sync::atomic::Ordering;

use crate::scheduler::{NO_SCHED_COUNTER, SCHEDULER, TEMP_SPAWNER_ON};

use crate::arch::gdt;
use crate::arch::thread::ThreadControlBlock;
use crate::process::Process;
use crate::thread::Thread;

extern "C" {
    fn switch_threads(
        from: *mut ThreadControlBlock,
        to: *const ThreadControlBlock,
        tss: *mut gdt::TaskStateSegment,
    );
}

impl crate::scheduler::Scheduler {
    pub fn switch_threads(
        &self,
        from: *mut ThreadControlBlock,
        to: *const ThreadControlBlock,
    ) {
        // NOTE: call this method with interrupts disabled and enable them after
        // it returns.
        unsafe {
            let tss = &mut gdt::TSS as *mut gdt::TaskStateSegment;
            switch_threads(from, to, tss);
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

pub fn init() {
    let mut tss = unsafe { &mut gdt::TSS };
    tss.ss0 = gdt::KERNEL_DATA_SEG;

    unsafe {
        let init_process_id = SCHEDULER.allocate_process_id();
        let mut init_process = Process::new(init_process_id);
        let init_thread_id = init_process.allocate_thread_id();
        SCHEDULER.add_process(init_process);

        // This thread has no entry point like an ordinary one, as it is simply
        // the code that is executing now.  The first thread switch that happens
        // after enablig the spawner will save the current context as a context
        // of the thread with index 0.
        let init_thread = Thread::new(init_process_id, init_thread_id);
        tss.esp0 = init_thread.tcb.esp0;

        // Load the GDT with the new entries.
        gdt::GDT.lock().load();

        // Load the TSS.
        asm!("ltr %ax", in("ax") gdt::TSS_SEG, options(att_syntax));

        SCHEDULER.run_thread(init_thread);

        println!("[SCHED] Enabling the spawner.");
        TEMP_SPAWNER_ON = true;
    }
}
