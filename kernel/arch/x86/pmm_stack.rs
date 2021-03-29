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

use crate::kernel_static::{Mutex, MutexWrapper};
use crate::memory_region::OverlappingWith;
use crate::KERNEL_INFO;

extern "C" {
    // see the linker.ld script
    static mut pmm_stack_bottom: u32;
    static mut pmm_stack_top: u32;
}

pub struct PmmStack {
    top: *mut u32,
    pointer: *mut u32,
    bottom: *mut u32,
}

impl PmmStack {
    fn new(bottom: *mut u32, top: *mut u32) -> Self {
        PmmStack {
            top,
            pointer: top,
            bottom,
        }
    }

    unsafe fn fill(&mut self) {
        for region in KERNEL_INFO.available_memory_regions.iter() {
            let mut region = region.clone();
            if region.start == 0 && region.end == 0 {
                // End of slice.
                break;
            }
            match region
                .overlapping_with(KERNEL_INFO.arch_init_info.kernel_region)
            {
                OverlappingWith::Covers => {
                    unimplemented!("a free region covers the kernel");
                }
                OverlappingWith::StartsIn => {
                    region.start = KERNEL_INFO.arch_init_info.kernel_region.end;
                }
                OverlappingWith::IsIn => {
                    continue;
                }
                OverlappingWith::EndsIn => {
                    region.end = KERNEL_INFO.arch_init_info.kernel_region.start;
                }
                OverlappingWith::NoOverlap => {}
            }
            region.start = (region.start + 0xFFF) & !0xFFF;
            region.end &= !0xFFF;
            if region.start >= region.end {
                // The region is too small.
                continue;
            }
            for page_addr in (region.start..region.end).step_by(4096) {
                self.push_page(page_addr as u32);
            }
        }
    }

    fn push_page(&mut self, addr: u32) {
        assert!(
            self.bottom <= self.pointer && self.pointer <= self.top,
            "stack pointer is outside the stack",
        );
        assert!(self.pointer > self.bottom, "push: stack bottom reached");
        unsafe {
            self.pointer = self.pointer.sub(1);
            *self.pointer = addr;
        }
    }

    pub fn pop_page(&mut self) -> u32 {
        assert!(
            self.bottom <= self.pointer && self.pointer <= self.top,
            "stack pointer is outside the stack",
        );
        assert!(self.pointer < self.top, "pop: stack top reached");
        unsafe {
            let addr = *self.pointer;
            self.pointer = self.pointer.add(1);
            addr
        }
    }
}

kernel_static! {
    pub static ref PMM_STACK: Mutex<PmmStack> = Mutex::new({
        let stack_bottom_addr = unsafe { &mut pmm_stack_bottom as *mut u32 };
        let stack_top_addr = unsafe { &mut pmm_stack_top as *mut u32 };
        PmmStack::new(stack_bottom_addr, stack_top_addr)
    });
}

pub fn init() {
    let mut stack: MutexWrapper<PmmStack> = PMM_STACK.lock();
    unsafe {
        stack.fill();
    }
    let num_entries = (stack.top as u32 - stack.pointer as u32) / 4;
    println!(
        "[PMM] Stack: top: 0x{:08X}, ptr: 0x{:08X}, bottom: 0x{:08X}, \
         {} entries, free memory: {:.1} MiB",
        stack.top as u32,
        stack.pointer as u32,
        stack.bottom as u32,
        num_entries,
        num_entries as f64 * 4096.0 / 1024.0 / 1024.0,
    );
}
