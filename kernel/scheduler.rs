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

use crate::arch;
use crate::arch::process::Process;

use alloc::vec::Vec;

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

    pub fn schedule(&mut self, add_count: u32) {
        self.counter += add_count as u64;
        if self.processes.len() > 1 {
            // println!("[SCHED] Next process, total: {}", self.processes.len());
            self.next_process();
        } else {
            println!("[SCHED] Too few processes: {}.", self.processes.len());
        }
    }

    fn next_process(&mut self) {
        assert!(
            self.current_idx < self.processes.len(),
            "current process index is outside the vector of processes",
        );

        let from_idx = self.current_idx;
        let from = &self.processes[self.current_idx] as *const _ as *mut _;

        self.current_idx = match self.current_idx {
            max if max + 1 == self.processes.len() => 0,
            not_max => not_max + 1,
        };
        let to = &self.processes[self.current_idx];
        let to_idx = self.current_idx;

        // println!(" switching from {} to {}", from_idx, to_idx);
        // println!(" to Process struct addr: 0x{:08X}", to as *const _ as u32);
        // println!("  to.cr3 = 0x{:08X}", to.cr3);
        // println!("  to.esp0 = 0x{:08X}", to.esp0);
        // println!("  to.esp = 0x{:08X}", to.esp);

        assert!(from as *const _ != to, "from and to point to the same task");
        self.switch_tasks(from, to);
    }
}

pub static mut SCHEDULER: Scheduler = Scheduler::new();

pub fn init() -> ! {
    arch::scheduler::init();
}
