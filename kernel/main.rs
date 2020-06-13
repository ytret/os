#![no_std]

#[cfg_attr(target_arch = "x86", path = "arch/x86/mod.rs")]
#[cfg_attr(target_arch = "x86_64", path = "arch/x86-64/mod.rs")]
mod arch;

mod vga;

#[macro_use]
mod kernel_static;

use core::panic::PanicInfo;

kernel_static! {
    static ref SOMETHING: usize = {
        println!("SOMETHING: constructor run");
        2
    };
    static ref ANOTHER: bool = false;
}

#[no_mangle]
pub extern "C" fn main() -> ! {
    arch::init();
    vga::init();

    println!("{}", *SOMETHING);
    println!("{}", *SOMETHING);
    println!("{}", *SOMETHING);
    println!("{}", *SOMETHING);
    println!("{}", *ANOTHER);

    loop {}
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    println!("{}", info);
    loop {}
}
