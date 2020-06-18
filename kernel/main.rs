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

pub struct ArchInitInfo {
    kernel_size: u32,
}

#[no_mangle]
pub extern "C" fn main() {
    vga::init();
    let aif: ArchInitInfo = arch::init();
    println!(
        "Kernel size: {} KiB ({} pages)",
        aif.kernel_size / 1024,
        aif.kernel_size / 4096,
    );

    loop {}
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    println!("{}", info);
    arch::panic();
    loop {}
}
