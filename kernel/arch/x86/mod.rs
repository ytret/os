pub mod interrupts;

use crate::ArchInitInfo;

extern "C" {
    // see the linker.ld script
    static text_start: u32;
    static kernel_end: u32;
}

pub fn init() -> ArchInitInfo {
    //interrupts::init();

    let text_start_addr = unsafe { &text_start as *const _ as u32 };
    let kernel_end_addr = unsafe { &kernel_end as *const _ as u32 };
    print!("text_start = 0x{:08X}; ", text_start_addr);
    println!("kernel_end = 0x{:08X}", kernel_end_addr);

    ArchInitInfo {
        kernel_size: (kernel_end_addr - text_start_addr) / 1024,
    }
}
