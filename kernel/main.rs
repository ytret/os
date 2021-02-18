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
#![feature(alloc_error_handler)]
#![cfg_attr(target_arch = "x86", feature(asm))]

#[macro_use]
extern crate alloc;

#[macro_use]
mod bitflags;

#[macro_use]
mod kernel_static;

pub mod port;

#[macro_use]
mod vga;

#[cfg_attr(target_arch = "x86", path = "arch/x86/mod.rs")]
pub mod arch;

mod heap;
mod mbi;
mod memory_region;

mod scheduler;

pub mod disk;

pub mod fs;

pub mod elf;

use core::panic::PanicInfo;
use memory_region::Region;

pub struct KernelInfo {
    arch_init_info: arch::ArchInitInfo,
    available_memory_regions: [Region<u64>; 32], // 32 is enough maybe
}

impl KernelInfo {
    fn new() -> Self {
        KernelInfo {
            arch_init_info: arch::ArchInitInfo::new(),
            available_memory_regions: [Region { start: 0, end: 0 }; 32],
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

    heap::init(&kernel_info);

    arch::pci::init();

    scheduler::init();

    /*
    let data: Box<[u8]> = Box::new([
        0x7f, 101, 108, 102, 1, 1, 1, 7, 8, 9,
        2, 0,
        1, 0,
        2, 0, 0, 0,
        3, 0, 0, 0,
        4, 0, 0, 0,
        5, 0, 0, 0,
        6, 0, 0, 0,
        7, 0,
        8, 0,
        9, 0,
        10, 0,
        11, 0,
        12, 0,
    ]);
    elf::read_elf_header(data).unwrap();
    */

    /*
    use alloc::boxed::Box;
    let disk = &mut disk::DISKS.lock()[0];
    let superblock: Box<[u8]> = disk.rw_interface.read_sectors(2, 2).unwrap();
    let bgd_tbl: Box<[u8]> = disk.rw_interface.read_sector(4).unwrap();
    // FIXME: The BGD Table is not always at ext2-block 2.
    // 0: 0-511
    // 1: 512-1023
    // 2: 1024-1535
    // 3: 1536-2047
    // 4: 2048-2559
    // 5: 2560-3071
    // 6: 3072-3583
    disk.file_system = Some(Box::new(unsafe {
        fs::ext2::Ext2::from_raw(&superblock, &bgd_tbl)
    }));
    match &disk.file_system {
        Some(fs) => {
            println!("{:#?}", fs.root_dir(&disk.rw_interface).unwrap());
            let data = fs.read_file(15, &disk.rw_interface).unwrap();
            println!("{:?}", elf::ElfInfo::from_raw_data(&data[0]));
        }
        None => panic!(),
    }
    */

    loop {}

    // println!("Reached the end of main.");
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    println!("{}", info);
    arch::panic();
    loop {}
}
