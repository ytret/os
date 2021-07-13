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
use alloc::boxed::Box;
use alloc::vec;
use alloc::vec::Vec;
use core::slice;

use crate::arch::pmm_stack::PMM_STACK;
use crate::arch::vas::USERMODE_REGION;
use crate::dev::console::CONSOLE;

use crate::arch::task::{MemMapping, TaskControlBlock};
use crate::arch::vas::{Table, VirtAddrSpace};
use crate::elf::{ElfObj, ProgSegmentType};
use crate::feeder::Feeder;
use crate::fs;
use crate::memory_region::Region;
use crate::stack::Stack;
use crate::syscall;

pub const USERMODE_STACK_REGION: Region<usize> = Region {
    start: 3 * 1024 * 1024 * 1024,      // 3 GiB
    end: 3 * 1024 * 1024 * 1024 + 4096, // 3 GiB + 4 KiB
};

pub const MAX_OPENED_FILES: usize = 32;

pub struct Task {
    pub id: usize,

    pub vas: VirtAddrSpace,
    pub program_segments: Vec<Region<usize>>,
    pub mem_mappings: Vec<MemMapping>,
    pub kernel_stack: Stack<u32>,
    pub usermode_stack: Option<Stack<u32>>,
    pub tls: u32,

    opened_files: Vec<OpenedFile>,

    pub tcb: TaskControlBlock,
}

impl Task {
    /// Creates a task with an empty kernel stack.
    ///
    /// For the task to be scheduled, it must be added to [the task
    /// manager](crate::task_manager::TaskManager).  However, in order for the
    /// task switch to be successful, there must be certain items on the task's
    /// kernel stack (see [`crate::arch::task::Task::with_filled_stack()`]).
    pub fn with_empty_stack(id: usize, vas: VirtAddrSpace) -> Self {
        let kernel_stack_layout = Layout::from_size_align(65536, 4096).unwrap();
        let kernel_stack = Stack::with_layout(kernel_stack_layout);

        let mut task = Task {
            id,

            vas,
            mem_mappings: Vec::new(),
            program_segments: Vec::new(),
            kernel_stack,
            usermode_stack: None,
            tls: 0x00000000,

            opened_files: Vec::new(),

            tcb: TaskControlBlock::default(),
        };

        // Open stdin, stdout, stderr.
        assert!(CONSOLE.lock().is_some());
        let stdin = fs::VFS_ROOT
            .lock()
            .as_mut()
            .unwrap()
            .path("/dev/chr0")
            .unwrap();
        let stdout = fs::VFS_ROOT
            .lock()
            .as_mut()
            .unwrap()
            .path("/dev/chr0")
            .unwrap();
        let stderr = fs::VFS_ROOT
            .lock()
            .as_mut()
            .unwrap()
            .path("/dev/chr0")
            .unwrap();
        assert_eq!(task.open_file_by_node(stdin).unwrap(), 0);
        assert_eq!(task.open_file_by_node(stdout).unwrap(), 1);
        assert_eq!(task.open_file_by_node(stderr).unwrap(), 2);

        task
    }

    /// Reads loadable ELF segments into memory from an executable.
    pub unsafe fn load_from_file(&mut self, pathname: &str) -> ElfObj {
        // FIXME: no syscalls here

        println!("[TASK] Loading from file {}.", pathname);

        let fd = syscall::open(pathname).unwrap();
        let elf = ElfObj::from(self.opened_file(fd)).unwrap();

        for segment in &elf.program_segments {
            let mem_reg =
                Region::from_start_len(segment.in_mem_at, segment.in_mem_size);

            self.program_segments.push(mem_reg);

            if segment._type != ProgSegmentType::Load {
                continue;
            }

            assert!(mem_reg.is_in(&USERMODE_REGION));
            assert!(!mem_reg.conflicts_with(&USERMODE_STACK_REGION));
            // FIXME: check for conflicting with other regions?

            if self.vas.pgtbl_virt_of(mem_reg.start as u32).is_null() {
                let pde_idx = mem_reg.start >> 22;
                let pgtbl_virt =
                    alloc(Layout::from_size_align(4096, 4096).unwrap())
                        as *mut Table;
                pgtbl_virt.write_bytes(0, 1);
                self.vas.set_pde_virt(pde_idx, pgtbl_virt);
            }

            for virt_page in
                mem_reg.align_boundaries_at(4096).range().step_by(4096)
            {
                if self.vas.virt_to_phys(virt_page as u32).is_none() {
                    let phys_page = PMM_STACK.lock().pop_page();
                    self.vas.map_page(virt_page as u32, phys_page);
                    (virt_page as *mut u8).write_bytes(0, 4096);
                }
            }

            let buf = slice::from_raw_parts_mut(
                mem_reg.start as *mut u8,
                segment.in_file_size as usize,
            );
            syscall::seek(syscall::Seek::Abs, fd, segment.in_file_at).unwrap();
            syscall::read(fd, buf).unwrap();
        }

        println!(
            "[TASK] Program entry point is at 0x{:08X}.",
            elf.entry_point,
        );

        elf
    }

    /// Clones the task.
    ///
    /// What is cloned:
    /// * virtual address space layout (physical memory is copied),
    /// * program segments,
    /// * memory mappings,
    /// * usermode stack,
    /// * opened files.
    ///
    /// What is not cloned:
    /// * task ID,
    /// * guard pages,
    /// * kernel stack,
    /// * thread local storage pointer.
    ///
    /// # Safety
    /// See [`Task::with_filled_stack()`].
    pub fn clone(
        &self,
        clone_id: usize,
        entry: u32,
        entry_args: &[u32],
    ) -> Self {
        print!("[TASK] Copying VAS...");
        let vas = unsafe { self.vas.copy() };
        println!("done");

        let mut clone =
            Self::with_filled_stack(clone_id, vas, entry, entry_args);
        clone.mem_mappings = self.mem_mappings.clone();
        clone
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
            if self.opened_files.len() == MAX_OPENED_FILES {
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

#[derive(Clone)]
pub struct OpenedFile {
    pub node: fs::Node,
    offset: Option<usize>,
}

impl OpenedFile {
    fn new(node: fs::Node, seekable: bool) -> Self {
        OpenedFile {
            node,
            offset: if seekable { Some(0) } else { None },
        }
    }

    pub fn seek_abs(&mut self, new_offset: usize) -> usize {
        if let Some(offset) = self.offset.as_mut() {
            *offset = new_offset;
            return *offset;
        } else {
            // FIXME: error 'not seekable'.
            return 0;
        }
    }

    pub fn seek_rel(&mut self, add_offset: usize) -> usize {
        if let Some(offset) = self.offset.as_mut() {
            *offset += add_offset;
            return *offset;
        } else {
            // FIXME: error 'not seekable'.
            return 0;
        }
    }

    pub fn read(&mut self, buf: &mut [u8]) -> Result<usize, fs::ReadFileErr> {
        let fs = self.node.fs();
        let id_in_fs = self.node.0.borrow().id_in_fs.unwrap();
        let n = fs.read_file(id_in_fs, self.offset.unwrap_or(0), buf)?;
        self.seek_rel(n);
        Ok(n)
    }

    pub fn write(&mut self, buf: &[u8]) -> usize {
        let fs = self.node.fs();
        let id_in_fs = self.node.0.borrow().id_in_fs.unwrap();
        fs.write_file(id_in_fs, self.offset.unwrap_or(0), buf)
            .unwrap();
        self.seek_rel(buf.len());
        buf.len()
    }
}

impl Feeder for OpenedFile {
    fn get_len(&mut self, offset: usize, len: usize) -> Box<[u8]> {
        let mut buf = vec![0u8; len].into_boxed_slice();
        self.seek_abs(offset);
        self.read(&mut buf).unwrap();
        buf
    }

    fn get_until(&mut self, offset: usize, cond: fn(&u8) -> bool) -> Box<[u8]> {
        let mut buf = vec![0u8; 64]; // FIXME: len
        let mut i = 0;
        loop {
            buf.resize(buf.len() + 1, 0); // FIXME: +1

            self.seek_abs(offset + i);
            self.read(&mut buf).unwrap();

            if let Some(true_at) = buf[i..].iter().position(cond) {
                return buf.drain(0..true_at).collect();
            } else {
                i = buf.len();
            }
        }
    }
}
