#![no_std]
#![cfg_attr(target_arch = "x86", feature(abi_x86_interrupt))]
#![cfg_attr(target_arch = "x86", feature(asm))]

#[macro_use]
mod kernel_static;

#[macro_use]
mod vga;

#[cfg_attr(target_arch = "x86", path = "arch/x86/mod.rs")]
mod arch;

use core::panic::PanicInfo;

pub struct ArchInitInfo {
    kernel_size: u32,
}

#[no_mangle]
pub extern "C" fn main() -> ! {
    vga::init();
    let aif: ArchInitInfo = arch::init();
    println!(
        "Kernel size: {} KiB ({} pages)",
        aif.kernel_size,
        aif.kernel_size / 4,
    );

    loop {}
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    println!("{}", info);
    loop {}
}
