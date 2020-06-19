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

#![no_std]
#![cfg_attr(target_arch = "x86", feature(abi_x86_interrupt))]
#![cfg_attr(target_arch = "x86", feature(asm))]

use core::panic::PanicInfo;

#[macro_use]
mod bitflags;

#[macro_use]
mod kernel_static;

#[macro_use]
mod vga;

#[cfg_attr(target_arch = "x86", path = "arch/x86/mod.rs")]
mod arch;

mod mbi;

pub struct KernelInfo {
    arch_init_info: ArchInitInfo,
    available_memory_regions: [(u64, u64); 32], // 32 is enough maybe
}

pub struct ArchInitInfo {
    kernel_start: u64,
    kernel_end: u64,
}

impl KernelInfo {
    fn new() -> Self {
        KernelInfo {
            arch_init_info: ArchInitInfo {
                kernel_start: 0,
                kernel_end: 0,
            },
            available_memory_regions: [(0, 0); 32],
        }
    }
}

#[no_mangle]
pub extern "C" fn main(magic_num: u32, boot_info: *const mbi::BootInfo) {
    let mut kernel_info = KernelInfo::new();
    vga::init();

    if magic_num == 0x36D76289 {
        println!("Booted by Multiboot2-compatible bootloader");
        unsafe {
            mbi::parse(boot_info, &mut kernel_info);
        }
    } else {
        panic!("Booted by unknown bootloader.");
    }

    arch::init(&mut kernel_info);
    let kernel_size = kernel_info.arch_init_info.kernel_end
        - kernel_info.arch_init_info.kernel_start;
    println!(
        "Kernel size: {} KiB ({} pages)",
        kernel_size / 1024,
        kernel_size / 4096,
    );

    loop {}
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    println!("{}", info);
    arch::panic();
    loop {}
}
