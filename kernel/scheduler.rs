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

use alloc::collections::vec_deque::VecDeque;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicU32, Ordering};

use crate::dev::timer::TIMER;
use crate::process::default_entry_point;

use crate::arch;
use crate::arch::thread::ThreadControlBlock;
use crate::arch::vas::VirtAddrSpace;
use crate::process::Process;
use crate::thread::Thread;

const SCHEDULING_PERIOD_MS: u32 = 50;

/// A counter used by the scheduler to count the number of threads that want the
/// interrupts to be disabled in order to perform their critical stuff.
pub static NO_SCHED_COUNTER: AtomicU32 = AtomicU32::new(0);

pub struct Scheduler {
    counter: u64, // ms

    processes: Vec<Process>,

    runnable_threads: Option<VecDeque<Thread>>,
    blocked_threads: Option<VecDeque<Thread>>,
    terminated_threads: Option<VecDeque<(Thread, i32)>>,
    running_thread: Option<Thread>,

    new_process_id: usize,
}

impl Scheduler {
    pub const fn new() -> Self {
        Scheduler {
            counter: 0,

            processes: Vec::new(),

            runnable_threads: None,
            blocked_threads: None,
            terminated_threads: None,
            running_thread: None,

            new_process_id: 0,
        }
    }

    pub fn init_vec_deques(&mut self) {
        assert!(self.runnable_threads.is_none());
        assert!(self.blocked_threads.is_none());
        assert!(self.terminated_threads.is_none());
        self.runnable_threads = Some(VecDeque::new());
        self.blocked_threads = Some(VecDeque::new());
        self.terminated_threads = Some(VecDeque::new());
    }

    pub fn allocate_process_id(&mut self) -> usize {
        let id = self.new_process_id;
        self.new_process_id += 1;
        id
    }

    pub fn add_process(&mut self, process: Process) {
        self.processes.push(process)
    }

    pub fn next_runnable_thread(&mut self) -> Thread {
        self.runnable_threads.as_mut().unwrap().pop_front().unwrap()
    }

    pub fn unblock_thread_by_id(
        &mut self,
        process_id: usize,
        thread_id: usize,
    ) {
        let idx = self
            .blocked_threads
            .as_ref()
            .unwrap()
            .iter()
            .position(|x| x.process_id == process_id && x.id == thread_id)
            .unwrap();
        let thread =
            self.blocked_threads.as_mut().unwrap().remove(idx).unwrap();
        self.runnable_threads.as_mut().unwrap().push_front(thread);
        // println!(
        //     "[SCHED] Unblocked thread {} of pid {}.",
        //     thread_id, process_id,
        // );
    }

    pub fn run_thread(&mut self, thread: Thread) {
        thread.load_tls();
        self.running_thread = Some(thread);
    }

    pub fn running_process(&mut self) -> &mut Process {
        let id = self.running_thread().process_id;
        self.process_by_id(id).unwrap()
    }

    pub fn running_thread(&mut self) -> &mut Thread {
        self.running_thread.as_mut().unwrap()
    }

    pub fn process_by_id(&mut self, id: usize) -> Option<&mut Process> {
        if let Some(idx) = self.processes.iter().position(|x| x.id == id) {
            Some(&mut self.processes[idx])
        } else {
            None
        }
    }

    pub fn terminate_running_thread(&mut self, status: i32) -> ! {
        assert_ne!(
            self.runnable_threads.as_ref().unwrap().len(),
            0,
            "cannot terminate the last thread",
        );
        let old_thread = self.running_thread.take().unwrap();
        let new_thread = self.next_runnable_thread();
        self.run_thread(new_thread);

        println!(
            "[SCHED] Terminated thread pid {} tid {} with status {}.",
            old_thread.process_id, old_thread.id, status,
        );

        self.terminated_threads
            .as_mut()
            .unwrap()
            .push_back((old_thread, status));
        let from_tcb = &mut self
            .terminated_threads
            .as_mut()
            .unwrap()
            .back_mut()
            .unwrap()
            .0
            .tcb as *mut ThreadControlBlock;
        let to_tcb =
            &mut self.running_thread().tcb as *const ThreadControlBlock;

        unsafe {
            self.switch_threads(from_tcb, to_tcb);
        }

        unreachable!();
    }

    pub fn block_running_thread(&mut self) {
        self.schedule(0, false);
    }

    pub fn schedule(&mut self, add_count: u32, still_runnable: bool) {
        self.counter += add_count as u64;
        if NO_SCHED_COUNTER.load(Ordering::SeqCst) == 0
            && self.runnable_threads.as_ref().unwrap().len() > 0
        {
            let old_thread = self.running_thread.take().unwrap();
            let new_thread = self.next_runnable_thread();

            self.run_thread(new_thread);
            let from_tcb = if still_runnable {
                self.runnable_threads
                    .as_mut()
                    .unwrap()
                    .push_back(old_thread);
                &mut self
                    .runnable_threads
                    .as_mut()
                    .unwrap()
                    .back_mut()
                    .unwrap()
                    .tcb as *mut ThreadControlBlock
            } else {
                // println!(
                //     "[SCHED] Blocking thread {} of pid {}.",
                //     old_thread.id, old_thread.process_id,
                // );
                self.blocked_threads.as_mut().unwrap().push_back(old_thread);
                &mut self
                    .blocked_threads
                    .as_mut()
                    .unwrap()
                    .back_mut()
                    .unwrap()
                    .tcb as *mut ThreadControlBlock
            };

            let to_tcb =
                &mut self.running_thread().tcb as *const ThreadControlBlock;

            unsafe {
                self.switch_threads(from_tcb, to_tcb);
            }
        } else {
            // if self.counter % 1000 == 0 {
            //     println!(
            //         "[SCHED] Not scheduling. (There are {} runnable and {} blocked threads.)",
            //         self.runnable_threads.as_ref().unwrap().len(),
            //         self.blocked_threads.as_ref().unwrap().len(),
            //     );
            // }
        }
    }
}

pub static mut SCHEDULER: Scheduler = Scheduler::new();

pub fn init() -> ! {
    unsafe {
        SCHEDULER.init_vec_deques();
    }

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

        if TEMP_SPAWNER_ON && NUM_SPAWNED < 1 {
            let process_id = SCHEDULER.allocate_process_id();
            let mut process =
                Process::new(process_id, VirtAddrSpace::kvas_copy_on_heap());
            let thread_id = process.allocate_thread_id();
            let pgdir_phys = process.vas.pgdir_phys;
            SCHEDULER.add_process(process);
            println!("[SCHED] Created a process with ID {}.", process_id);

            let mut new_thread = Thread::new_with_stack(
                process_id,
                thread_id,
                default_entry_point,
            );
            new_thread.tcb.cr3 = pgdir_phys;
            SCHEDULER
                .runnable_threads
                .as_mut()
                .unwrap()
                .push_back(new_thread);
            println!("[SCHED] Created a thread with ID {}.", thread_id);

            NUM_SPAWNED += 1;
        }

        if COUNTER_MS >= SCHEDULING_PERIOD_MS {
            COUNTER_MS = 0;
            SCHEDULER.schedule(SCHEDULING_PERIOD_MS, true);
        }
    }
}

fn init_entry_point() -> ! {
    println!("[INIT] Init process entry point.");
    println!("[INIT] End of init process reached.");
    loop {}
}
