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
use crate::dev::console::CONSOLE;
use crate::fs::VFS_ROOT;

pub use crate::arch::process::default_entry_point;
use crate::arch::process::MemMapping;
use crate::arch::vas::{Table, VirtAddrSpace};
use crate::elf::{ElfObj, ProgSegmentType};
use crate::feeder::Feeder;
use crate::fs;
use crate::memory_region::{OverlappingWith, Region};
use crate::syscall;

pub const MAX_OPENED_FILES: i32 = 32;

pub struct Process {
    pub id: usize,
    new_thread_id: usize,

    pub vas: VirtAddrSpace,
    pub program_region: Region<usize>,
    pub program_segments: Vec<Region<usize>>,
    pub usermode_stack: Region<usize>,
    pub mem_mappings: Vec<MemMapping>,

    opened_files: Vec<OpenedFile>,
}

impl Process {
    pub fn new(id: usize, vas: VirtAddrSpace) -> Self {
        let mut process = Process {
            id,
            new_thread_id: 0,

            vas,
            program_region: Region {
                start: 128 * 1024 * 1024,                      // 128 MiB
                end: 3 * 1024 * 1024 * 1024 + 4 * 1024 * 1024, // 3 GiB + 4 MiB
            },
            program_segments: Vec::new(),
            usermode_stack: Region {
                start: 3 * 1024 * 1024 * 1024,      // 3 GiB
                end: 3 * 1024 * 1024 * 1024 + 4096, // 3 GiB + 4 KiB
            },
            mem_mappings: Vec::new(),

            opened_files: Vec::new(),
        };

        assert!(CONSOLE.lock().is_some());
        let stdin =
            VFS_ROOT.lock().as_mut().unwrap().path("/dev/chr0").unwrap();
        let stdout =
            VFS_ROOT.lock().as_mut().unwrap().path("/dev/chr0").unwrap();
        let stderr =
            VFS_ROOT.lock().as_mut().unwrap().path("/dev/chr0").unwrap();
        assert_eq!(process.open_file_by_node(stdin).unwrap(), 0);
        assert_eq!(process.open_file_by_node(stdout).unwrap(), 1);
        assert_eq!(process.open_file_by_node(stderr).unwrap(), 2);

        process
    }

    pub fn allocate_thread_id(&mut self) -> usize {
        let id = self.new_thread_id;
        self.new_thread_id += 1;
        id
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

    /// Reads loadable segments into memory from an ELF executable.
    pub unsafe fn load_from_file(&mut self, pathname: &str) -> ElfObj {
        // FIXME: no syscalls here

        let fd = syscall::open(pathname).unwrap();
        let elf = ElfObj::from(self.opened_file(fd)).unwrap();
        println!("[PROC] {:#X?}", elf);

        assert!(self.program_region.start.trailing_zeros() >= 22);
        assert!(self.program_region.end.trailing_zeros() >= 22);

        for seg in &elf.program_segments {
            let mem_reg =
                Region::from_start_len(seg.in_mem_at, seg.in_mem_size);

            // FIXME: make everything usize
            self.program_segments.push(Region {
                start: mem_reg.start as usize,
                end: mem_reg.end as usize,
            });

            if seg._type != ProgSegmentType::Load {
                continue;
            }

            assert_eq!(
                mem_reg.overlapping_with(self.program_region),
                OverlappingWith::IsIn,
            );
            assert!(!mem_reg.conflicts_with(self.usermode_stack));

            if self.vas.pgtbl_virt_of(mem_reg.start as u32).is_null() {
                let pde_idx = (mem_reg.start >> 22) as usize;
                let pgtbl_virt =
                    alloc(Layout::from_size_align(4096, 4096).unwrap())
                        as *mut Table;
                pgtbl_virt.write_bytes(0, 1);
                self.vas.set_pde_virt(pde_idx, pgtbl_virt);
                println!(
                    "[PROC] Allocated a page table for region {:?}.",
                    mem_reg,
                );
            } else {
                println!(
                    "[PROC] Page table for region {:?} is already allocated.",
                    mem_reg,
                );
            }

            let mem_reg_pages = Region {
                start: mem_reg.start & !0xFFF,
                end: (mem_reg.end + 0xFFF) & !0xFFF,
            };
            for virt_page in mem_reg_pages.range().step_by(4096) {
                print!("[PROC] Page 0x{:08X}", virt_page);
                if self.vas.virt_to_phys(virt_page as u32).is_none() {
                    let phys = PMM_STACK.lock().pop_page();
                    self.vas.map_page(virt_page as u32, phys);
                    (virt_page as *mut u8).write_bytes(0, 4096);
                    println!(" has been mapped to 0x{:08X}.", phys);
                } else {
                    println!(" has been mapped already.");
                }
            }

            let buf = slice::from_raw_parts_mut(
                mem_reg.start as *mut u8,
                seg.in_file_size as usize,
            );
            syscall::seek(syscall::Seek::Abs, fd, seg.in_file_at).unwrap();
            syscall::read(fd, buf).unwrap();
        }

        println!(
            "[RPOC] Program entry point is at 0x{:08X}.",
            elf.entry_point,
        );

        elf
    }
}

#[derive(Debug)]
pub enum OpenFileErr {
    MaxOpenedFiles,
    UnsupportedFileType,
}

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
