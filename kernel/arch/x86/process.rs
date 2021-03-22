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
use alloc::vec::Vec;

use crate::arch::gdt;
use crate::arch::vas::VirtAddrSpace;
use crate::fs;
use crate::scheduler::SCHEDULER;

extern "C" {
    fn jump_into_usermode(
        code_seg: u16,
        data_seg: u16,
        jump_to: unsafe extern "C" fn() -> !,
    ) -> !;

    fn usermode_part() -> !;
}

pub const MAX_OPENED_FILES: i32 = 32;

pub struct Process {
    pub pcb: ProcessControlBlock,
    pub opened_files: Vec<OpenedFile>,
}

impl Process {
    pub fn new() -> Self {
        let kernel_stack_bottom =
            unsafe { alloc(Layout::from_size_align(65536, 4096).unwrap()) }
                .wrapping_offset(65536) as *mut u32;

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

    pub fn open_file_by_node(
        &mut self,
        node: fs::Node,
    ) -> Result<i32, OpenFileErr> {
        let file_type = node.0.borrow()._type.clone();
        if file_type == fs::NodeType::RegularFile
            || file_type == fs::NodeType::BlockDevice
            || file_type == fs::NodeType::CharDevice
        {
            if self.opened_files.len() == MAX_OPENED_FILES as usize {
                return Err(OpenFileErr::MaxOpenedFiles);
            }
            let fd = self.opened_files.len() as i32;
            self.opened_files
                .push(OpenedFile::new(node.clone(), file_type.is_seekable()));
            Ok(fd)
        } else {
            Err(OpenFileErr::UnsupportedFileType)
        }
    }

    pub fn opened_file(&mut self, fd: i32) -> &mut OpenedFile {
        &mut self.opened_files[fd as usize]
    }

    pub fn check_fd(&self, fd: i32) -> bool {
        return 0 <= fd && fd < self.opened_files.len() as i32;
    }
}

#[derive(Debug)]
pub enum OpenFileErr {
    MaxOpenedFiles,
    UnsupportedFileType,
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

pub struct OpenedFile {
    node: fs::Node,
    offset: Option<usize>,
}

impl OpenedFile {
    fn new(node: fs::Node, seekable: bool) -> Self {
        OpenedFile {
            node,
            offset: if seekable { Some(0) } else { None },
        }
    }

    fn seek(&mut self, add_offset: usize) {
        if let Some(offset) = self.offset.as_mut() {
            *offset += add_offset;
        }
    }

    fn read(&mut self, count: usize) -> Vec<u8> {
        let fs = self.node.fs();
        let id_in_fs = self.node.0.borrow().id_in_fs.unwrap();
        let res = fs
            .read_file(id_in_fs, self.offset.unwrap_or(0), count)
            .unwrap();
        self.seek(count);
        res
    }

    pub fn write(&mut self, buf: &[u8]) {
        let fs = self.node.fs();
        let id_in_fs = self.node.0.borrow().id_in_fs.unwrap();
        fs.write_file(id_in_fs, self.offset.unwrap_or(0), buf)
            .unwrap();
        self.seek(buf.len());
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

    unsafe {
        println!("[PROC] Entering usermode.");
        jump_into_usermode(
            gdt::USERMODE_CODE_SEG,
            gdt::USERMODE_DATA_SEG,
            usermode_part,
        );
    }
}
