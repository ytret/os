#![no_std]

#[cfg_attr(target_arch = "x86", path = "arch/x86/mod.rs")]
#[cfg_attr(target_arch = "x86_64", path = "arch/x86-64/mod.rs")]
mod arch;

mod vga;

use core::panic::PanicInfo;

#[no_mangle]
pub extern "C" fn main() -> ! {
    arch::init();
    vga::init();
    for i in 0..25 {
        println!("Hello, world! {}", i);
    }
    loop {}
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    println!("{}", info);
    loop {}
}
