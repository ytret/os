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

use crate::arch::pmm_stack::PMM_STACK;
use crate::scheduler::SCHEDULER;

use crate::arch::gdt;
use crate::arch::vas::Table;
use crate::cstring::CString;
use crate::memory_region::Region;
use crate::process::Process;

extern "C" {
    fn jump_into_usermode(
        code_seg: u16,
        data_seg: u16,
        tls_seg: u16,
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

    pub unsafe fn set_up_usermode_stack(
        &mut self,
        argv: &[CString],
        environ: &[CString],
    ) -> *mut u32 {
        assert_eq!(self.usermode_stack.start % 4096, 0);
        assert_eq!(self.usermode_stack.end % 4096, 0);
        assert!(self.usermode_stack.len() <= 4 * 1024 * 1024);

        let pde_idx = (self.usermode_stack.start >> 22) as usize;
        let pgtbl_virt =
            alloc(Layout::from_size_align(4096, 4096).unwrap()) as *mut Table;
        pgtbl_virt.write_bytes(0, 1);
        self.vas.set_pde_addr(pde_idx, pgtbl_virt);
        println!(
            "[PROC] Allocated a page table for a usermode stack at {:?}.",
            self.usermode_stack,
        );

        assert_eq!(self.usermode_stack.len(), 4096);
        let phys = PMM_STACK.lock().pop_page();
        self.vas.map_page(self.usermode_stack.start as u32, phys);
        (self.usermode_stack.start as *mut u8).write_bytes(0, 4096);
        println!(
            "[PROC] Page 0x{:08X} has been mapped to 0x{:08X}.",
            self.usermode_stack.start, phys,
        );

        // Length of the initial stack in 32-bit units.
        // Init stack = argc + argv + NULL + environ + NULL.
        let init_stack_len = 1 + argv.len() + 1 + environ.len() + 1;

        let usermode_stack_top =
            (self.usermode_stack.end as *mut u32).wrapping_sub(init_stack_len);
        let mut offset = 0;

        // FIXME: copy the strings into usermode memory?

        // argc
        *usermode_stack_top.wrapping_add(offset) = argv.len() as u32;

        // argv
        for (i, arg) in argv.iter().enumerate() {
            offset += 1;
            *usermode_stack_top.wrapping_add(offset) = argv[i].as_ptr() as u32;
        }
        offset += 1;
        *usermode_stack_top.wrapping_add(offset) = 0;

        // environ
        for (j, env) in environ.iter().enumerate() {
            offset += 1;
            *usermode_stack_top.wrapping_add(offset) =
                environ[j].as_ptr() as u32;
        }
        offset += 1;
        *usermode_stack_top.wrapping_add(offset) = 0;

        return usermode_stack_top;
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
        let mut this_process = SCHEDULER.running_process();
        // let this_thread = SCHEDULER.running_thread();

        let argv = vec![CString::new("/bin/test-arg-env").unwrap()];
        let environ = Vec::new();

        let elf = this_process.load_from_file("/bin/test-arg-env");
        let usermode_stack_top =
            this_process.set_up_usermode_stack(&argv, &environ);

        SCHEDULER.keep_scheduling();

        println!("[PROC] Entering usermode.");
        jump_into_usermode(
            gdt::USERMODE_CODE_SEG,
            gdt::USERMODE_DATA_SEG,
            gdt::TLS_SEG,
            elf.entry_point as u32,
            usermode_stack_top as u32,
        );
    }
}
