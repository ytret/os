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

use crate::scheduler::SCHEDULER;

use crate::arch::gdt;
use crate::arch::vas::VirtAddrSpace;

extern "C" {
    fn jump_into_usermode(
        code_seg: u16,
        data_seg: u16,
        jump_to: unsafe extern "C" fn() -> !,
    ) -> !;

    fn usermode_part() -> !;
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
