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

impl Process {
    // PROT_READ, PROT_WRITE, MAP_ANONYMOUS, MAP_PRIVATE
    pub fn mem_map(&mut self, len: usize) -> &MemMapping {
        assert_eq!(len % 4096, 0, "len must be page-aligned");
        let mut start = self.program_region.start;
        let mut last = start;
        loop {
            let reg = Region { start, end: last };
            assert!(start < self.program_region.end);
            if last - start == len {
                break;
            } else if last - start > len {
                unreachable!();
            }
            if self.usermode_stack.conflicts_with(reg) {
                start = self.usermode_stack.end;
                last = self.usermode_stack.end;
            }
            for segment in &self.program_segments {
                if segment.conflicts_with(reg) {
                    start = (segment.end + 0xFFF) & !0xFFF;
                    last = (segment.end + 0xFFF) & !0xFFF;
                }
            }
            for mapping in &self.mem_mappings {
                if mapping.region.conflicts_with(reg) {
                    start = (mapping.region.end + 0xFFF) & !0xFFF;
                    last = (mapping.region.end + 0xFFF) & !0xFFF;
                }
            }
            last += 4096;
        }

        self.mem_mappings.push(MemMapping {
            region: Region { start, end: last },
        });
        let mapping = self.mem_mappings.last().unwrap();
        println!("mapping: {:?}", mapping.region);

        let whole = Region {
            start: mapping.region.start & !0x3FFFFF,
            end: (mapping.region.end + 0x400000) & !0x3FFFFF,
        };
        for aligned_at_4mib in whole.range().step_by(4 * 1024 * 1024) {
            unsafe {
                let pgtbl_virt = self.vas.pgtbl_virt_of(aligned_at_4mib as u32);
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
                    self.vas.virt_to_phys(virt_page as u32).is_none(),
                    "page 0x{:08X} is already mapped to {:#X?}",
                    virt_page,
                    self.vas.virt_to_phys(virt_page as u32).unwrap(),
                );

                let phys_page = PMM_STACK.lock().pop_page();
                self.vas.map_page(virt_page as u32, phys_page);
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
    pub region: Region<usize>,
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
        // let this_thread = SCHEDULER.running_thread();

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

        assert!(this_process.program_region.start.trailing_zeros() >= 22);
        assert!(this_process.program_region.end.trailing_zeros() >= 22);

        print!("[PROC] Checking if the program region is unmapped... ");
        // for program_page in this_process.program_region.range().step_by(4096) {
        //     assert!(
        //         this_process.vas.pgtbl_virt_of(program_page).is_null(),
        //         "program region must be unmapped on a process start up",
        //     );
        // }
        // println!("done.");
        println!("skipped.");

        for seg in elf.program_segments {
            let mem_reg =
                Region::from_start_len(seg.in_mem_at, seg.in_mem_size);

            // FIXME: make everything usize.
            this_process.program_segments.push(Region {
                start: mem_reg.start as usize,
                end: mem_reg.end as usize,
            });

            if seg._type != ProgSegmentType::Load {
                continue;
            }

            assert_eq!(
                mem_reg.overlapping_with(this_process.program_region),
                OverlappingWith::IsIn,
            );
            assert!(!mem_reg.conflicts_with(this_process.usermode_stack));

            if this_process
                .vas
                .pgtbl_virt_of(mem_reg.start as u32)
                .is_null()
            {
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

            let mem_reg_pages = Region {
                start: mem_reg.start & !0xFFF,
                end: (mem_reg.end + 0xFFF) & !0xFFF,
            };
            for virt_page in mem_reg_pages.range().step_by(4096) {
                print!("[PROC] Page 0x{:08X}", virt_page);
                if this_process.vas.virt_to_phys(virt_page as u32).is_none() {
                    let phys = PMM_STACK.lock().pop_page();
                    this_process.vas.map_page(virt_page as u32, phys);
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

        assert_eq!(this_process.usermode_stack.start % 4096, 0);
        assert_eq!(this_process.usermode_stack.end % 4096, 0);
        assert!(this_process.usermode_stack.len() <= 4 * 1024 * 1024);

        let pde_idx = (this_process.usermode_stack.start >> 22) as usize;
        let pgtbl_virt =
            alloc(Layout::from_size_align(4096, 4096).unwrap()) as *mut Table;
        pgtbl_virt.write_bytes(0, 1);
        this_process.vas.set_pde_addr(pde_idx, pgtbl_virt);
        println!(
            "[PROC] Allocated a page table for a usermode stack at {:?}.",
            this_process.usermode_stack,
        );

        assert_eq!(this_process.usermode_stack.len(), 4096);
        let phys = PMM_STACK.lock().pop_page();
        this_process
            .vas
            .map_page(this_process.usermode_stack.start as u32, phys);
        (this_process.usermode_stack.start as *mut u8).write_bytes(0, 4096);
        println!(
            "[PROC] Page 0x{:08X} has been mapped to 0x{:08X}.",
            this_process.usermode_stack.start, phys,
        );

        let usermode_stack_top =
            (this_process.usermode_stack.end as *mut u32).wrapping_sub(3);
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
