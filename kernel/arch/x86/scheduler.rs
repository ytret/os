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
use crate::arch::process::Process;
use crate::bitflags::BitFlags;
use crate::scheduler::SCHEDULER;

use core::mem::size_of;

extern "C" {
    fn jump_into_usermode(
        code_seg: u16,
        data_seg: u16,
        jump_to: UsermodeInitFunc,
    ) -> !;

    fn switch_tasks(
        from: *mut Process,
        to: *const Process,
        tss: *mut TaskStateSegment,
    );
}

type UsermodeInitFunc = extern "C" fn() -> !;

#[allow(dead_code)]
#[repr(C, packed)]
pub struct TaskStateSegment {
    link: u16,
    _reserved_link: u16,
    pub esp0: u32,
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
    pub const fn new() -> Self {
        TaskStateSegment {
            link: 0,
            _reserved_link: 0,
            esp0: 0,
            ss0: 0,
            _reserved_ss0: 0,
            esp1: 0,
            ss1: 0,
            _reserved_ss1: 0,
            esp2: 0,
            ss2: 0,
            _reserved_ss2: 0,
            cr3: 0,
            eip: 0,
            eflags: 0,
            eax: 0,
            ecx: 0,
            edx: 0,
            ebx: 0,
            esp: 0,
            ebp: 0,
            esi: 0,
            edi: 0,
            es: 0,
            _reserved_es: 0,
            cs: 0,
            _reserved_cs: 0,
            ss: 0,
            _reserved_ss: 0,
            ds: 0,
            _reserved_ds: 0,
            fs: 0,
            _reserved_fs: 0,
            gs: 0,
            _reserved_gs: 0,
            ldtr: 0,
            _reserved_ldtr: 0,
            _reserved_iopb_offset: 0,
            iobp_offset: size_of::<Self>() as u16,
        }
    }
}

impl crate::scheduler::Scheduler {
    pub fn switch_tasks(&self, from: *mut Process, to: *const Process) {
        // NOTE: call this method with interrupts disabled and enable them after
        // it returns.
        let tss = unsafe { &mut TSS };
        unsafe {
            switch_tasks(from, to, tss);
        }
    }
}

pub static mut TSS: TaskStateSegment = TaskStateSegment::new();

pub fn init() -> ! {
    let tss = unsafe { &mut TSS };
    tss.ss0 = gdt::GDT.lock().kernel_data_segment();

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

    let entry_for_tss = gdt::Entry::new(
        tss as *const _ as u32,
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

    // Create the init process and set up its kernel stack.
    let init_process = Process::new();
    unsafe {
        SCHEDULER.add_process(init_process);
    }
    tss.esp0 = init_process.esp0;

    unsafe {
        // Load the GDT with the new entries.
        gdt::GDT.lock().load();

        // Load the TSS.
        asm!("ltr %ax", in("ax") tss_seg, options(att_syntax));
    }

    unsafe {
        // Jump into usermode.
        jump_into_usermode(usermode_code_seg, usermode_data_seg, usermode_init);
    }
}

extern "C" fn usermode_init() -> ! {
    println!("Hello from usermode init!");
    println!("Enabling the spawner");
    crate::arch::pit::TEMP_SPAWNER_ON
        .store(true, core::sync::atomic::Ordering::SeqCst);
    loop {}
}
