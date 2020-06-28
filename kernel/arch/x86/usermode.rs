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

use crate::arch::gdt;
use crate::bitflags::BitFlags;

use alloc::alloc::{alloc, Layout};
use core::mem::size_of;

extern "C" {
    fn jump_into_usermode(
        code_seg: u16,
        data_seg: u16,
        jump_to: InitUsermodeFunc,
    ) -> !;
}

type InitUsermodeFunc = extern "C" fn() -> !;

#[allow(dead_code)]
#[derive(Default)]
#[repr(C, packed)]
struct TaskStateSegment {
    link: u16,
    _reserved_link: u16,
    esp0: u32,
    ss0: u16,
    _reserved_ss0: u16,
    esp1: u32,
    ss1: u16,
    _reserved_ss1: u16,
    esp2: u16,
    ss2: u16,
    _reserved_ss2: u16,
    cr3: u32,
    eip: u32,
    eflags: u32,
    eax: u32,
    ecx: u32,
    edx: u32,
    ebx: u32,
    esp: u32,
    ebp: u32,
    esi: u32,
    edi: u32,
    es: u16,
    _reserved_es: u16,
    cs: u16,
    _reserved_cs: u16,
    ss: u16,
    _reserved_ss: u16,
    ds: u16,
    _reserved_ds: u16,
    fs: u16,
    _reserved_fs: u16,
    gs: u16,
    _reserved_gs: u16,
    ldtr: u16,
    _reserved_ldtr: u16,
    _reserved_iopb_offset: u16,
    iobp_offset: u16,
}

impl TaskStateSegment {
    fn new(ss0: u16, esp0: u32) -> Self {
        let mut tss = TaskStateSegment::default();
        tss.ss0 = ss0;
        tss.esp0 = esp0;
        tss.iobp_offset = size_of::<Self>() as u16;
        tss
    }
}

static KERNEL_STACK: [u32; 1024] = [0; 1024];

pub fn init() -> ! {
    // Update the GDT.
    let entry_for_usermode_code = gdt::Entry::new(
        0x0000_0000,
        0xFFFFF,
        (BitFlags::new(0)
            | gdt::AccessByte::Present
            | gdt::AccessByte::Usermode
            | gdt::AccessByte::NotTaskStateSegment
            | gdt::AccessByte::Executable
            | gdt::AccessByte::ReadableWritable)
            .value,
        (BitFlags::new(0)
            | gdt::EntryFlags::ProtectedMode32Bit
            | gdt::EntryFlags::PageGranularity)
            .value,
    );

    let entry_for_usermode_data = gdt::Entry::new(
        0x0000_0000,
        0xFFFFF,
        (BitFlags::new(0)
            | gdt::AccessByte::Present
            | gdt::AccessByte::Usermode
            | gdt::AccessByte::NotTaskStateSegment
            | gdt::AccessByte::ReadableWritable)
            .value,
        (BitFlags::new(0)
            | gdt::EntryFlags::ProtectedMode32Bit
            | gdt::EntryFlags::PageGranularity)
            .value,
    );

    let tss_ptr;
    unsafe {
        tss_ptr =
            alloc(Layout::new::<TaskStateSegment>()) as *mut TaskStateSegment;
        *tss_ptr = TaskStateSegment::new(
            gdt::GDT.lock().kernel_data_segment(),
            &KERNEL_STACK[1023] as *const _ as u32,
        );
    };
    let entry_for_tss = gdt::Entry::new(
        tss_ptr as u32,
        size_of::<TaskStateSegment>() as u32,
        (BitFlags::new(0)
            | gdt::AccessByte::Present
            | gdt::AccessByte::Executable
            | gdt::AccessByte::Accessed)
            .value,
        (BitFlags::new(0) | gdt::EntryFlags::PageGranularity).value,
    );

    let usermode_code_seg =
        gdt::GDT.lock().add_segment(entry_for_usermode_code);
    let usermode_data_seg =
        gdt::GDT.lock().add_segment(entry_for_usermode_data);
    let tss_seg = gdt::GDT.lock().add_segment(entry_for_tss);

    unsafe {
        // Load the GDT with the new entries.
        gdt::GDT.lock().load();

        // Load the TSS.
        asm!("ltr %ax", in("ax") tss_seg, options(att_syntax));

        // Jump into usermode.
        jump_into_usermode(usermode_code_seg, usermode_data_seg, usermode_init);
    }
}

extern "C" fn usermode_init() -> ! {
    println!("Hello from usermode!");
    unsafe {
        // Cause a General protection fault.
        asm!("cli");
    }
    loop {}
}
