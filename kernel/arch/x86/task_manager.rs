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

use crate::arch::vas::KERNEL_VAS;
use crate::task_manager::{NO_SCHED_COUNTER, TASK_MANAGER, TEMP_SPAWNER_ON};

use crate::arch::gdt;
use crate::arch::task::TaskControlBlock;
use crate::task::Task;
use crate::task_manager::TaskManager;

extern "C" {
    fn switch_tasks(
        from: *const TaskControlBlock,
        to: *const TaskControlBlock,
        tss: *mut gdt::TaskStateSegment,
    );
}

impl TaskManager {
    /// Stores the current task's context in its [TaskControlBlock] and loads
    /// the next task's context.
    ///
    /// # Notes
    /// Call this method with disabled interrupts and enable them after it
    /// returns.
    pub unsafe fn switch_tasks(
        &self,
        from: *const TaskControlBlock,
        to: *const TaskControlBlock,
    ) {
        let tss = &mut gdt::TSS as *mut gdt::TaskStateSegment;
        switch_tasks(from, to, tss);
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
        let init_task_id = TASK_MANAGER.allocate_task_id();

        // The init task is created with an empty kernel stack because it will
        // not switched to, it will be switched from, so its context will be
        // pushed, not popped on the next task switch.
        let init_task =
            Task::with_empty_stack(init_task_id, KERNEL_VAS.lock().clone());

        // Load the GDT with the new entries.
        gdt::GDT.lock().load();

        // Load the TSS.
        asm!("ltr %ax", in("ax") gdt::TSS_SEG, options(att_syntax));

        TASK_MANAGER.run_task(init_task);

        println!("[TASKMGR] Enabling the spawner.");
        TEMP_SPAWNER_ON = true;
    }
}
