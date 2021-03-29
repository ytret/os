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

use alloc::vec::Vec;
use core::sync::atomic::{AtomicU32, Ordering};

use crate::timer::TIMER;

use crate::arch;
use crate::arch::process::{Process, ProcessControlBlock};

/// A counter used by the scheduler to count the number of tasks that want
/// the interrupts to be disabled in order to perform their critical stuff.
pub static NO_SCHED_COUNTER: AtomicU32 = AtomicU32::new(0);

pub struct Scheduler {
    counter: u64, // ms
    processes: Vec<Process>,
    current_idx: usize,
}

impl Scheduler {
    pub const fn new() -> Self {
        Scheduler {
            counter: 0,
            processes: Vec::new(),
            current_idx: 0,
        }
    }

    pub fn add_process(&mut self, process: Process) {
        self.processes.push(process);
    }

    pub fn current_process(&mut self) -> &mut Process {
        &mut self.processes[self.current_idx]
    }

    pub fn schedule(&mut self, add_count: u32) {
        self.counter += add_count as u64;
        if NO_SCHED_COUNTER.load(Ordering::SeqCst) == 0
            && self.processes.len() > 1
        {
            // println!("[SCHED] Next process, total: {}", self.processes.len());
            self.next_process();
        } else {
            println!(
                "[SCHED] Not scheduling. (There are {} processes.)",
                self.processes.len(),
            );
        }
    }

    fn next_process(&mut self) {
        assert!(
            self.current_idx < self.processes.len(),
            "current process index is outside the vector of processes",
        );

        let from_idx = self.current_idx;
        let from =
            &mut self.processes[from_idx].pcb as *mut ProcessControlBlock;

        self.current_idx = match self.current_idx {
            max if max + 1 == self.processes.len() => 0,
            not_max => not_max + 1,
        };
        let to_idx = self.current_idx;
        let to = &self.processes[to_idx].pcb as *const ProcessControlBlock;

        println!(" switching from {} to {}", from_idx, to_idx);
        // println!(" to Process struct addr: 0x{:08X}", to as *const _ as u32);
        // println!("  to.cr3 = 0x{:08X}", to.cr3);
        // println!("  to.esp0 = 0x{:08X}", to.esp0);
        // println!("  to.esp = 0x{:08X}", to.esp);

        self.switch_tasks(from, to);
    }
}

pub static mut SCHEDULER: Scheduler = Scheduler::new();

pub fn init() -> ! {
    arch::scheduler::init();

    unsafe {
        TIMER.as_mut().unwrap().set_callback(schedule);
    }

    init_entry_point();
}

static mut COUNTER_MS: u32 = 0;
pub static mut TEMP_SPAWNER_ON: bool = false;
static mut NUM_SPAWNED: usize = 0;

fn schedule() {
    unsafe {
        let period_ms = TIMER.as_ref().unwrap().period_ms() as u32;
        COUNTER_MS += period_ms;

        if TEMP_SPAWNER_ON && NUM_SPAWNED < 2 {
            println!("[PIT] Creating a new process.");
            let new_process = Process::new();
            SCHEDULER.add_process(new_process);
            NUM_SPAWNED += 1;
        }

        if COUNTER_MS >= 1000 {
            COUNTER_MS = 0;
            // println!("SCHEDULING (period_ms = {})", period_ms);
            SCHEDULER.schedule(period_ms);
        }
    }
}

fn init_entry_point() -> ! {
    println!("[INIT] Init process entry point.");
    println!("[INIT] End of init process reached.");
    loop {}
}
