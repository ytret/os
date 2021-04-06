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
use alloc::vec::Vec;
use core::slice;

use crate::arch::pmm_stack::PMM_STACK;
use crate::scheduler::SCHEDULER;

use crate::arch::gdt;
use crate::arch::vas::Table;
use crate::elf::{ElfObj, ProgSegmentType};
use crate::memory_region::{OverlappingWith, Region};
use crate::process::Process;
use crate::syscall;

extern "C" {
    fn jump_into_usermode(
        code_seg: u16,
        data_seg: u16,
        jump_to: u32,
        esp: u32,
    ) -> !;
}

/// Region of program's virtual memory intended to be used by the program itself
/// and not the kernel.
///
/// Its start and end must be aligned at 4 MiB.
pub const PROGRAM_REGION: Region<u32> = Region {
    start: 128 * 1024 * 1024,                      // 128 MiB
    end: 3 * 1024 * 1024 * 1024 + 4 * 1024 * 1024, // 3 GiB + 4 MiB
};

pub const USERMODE_STACK: Region<u32> = Region {
    start: 3 * 1024 * 1024 * 1024,      // 3 GiB
    end: 3 * 1024 * 1024 * 1024 + 4096, // 3 GiB + 4 KiB
};

pub const ARGV_ENVIRON: Region<u32> = Region {
    start: 3 * 1024 * 1024 * 1024 + 1 * 4096, // 3 GiB + 4 KiB
    end: 3 * 1024 * 1024 * 1024 + 2 * 4096,   // 3 GiB + 8 KiB
};

// NOTE: The above three regions must have page-aligned start and end.

impl Process {
    // PROT_READ, PROT_WRITE, MAP_ANONYMOUS, MAP_PRIVATE
    pub fn mem_map(&mut self, len: usize) -> &MemMapping {
        assert_eq!(len % 4096, 0, "len must be page-aligned");
        let mut start = PROGRAM_REGION.start;
        let mut last = start;
        loop {
            assert!(start < PROGRAM_REGION.end);
            if last - start == len as u32 {
                break;
            } else if last - start > len as u32 {
                unreachable!();
            }
            for mapping in &self.mem_mappings {
                if mapping.region.contains(&last) {
                    start = (mapping.region.end + 0xFFF) & !0xFFF;
                    last = (mapping.region.end + 0xFFF) & !0xFFF;
                }
            }
            for segment in &self.program_segments {
                if segment.contains(&(last as usize)) {
                    start = ((segment.end as u32) + 0xFFF) & !0xFFF;
                    last = ((segment.end as u32) + 0xFFF) & !0xFFF;
                }
            }
            if USERMODE_STACK.contains(&last as &u32) {
                start = USERMODE_STACK.end;
                last = USERMODE_STACK.end;
            }
            if ARGV_ENVIRON.contains(&last as &u32) {
                start = ARGV_ENVIRON.end;
                last = ARGV_ENVIRON.end;
            }
            last += 4096;
        }

        self.mem_mappings.push(MemMapping {
            region: Region { start, end: last },
        });
        let mapping = self.mem_mappings.last().unwrap();

        let whole = Region {
            start: mapping.region.start & !0x3FFFFF,
            end: (mapping.region.end + 0x400000) & !0x3FFFFF,
        };
        for aligned_at_4mib in whole.range().step_by(4 * 1024 * 1024) {
            unsafe {
                let pgtbl_virt = self.vas.pgtbl_virt_of(aligned_at_4mib);
                if pgtbl_virt.is_null() {
                    let pde_idx = (aligned_at_4mib >> 22) as usize;
                    let new_pgtbl_virt =
                        alloc(Layout::from_size_align(4096, 4096).unwrap())
                            as *mut Table;
                    new_pgtbl_virt.write_bytes(0, 1);
                    self.vas.set_pde_addr(pde_idx, new_pgtbl_virt);
                    println!(
                        "[PROC MEM_MAP] Allocated a page table for 0x{:08X}..0x{:08X}.",
                        aligned_at_4mib,
                        aligned_at_4mib + 0x400000,
                    );
                } else {
                    // println!(
                    //     "[PROC MEM_MAP] There is a page table for 0x{:08X}..0x{:08X}.",
                    //     aligned_at_4mib,
                    //     aligned_at_4mib + 0x400000,
                    // );
                }
            }
        }

        for virt_page in mapping.region.range().step_by(4096) {
            unsafe {
                assert!(
                    self.vas.virt_to_phys(virt_page).is_none(),
                    "page 0x{:08X} is already mapped to {:#X?}",
                    virt_page,
                    self.vas.virt_to_phys(virt_page).unwrap(),
                );

                let phys_page = PMM_STACK.lock().pop_page();
                self.vas.map_page(virt_page, phys_page);
                // println!(
                //     "[PROC MEM_MAP] Page 0x{:08X} has been mapped to 0x{:08X}.",
                //     virt_page, phys_page,
                // );

                let raw_ptr = virt_page as *mut u8;
                raw_ptr.write_bytes(0, 4096);
            }
        }

        println!("[PROC MEM_MAP] New memory mapping at {:?}.", mapping.region);

        mapping
    }
}

pub struct MemMapping {
    pub region: Region<u32>,
}

pub fn default_entry_point() -> ! {
    // This function must always be a result of ret from switch_threads (see
    // scheduler.s) which requires that interrupts be enabled after it returns
    // so that task switching remains possible.
    unsafe {
        asm!("sti");
    }

    println!("[PROC] Default process entry. Starting initialization.");

    unsafe {
        SCHEDULER.stop_scheduling();
        let this_process = SCHEDULER.running_process();

        // println!("[PROC] Loading the VAS.");
        // this_process.vas.load();
        // SCHEDULER.running_thread().tcb.cr3 = this_process.vas.pgdir_phys;

        let fd = syscall::open("/bin/test-hello-world").unwrap();
        let elf = ElfObj::from_feeder(|offset, len| {
            let buf_len = match len {
                0 => 64,
                other => other,
            };
            let mut pre_buf = Vec::with_capacity(buf_len);
            for _ in 0..pre_buf.capacity() {
                pre_buf.push(0);
            }
            let mut buf = pre_buf.into_boxed_slice();
            syscall::seek(syscall::Seek::Abs, fd, offset).unwrap();
            if len == 0 {
                syscall::read(fd, &mut buf).unwrap();
                let null_at = buf.iter().position(|&x| x == 0).unwrap();
                buf.into_vec().drain(0..null_at).collect()
            } else {
                syscall::read(fd, &mut buf).unwrap();
                buf
            }
        })
        .unwrap();
        println!("[PROC] {:#X?}", elf);

        assert!(PROGRAM_REGION.start.trailing_zeros() >= 22);
        assert!(PROGRAM_REGION.end.trailing_zeros() >= 22);

        print!("[PROC] Checking if the program region is unmapped... ");
        // for program_page in PROGRAM_REGION.range().step_by(4096) {
        //     assert!(
        //         this_process.vas.pgtbl_virt_of(program_page).is_null(),
        //         "program region must be unmapped on a process start up",
        //     );
        // }
        // println!("done.");
        println!("skipped.");

        for seg in elf.program_segments {
            let mem_reg = Region::from_start_len(
                seg.in_mem_at as u32,
                seg.in_mem_size as u32,
            );

            // FIXME: make everything usize.
            this_process.program_segments.push(Region {
                start: mem_reg.start as usize,
                end: mem_reg.end as usize,
            });

            if seg._type != ProgSegmentType::Load {
                continue;
            }

            assert_eq!(
                mem_reg.overlapping_with(PROGRAM_REGION),
                OverlappingWith::IsIn,
            );
            assert_eq!(
                mem_reg.overlapping_with(USERMODE_STACK),
                OverlappingWith::NoOverlap,
            );
            assert_eq!(
                mem_reg.overlapping_with(ARGV_ENVIRON),
                OverlappingWith::NoOverlap,
            );

            if this_process.vas.pgtbl_virt_of(mem_reg.start).is_null() {
                let pde_idx = (mem_reg.start >> 22) as usize;
                let pgtbl_virt =
                    alloc(Layout::from_size_align(4096, 4096).unwrap())
                        as *mut Table;
                pgtbl_virt.write_bytes(0, 1);
                this_process.vas.set_pde_addr(pde_idx, pgtbl_virt);
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

            for virt_page in mem_reg.range().step_by(4096) {
                print!("[PROC] Page 0x{:08X}", virt_page);
                if this_process.vas.virt_to_phys(virt_page).is_none() {
                    let phys = PMM_STACK.lock().pop_page();
                    this_process.vas.map_page(virt_page, phys);
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

        assert_eq!(USERMODE_STACK.start % 4096, 0);
        assert_eq!(USERMODE_STACK.end % 4096, 0);
        assert!(USERMODE_STACK.size() <= 4 * 1024 * 1024);

        let pde_idx = (USERMODE_STACK.start >> 22) as usize;
        let pgtbl_virt =
            alloc(Layout::from_size_align(4096, 4096).unwrap()) as *mut Table;
        pgtbl_virt.write_bytes(0, 1);
        this_process.vas.set_pde_addr(pde_idx, pgtbl_virt);
        println!(
            "[PROC] Allocated a page table for a usermode stack at {:?}.",
            USERMODE_STACK,
        );

        assert_eq!(USERMODE_STACK.size(), 4096);
        let mut phys = PMM_STACK.lock().pop_page();
        this_process.vas.map_page(USERMODE_STACK.start, phys);
        (USERMODE_STACK.start as *mut u8).write_bytes(0, 4096);
        println!(
            "[PROC] Page 0x{:08X} has been mapped to 0x{:08X}.",
            USERMODE_STACK.start, phys,
        );

        assert_eq!(ARGV_ENVIRON.start % 4096, 0);
        assert_eq!(ARGV_ENVIRON.end % 4096, 0);
        assert_eq!(ARGV_ENVIRON.size() % 4096, 0);
        phys = PMM_STACK.lock().pop_page();
        this_process.vas.map_page(ARGV_ENVIRON.start, phys);
        (ARGV_ENVIRON.start as *mut u8).write_bytes(0, 4096);
        println!(
            "[PROC] Page 0x{:08X} has been mapped to 0x{:08X} for ARGV_ENVIRON.",
            ARGV_ENVIRON.start, phys,
        );

        let usermode_stack_top =
            (USERMODE_STACK.end as *mut u32).wrapping_sub(3);
        *usermode_stack_top.wrapping_add(0) = 0; // argc
        *usermode_stack_top.wrapping_add(1) = 0; // argv
        *usermode_stack_top.wrapping_add(2) = 0; // environ

        SCHEDULER.keep_scheduling();

        println!("[PROC] Entering usermode.");
        jump_into_usermode(
            gdt::USERMODE_CODE_SEG,
            gdt::USERMODE_DATA_SEG,
            elf.entry_point as u32,
            usermode_stack_top as u32,
        );
    }
}
