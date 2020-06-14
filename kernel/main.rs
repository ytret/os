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

#[no_mangle]
pub extern "C" fn main() -> ! {
    vga::init();
    arch::init();

    unsafe {
        asm!("movl $0, %eax; div %eax", options(att_syntax));
    }

    loop {}
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    println!("{}", info);
    loop {}
}
