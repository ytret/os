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

use crate::process::default_entry_point;
use crate::timer::TIMER;

use crate::arch;
use crate::arch::thread::ThreadControlBlock;
use crate::process::Process;
use crate::thread::Thread;

/// A counter used by the scheduler to count the number of threads that want the
/// interrupts to be disabled in order to perform their critical stuff.
pub static NO_SCHED_COUNTER: AtomicU32 = AtomicU32::new(0);

pub struct Scheduler {
    counter: u64, // ms

    processes: Vec<Process>,
    threads: Vec<Thread>,

    current_process_id: usize,
    current_thread_id: usize,

    new_process_id: usize,
}

impl Scheduler {
    pub const fn new() -> Self {
        Scheduler {
            counter: 0,

            processes: Vec::new(),
            threads: Vec::new(),

            current_process_id: 0,
            current_thread_id: 0,

            new_process_id: 0,
        }
    }

    pub fn allocate_process_id(&mut self) -> usize {
        let id = self.new_process_id;
        self.new_process_id += 1;
        id
    }

    pub fn add_process(&mut self, process: Process) {
        self.processes.push(process)
    }

    pub fn add_thread(&mut self, thread: Thread) {
        self.threads.push(thread)
    }

    pub fn current_process(&mut self) -> &mut Process {
        let current_process_id = self.current_thread().process_id;
        self.process_by_id(current_process_id).unwrap()
    }

    pub fn current_thread(&mut self) -> &mut Thread {
        self.thread_by_ids(self.current_process_id, self.current_thread_id)
            .unwrap()
    }

    pub fn process_by_id(&mut self, id: usize) -> Option<&mut Process> {
        if let Some(id_in_vec) = self.processes.iter().position(|x| x.id == id)
        {
            Some(&mut self.processes[id_in_vec])
        } else {
            None
        }
    }

    pub fn thread_by_ids(
        &mut self,
        process_id: usize,
        thread_id: usize,
    ) -> Option<&mut Thread> {
        if let Some(id_in_vec) = self
            .threads
            .iter()
            .position(|x| x.process_id == process_id && x.id == thread_id)
        {
            Some(&mut self.threads[id_in_vec])
        } else {
            None
        }
    }

    pub fn schedule(&mut self, add_count: u32) {
        self.counter += add_count as u64;
        if NO_SCHED_COUNTER.load(Ordering::SeqCst) == 0
            && self.threads.len() > 1
        {
            // println!("[SCHED] Next thread, total: {}", self.threads.len());
            self.next_thread();
        } else {
            println!(
                "[SCHED] Not scheduling. (There are {} threads.)",
                self.threads.len(),
            );
        }
    }

    fn next_thread(&mut self) {
        let from = &mut self.current_thread().tcb as *mut ThreadControlBlock;

        let next_idx = match self
            .threads
            .iter()
            .position(|x| {
                x.process_id == self.current_process_id
                    && x.id == self.current_thread_id
            })
            .unwrap()
            + 1
        {
            max if max == self.threads.len() => 0,
            not_max => not_max,
        };
        self.current_process_id = self.threads[next_idx].process_id;
        self.current_thread_id = self.threads[next_idx].id;
        let to = &self.current_thread().tcb as *const ThreadControlBlock;

        // println!(" switching from {} to {}", from_idx, to_idx);
        // println!(" to Thread struct addr: 0x{:08X}", to as *const _ as u32);
        // println!("  to.cr3 = 0x{:08X}", to.cr3);
        // println!("  to.esp0 = 0x{:08X}", to.esp0);
        // println!("  to.esp = 0x{:08X}", to.esp);

        assert_ne!(from as *const ThreadControlBlock, to);
        self.switch_threads(from, to);
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
            let process_id = SCHEDULER.allocate_process_id();
            let mut process = Process::new(process_id);
            let thread_id = process.allocate_thread_id();
            SCHEDULER.add_process(process);
            println!("[SCHED] Created a process with ID {}.", process_id);

            let new_thread = Thread::new_with_stack(
                process_id,
                thread_id,
                default_entry_point,
            );
            SCHEDULER.add_thread(new_thread);
            println!("[SCHED] Created a thread with ID {}.", thread_id);

            NUM_SPAWNED += 1;
        }

        if COUNTER_MS >= 25 {
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
