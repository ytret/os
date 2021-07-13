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
use core::sync::atomic::{AtomicU32, Ordering};

use crate::arch::task::default_entry_point;
use crate::dev::timer::TIMER;

use crate::arch;
use crate::arch::vas::VirtAddrSpace;
use crate::task::Task;

/// A counter used by the scheduler to count the number of tasks that want the
/// interrupts to be disabled in order to perform their critical stuff.
pub static NO_SCHED_COUNTER: AtomicU32 = AtomicU32::new(0);

pub struct TaskManager {
    counter_ms: u64,

    running_task: Option<Task>,
    runnable_tasks: Option<VecDeque<Task>>,
    blocked_tasks: Option<VecDeque<Task>>,
    terminated_tasks: Option<VecDeque<(Task, i32)>>,

    new_task_id: usize,
}

impl TaskManager {
    pub const fn new() -> Self {
        TaskManager {
            counter_ms: 0,

            running_task: None,
            runnable_tasks: None,
            blocked_tasks: None,
            terminated_tasks: None,

            new_task_id: 0,
        }
    }

    pub fn init_vecs(&mut self) {
        assert!(self.runnable_tasks.is_none());
        assert!(self.blocked_tasks.is_none());
        assert!(self.terminated_tasks.is_none());
        self.runnable_tasks = Some(VecDeque::new());
        self.blocked_tasks = Some(VecDeque::new());
        self.terminated_tasks = Some(VecDeque::new());
    }

    pub fn allocate_task_id(&mut self) -> usize {
        let id = self.new_task_id;
        self.new_task_id += 1;
        id
    }

    pub fn this_task(&mut self) -> &mut Task {
        self.running_task.as_mut().unwrap()
    }

    pub fn run_task(&mut self, task: Task) {
        unsafe {
            task.load_tls();
        }
        self.running_task = Some(task);
    }

    pub fn add_runnable_task(&mut self, task: Task) {
        self.runnable_tasks.as_mut().unwrap().push_back(task);
    }

    pub fn next_runnable_task(&mut self) -> Task {
        self.runnable_tasks.as_mut().unwrap().pop_front().unwrap()
    }

    pub fn block_this_task(&mut self) {
        self.schedule(0, false);
    }

    pub fn unblock_task(&mut self, task_id: usize) {
        let idx = self
            .blocked_tasks
            .as_ref()
            .unwrap()
            .iter()
            .position(|x| x.id == task_id)
            .unwrap();
        let task = self.runnable_tasks.as_mut().unwrap().remove(idx).unwrap();
        self.runnable_tasks.as_mut().unwrap().push_front(task);
    }

    pub fn terminate_this_task(&mut self, status: i32) -> ! {
        assert_ne!(
            self.runnable_tasks.as_ref().unwrap().len(),
            0,
            "cannot terminate the last task",
        );
        let from_task = self.running_task.take().unwrap();
        let to_task = self.next_runnable_task();

        let from_id = from_task.id;
        let to_id = to_task.id;

        self.run_task(to_task);

        println!(
            "[TASKMGR] Terminated task ID {} with status {}",
            from_id, status,
        );

        self.terminated_tasks
            .as_mut()
            .unwrap()
            .push_back((from_task, status));

        let from_tcb = self
            .terminated_tasks
            .as_mut()
            .unwrap()
            .back_mut()
            .unwrap()
            .0
            .raw_tcb();
        let to_tcb = self.this_task().raw_tcb();

        println!("[TASKMGR] id {} -> id {}", from_id, to_id);

        unsafe {
            self.switch_tasks(from_tcb, to_tcb);
        }

        unreachable!();
    }

    pub fn schedule(&mut self, add_count_ms: u64, keep_runnable: bool) {
        self.counter_ms += add_count_ms;
        if NO_SCHED_COUNTER.load(Ordering::SeqCst) == 0
            && self.runnable_tasks.as_ref().unwrap().len() > 0
        {
            let from_task = self.running_task.take().unwrap();
            let to_task = self.next_runnable_task();

            let from_id = from_task.id;
            let to_id = to_task.id;

            self.run_task(to_task);

            let where_from_goes = if keep_runnable {
                self.runnable_tasks.as_mut().unwrap()
            } else {
                println!("[TASKMGR] Blocking task ID {}", from_id);
                self.blocked_tasks.as_mut().unwrap()
            };
            where_from_goes.push_back(from_task);

            let from_tcb = where_from_goes.back_mut().unwrap().raw_tcb();
            let to_tcb = self.this_task().raw_tcb();

            println!("[TASKMGR] id {} -> id {}", from_id, to_id);

            unsafe {
                self.switch_tasks(from_tcb, to_tcb);
            }
        } else {
            if self.counter_ms % 10000 == 0 {
                println!(
                    "[TASKMGR] Not scheduling. (There are {} runnable and {} blocked tasks.)",
                    self.runnable_tasks.as_ref().unwrap().len(),
                    self.blocked_tasks.as_ref().unwrap().len(),
                );
            }
        }
    }
}

pub static mut TASK_MANAGER: TaskManager = TaskManager::new();

pub fn init() -> ! {
    unsafe {
        TASK_MANAGER.init_vecs();
    }

    arch::task_manager::init();

    unsafe {
        TIMER.as_mut().unwrap().set_callback(schedule);
    }

    init_entry_point();
}

const SCHEDULING_PERIOD_MS: u64 = 50;

static mut COUNTER_MS: u64 = 0;
pub static mut TEMP_SPAWNER_ON: bool = false;
static mut NUM_SPAWNED: usize = 0;

pub fn schedule() {
    unsafe {
        let period_ms = TIMER.as_ref().unwrap().period_ms() as u64;
        COUNTER_MS += period_ms;

        if TEMP_SPAWNER_ON && NUM_SPAWNED < 1 {
            let task_id = TASK_MANAGER.allocate_task_id();
            let task = Task::with_filled_stack(
                task_id,
                VirtAddrSpace::kvas_copy_on_heap(),
                default_entry_point as u32,
                &[],
            );
            TASK_MANAGER.add_runnable_task(task);
            println!("[TASKMGR] Created a task with ID {}.", task_id);
            NUM_SPAWNED += 1;
        }

        if COUNTER_MS >= SCHEDULING_PERIOD_MS {
            COUNTER_MS = 0;
            TASK_MANAGER.schedule(SCHEDULING_PERIOD_MS, true);
        }
    }
}

fn init_entry_point() -> ! {
    println!("[INIT] Init process entry point.");
    println!("[INIT] End of init process.");
    loop {}
}
