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

use alloc::alloc::{alloc, Layout};
use alloc::rc::Rc;
use alloc::vec::Vec;

use crate::arch::gdt;
use crate::arch::vas::VirtAddrSpace;
use crate::fs;
use crate::scheduler::SCHEDULER;

pub struct Process {
    pub pcb: ProcessControlBlock,
    opened_files: Vec<OpenedFile>,
}

impl Process {
    pub fn new() -> Self {
        let kernel_stack_bottom =
            unsafe { alloc(Layout::from_size_align(1024, 4096).unwrap()) }
                .wrapping_offset(1024) as *mut u32;

        // Make an initial stack frame that will be popped on a task switch (see
        // scheduler.s).
        let kernel_stack_top = kernel_stack_bottom.wrapping_sub(8);
        unsafe {
            *kernel_stack_top.wrapping_add(0) = 0; // edi
            *kernel_stack_top.wrapping_add(1) = 0; // esi
            *kernel_stack_top.wrapping_add(2) = 0; // ecx
            *kernel_stack_top.wrapping_add(3) = 0; // ebx
            *kernel_stack_top.wrapping_add(4) = 0; // eax
            *kernel_stack_top.wrapping_add(5) = 0x00000000;
            // ebp = 0x00000000 is a magic value that makes the stack tracer to
            // stop.  It is used here the same way as in boot.s.
            *kernel_stack_top.wrapping_add(6) =
                default_entry_point as *const () as u32; // eip
            *kernel_stack_top.wrapping_add(7) = 0x00000000;
            // Here 0x00000000 is just some value for the stack tracer to print
            // out as EIP instead of the heap garbage after the stack.  Also it
            // may serve as a return address for default_entry_point().
        }

        let pcb = ProcessControlBlock {
            cr3: crate::arch::vas::KERNEL_VAS.lock().pgdir_phys,
            esp0: kernel_stack_bottom as u32,
            esp: kernel_stack_top as u32,
        };

        Process {
            pcb,
            opened_files: Vec::new(),
        }
    }

    fn open_file_by_node(
        &mut self,
        node: fs::Node,
    ) -> Result<usize, OpenFileErr> {
        if node.0.borrow()._type == fs::NodeType::RegularFile {
            let fd = self.opened_files.len();
            self.opened_files.push(OpenedFile::new(node.clone()));
            Ok(fd)
        } else {
            Err(OpenFileErr::NotRegularFile)
        }
    }
}

#[derive(Debug)]
enum OpenFileErr {
    NotRegularFile,
    IdNotAssigned,
}

#[derive(Clone, Copy)]
#[repr(C, packed)]
pub struct ProcessControlBlock {
    // NOTE: when changing the order of these fields, also edit switch_tasks()
    // in scheduler.s.
    pub cr3: u32,
    pub esp0: u32,
    pub esp: u32,
}

struct OpenedFile {
    node: fs::Node,
    offset: usize,
    contents: Vec<u8>,
}

impl OpenedFile {
    fn new(node: fs::Node) -> Self {
        let id_in_fs = node.0.borrow().id_in_fs.unwrap();
        let fs = node.fs();
        OpenedFile {
            node,
            offset: 0,
            contents: fs.read_file(id_in_fs).unwrap(),
        }
    }

    fn read(&mut self, mut count: usize) -> &[u8] {
        if self.offset + count >= self.contents.len() {
            count = self.contents.len() - self.offset;
        }
        if count != 0 {
            let res = &self.contents[self.offset..count];
            res
        } else {
            &[]
        }
    }
}

fn default_entry_point() -> ! {
    // This function must always be a result of ret from switch_tasks (see
    // scheduler.s) which requires that interrupts be enabled after it returns
    // so that task switching remains possible.
    unsafe {
        asm!("sti");
    }

    println!("[PROC] Default process entry. Starting initialization.");

    unsafe {
        SCHEDULER.stop_scheduling();
        println!("[PROC] Creating a new VAS for the process.");
        let vas = VirtAddrSpace::kvas_copy_on_heap();
        println!("[PROC] Loading the VAS.");
        vas.load();
        SCHEDULER.keep_scheduling();
    }

    // unsafe {
    //     println!("[PROC] Entering usermode.");
    //     scheduler::jump_into_usermode(
    //         gdt::USERMODE_CODE_SEG,
    //         gdt::USERMODE_DATA_SEG,
    //         usermode_part,
    //     );
    // }

    unsafe {
        SCHEDULER.stop_scheduling();
        println!("[PROC] Opening the test file.");
        let root_node = crate::arch::pci::TEST_VFS.as_ref().unwrap().clone();
        println!("[PROC] Root node: {:?}", root_node.clone());
        let test_file =
            root_node.0.borrow().maybe_children.as_ref().unwrap()[2].clone();
        println!("[PROC] Test file node: {:?}", test_file);
        let fd = SCHEDULER
            .current_process()
            .open_file_by_node(test_file)
            .unwrap();
        let f = &mut SCHEDULER.current_process().opened_files[fd];
        let buf = f.read(11);
        println!("{}", core::str::from_utf8(&buf).unwrap());
        println!("[PROC] Closing the test file.");
        SCHEDULER.keep_scheduling();
    }

    println!("[PROC] End of default process entry.");
    loop {}
}
