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

use alloc::alloc::{alloc, Layout};
use alloc::vec;
use alloc::vec::Vec;
use core::default::Default;
use core::ptr;

use crate::arch::pmm_stack::PMM_STACK;
use crate::arch::vas::USERMODE_REGION;
use crate::task::USERMODE_STACK_REGION;
use crate::task_manager::TASK_MANAGER;

use crate::arch::gdt;
use crate::arch::syscall::GpRegs;
use crate::arch::vas::{Table, VirtAddrSpace};
use crate::ffi::cstring::CString;
use crate::memory_region::Region;
use crate::stack::Stack;
use crate::task::Task;

extern "C" {
    /// Does an interrupt return with requested privilege level 3 (usermode).
    pub fn jump_into_usermode(
        code_seg: u16,
        data_seg: u16,
        tls_seg: u16,
        jump_to: u32,
        gp_regs: *const GpRegs,
    ) -> !;
}

impl Task {
    /// Creates a task with a kernel stack prepared for a task switch.
    ///
    /// # Safety
    /// * `entry` must be an address of some `extern "C"` function that will be
    ///   called on the first task switch to this task,
    /// * its arguments must be represented as `u32`s according to the System V
    ///   ABI,
    /// * they must be ordered the same way as in the entry function definition,
    /// * and their number must be valid.
    pub fn with_filled_stack(
        id: usize,
        vas: VirtAddrSpace,
        entry: u32,
        entry_args: &[u32],
    ) -> Self {
        let mut task = Self::with_empty_stack(id, vas);

        // Set up an initial stack that will be popped on a task switch (see
        // task_manager.s).
        for arg in entry_args.iter().rev() {
            task.kernel_stack.push(arg.clone()).unwrap();
        }
        task.kernel_stack.push(0x00000000).unwrap();
        // Here 0x00000000 is just some value for the stack tracer to print
        // out as EIP instead of some heap garbage after the stack.  Also it
        // may serve as an address to return to from default_entry_point().
        task.kernel_stack.push(entry).unwrap(); // eip
        task.kernel_stack.push(0x00000000).unwrap();
        // ebp = 0x00000000 is a magic value that makes the stack tracer to
        // stop.  It is used here the same way as in boot.s.
        task.kernel_stack.push(0).unwrap(); // eax
        task.kernel_stack.push(0).unwrap(); // ecx
        task.kernel_stack.push(0).unwrap(); // edx
        task.kernel_stack.push(0).unwrap(); // ebx
        task.kernel_stack.push(0).unwrap(); // esi
        task.kernel_stack.push(0).unwrap(); // edi

        task
    }

    pub unsafe fn set_tls(&mut self, value: usize) {
        self.tls = value as u32;
        self.load_tls();
    }

    pub unsafe fn load_tls(&self) {
        gdt::GDT.lock().0[gdt::TLS_IDX].set_base(self.tls);
        asm!(
            "movw %ax, %gs",
            in("ax") gdt::TLS_SEG | 3, // usermode TLS segment selector
            options(att_syntax),
        );
    }

    pub fn set_up_usermode_stack(
        &mut self,
        argv: &[CString],
        environ: &[CString],
    ) {
        // Allocate physical memory for the stack and map it.
        unsafe {
            for four_mib_chunk in USERMODE_STACK_REGION
                .align_boundaries_at(4 * 1024 * 1024)
                .range()
                .step_by(4 * 1024 * 1024)
            {
                let pde_idx = four_mib_chunk >> 22;
                let pgtbl_virt: *mut Table =
                    alloc(Layout::from_size_align(4096, 4096).unwrap()).cast();
                pgtbl_virt.write_bytes(0, 1);
                self.vas.set_pde_virt(pde_idx, pgtbl_virt);
            }

            for four_kib_chunk in USERMODE_STACK_REGION
                .align_boundaries_at(4096)
                .range()
                .step_by(4096)
            {
                let phys = PMM_STACK.lock().pop_page();
                self.vas.map_page(four_kib_chunk as u32, phys);
                (four_kib_chunk as *mut u8).write_bytes(0, 4096);
            }
        }

        self.usermode_stack =
            unsafe { Some(Stack::from_region(USERMODE_STACK_REGION)) };
        let usermode_stack = self.usermode_stack.as_mut().unwrap();

        // envp[]
        usermode_stack.push(0).unwrap(); // environ[len(environ)] = NULL
        for envp in environ.iter().rev() {
            usermode_stack.push(envp.as_ptr() as u32).unwrap();
        }

        // argv[]
        usermode_stack.push(0).unwrap(); // argv[argc] = NULL
        for arg in argv.iter().rev() {
            usermode_stack.push(arg.as_ptr() as u32).unwrap();
        }

        // argc
        usermode_stack.push(argv.len() as u32).unwrap();
    }

    // PROT_READ, PROT_WRITE, MAP_ANONYMOUS, MAP_PRIVATE
    pub fn mem_map(&mut self, len: usize) -> &MemMapping {
        assert_eq!(len % 4096, 0, "len must be page-aligned");
        let mut candidate = Region {
            start: USERMODE_REGION.start,
            end: USERMODE_REGION.start,
        };
        while candidate.len() < len {
            if candidate.conflicts_with(&USERMODE_STACK_REGION) {
                candidate.start = USERMODE_STACK_REGION.end;
                candidate.end = USERMODE_STACK_REGION.end;
            }
            for segment in &self.program_segments {
                if candidate.conflicts_with(segment) {
                    candidate.start = (segment.end + 0xFFF) & !0xFFF;
                    candidate.end = (segment.end + 0xFFF) & !0xFFF;
                }
            }
            for mapping in &self.mem_mappings {
                if candidate.conflicts_with(&mapping.region) {
                    candidate.start = (mapping.region.end + 0xFFF) & !0xFFF;
                    candidate.end = (mapping.region.end + 0xFFF) & !0xFFF;
                }
            }
            candidate.end += 4096;
        }
        assert!(candidate.is_in(&USERMODE_REGION));

        let mapping = MemMapping { region: candidate };
        unsafe {
            for four_mib_chunk in mapping
                .region
                .align_boundaries_at(4 * 1024 * 1024)
                .range()
                .step_by(4 * 1024 * 1024)
            {
                let pgtbl_virt = self.vas.pgtbl_virt_of(four_mib_chunk as u32);
                if pgtbl_virt.is_null() {
                    let pde_idx = four_mib_chunk >> 22;
                    let new_pgtbl_virt =
                        alloc(Layout::from_size_align(4096, 4096).unwrap())
                            as *mut Table;
                    new_pgtbl_virt.write_bytes(0, 1);
                    self.vas.set_pde_virt(pde_idx, new_pgtbl_virt);
                }
            }

            for four_kib_chunk in mapping
                .region
                .align_boundaries_at(4096)
                .range()
                .step_by(4096)
            {
                assert!(
                    !self.vas.is_mapped(four_kib_chunk as u32),
                    "page 0x{:08X} is already mapped to {:#X?}",
                    four_kib_chunk,
                    self.vas.virt_to_phys(four_kib_chunk as u32).unwrap(),
                );
                let phys = PMM_STACK.lock().pop_page();
                self.vas.map_page(four_kib_chunk as u32, phys);
                (four_kib_chunk as *mut u8).write_bytes(0, 4096);
            }
        }

        self.mem_mappings.push(mapping);
        self.mem_mappings.last().unwrap()
    }

    /// Updates the task's control block and returns a raw pointer to it.
    ///
    /// This should be preferred over obtaining the `tcb` field directly because
    /// that way the control block may have dangling pointers, e.g. if the task
    /// was moved into one of the task manager's vectors.
    pub fn raw_tcb(&mut self) -> *const TaskControlBlock {
        self.tcb = TaskControlBlock {
            pgdir_phys: self.vas.pgdir_phys,
            esp0: &self.kernel_stack.bottom as *const _ as *const u32,
            esp: &self.kernel_stack.top as *const _ as *mut u32,
        };
        &self.tcb as *const TaskControlBlock
    }
}

/// Packed C representation of [Task] for task switching.
///
/// This representation is used by assembly code responsible for task switching.
#[derive(Debug, Clone, Copy)]
#[repr(C, packed)]
pub struct TaskControlBlock {
    // NOTE: the field order is hard-coded in task_manager.s.
    pub pgdir_phys: u32,
    pub esp0: *const u32,
    pub esp: *mut u32,
}

impl Default for TaskControlBlock {
    fn default() -> Self {
        TaskControlBlock {
            pgdir_phys: 0x00000000,
            esp0: ptr::null(),
            esp: ptr::null_mut(),
        }
    }
}

#[derive(Clone)]
pub struct MemMapping {
    pub region: Region<usize>,
}

pub extern "C" fn default_entry_point() -> ! {
    // Reaching this function must always be a result of ret from switch_tasks
    // (see task_manager.s) which requires that interrupts be disabled after it
    // returns so that task switching remains possible.
    unsafe {
        asm!("sti");
    }

    println!("[TASK] Default task entry.");

    unsafe {
        TASK_MANAGER.stop_scheduling();

        let this_task = TASK_MANAGER.this_task();

        let argv = vec![CString::new("/bin/test-fork").unwrap()];
        let environ = Vec::new();

        let elf = this_task.load_from_file("/bin/test-fork");
        this_task.set_up_usermode_stack(&argv, &environ);

        TASK_MANAGER.keep_scheduling();

        let gp_regs = GpRegs {
            edi: 0,
            esi: 0,
            ebp: 0,
            esp: this_task.usermode_stack.as_ref().unwrap().top as u32,
            ebx: 0,
            edx: 0,
            ecx: 0,
            eax: 0,
        };
        println!("[TASK] Entering usermode at 0x{:08X}.", elf.entry_point);
        jump_into_usermode(
            gdt::USERMODE_CODE_SEG,
            gdt::USERMODE_DATA_SEG,
            gdt::TLS_SEG,
            elf.entry_point as u32,
            &gp_regs as *const GpRegs,
        );
    }
}
