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

use core::mem::size_of;

use crate::arch::gdt;
use crate::arch::process::Process;
use crate::bitflags::BitFlags;
use crate::scheduler::SCHEDULER;

extern "C" {
    fn jump_into_usermode(
        code_seg: u16,
        data_seg: u16,
        jump_to: extern "C" fn() -> !,
    ) -> !;

    fn switch_tasks(
        from: *mut Process,
        to: *const Process,
        tss: *mut gdt::TaskStateSegment,
    );
}

impl crate::scheduler::Scheduler {
    pub fn switch_tasks(&self, from: *mut Process, to: *const Process) {
        // NOTE: call this method with interrupts disabled and enable them after
        // it returns.
        unsafe {
            let tss = &mut gdt::TSS as *mut gdt::TaskStateSegment;
            switch_tasks(from, to, tss);
        }
    }
}

pub fn init() -> ! {
    let mut gdt = gdt::GDT.lock();
    let usermode_code_seg = gdt.usermode_code_segment();
    let usermode_data_seg = gdt.usermode_data_segment();
    let tss_seg = gdt.tss_segment();

    let mut tss = unsafe { &mut gdt::TSS };
    tss.ss0 = gdt.kernel_data_segment();

    // This process has no entry point like an ordinary one, as it is simply
    // the code that is executing now.  The first task switch that happens
    // after enablig the spawner will save the current context as a context
    // of the process with index 0.
    let init_process = Process::new();
    tss.esp0 = init_process.esp0;

    unsafe {
        // Load the GDT with the new entries.
        gdt.load();

        // Load the TSS.
        asm!("ltr %ax", in("ax") tss_seg, options(att_syntax));
    }

    unsafe {
        SCHEDULER.add_process(init_process);
    }

    println!("[SCHED] Enabling the spawner");
    crate::arch::pit::TEMP_SPAWNER_ON
        .store(true, core::sync::atomic::Ordering::SeqCst);

    init_entry_point();

    // unsafe {
    //     // Jump into usermode.
    //     jump_into_usermode(usermode_code_seg, usermode_data_seg, usermode_init);
    // }
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
