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

#![no_std]
#![feature(alloc_error_handler)]
#![cfg_attr(target_arch = "x86", feature(asm))]

extern crate alloc;

#[macro_use]
pub mod bitflags;

#[macro_use]
pub mod kernel_static;

pub mod port;

#[macro_use]
pub mod vga;

pub mod timer;

#[cfg_attr(target_arch = "x86", path = "arch/x86/mod.rs")]
pub mod arch;

pub mod heap;
pub mod multiboot;
pub mod memory_region;

pub mod syscall;

pub mod process;
pub mod thread;

pub mod scheduler;

pub mod block_device;
pub mod disk;

pub mod fs;

pub mod char_device;
pub mod console;

pub mod elf;

use alloc::rc::Rc;
use core::panic::PanicInfo;

use memory_region::Region;

pub struct KernelInfo {
    arch: arch::ArchInitInfo,
    available_memory_regions: [Region<usize>; 32], // 32 is enough maybe
}

impl KernelInfo {
    const fn new() -> Self {
        KernelInfo {
            arch: arch::ArchInitInfo::new(),
            available_memory_regions: [Region { start: 0, end: 0 }; 32],
        }
    }
}

pub static mut KERNEL_INFO: KernelInfo = KernelInfo::new();

#[no_mangle]
pub extern "C" fn main(magic_num: u32, boot_info: *const multiboot::BootInfo) {
    vga::init();

    if magic_num == 0x36D76289 {
        println!("Booted by a Multiboot2-compliant bootloader.");
        unsafe {
            multiboot::parse(boot_info);
        }
    } else {
        panic!("Booted by an unknown bootloader.");
    }

    arch::init();

    unsafe {
        println!(
            "Kernel size: {} KiB ({} pages)",
            KERNEL_INFO.arch.kernel_region.size() / 1024,
            KERNEL_INFO.arch.kernel_region.size() / 4096,
        );
    }

    // FIXME
    arch::pci::init();
    arch::keyboard::init();

    console::init();

    let rc_console = Rc::clone(console::CONSOLE.lock().as_ref().unwrap());
    char_device::CHAR_DEVICES.lock().push(rc_console);

    if disk::DISKS.lock().len() > 0 {
        println!("Initializing the VFS root on disk 0.");
        fs::init_vfs_root_on_disk(0);
    }
    assert!(
        fs::VFS_ROOT.lock().is_some(),
        "VFS has not been initialized",
    );

    scheduler::init();
    // loop {}

    // println!("Reached the end of main.");
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    println!("{}", info);
    arch::panic();
    loop {}
}
