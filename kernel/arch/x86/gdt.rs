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

use core::mem::size_of;

use crate::bitflags::BitFlags;
use crate::kernel_static::Mutex;

extern "C" {
    fn load_gdt(gdt_descriptor: *const GdtDescriptor);
}

bitflags! {
    #[repr(u8)]
    pub enum AccessByte {
        Accessed = 1 << 0,
        ReadableWritable = 1 << 1,
        ConformingDirection = 1 << 2,
        Executable = 1 << 3, // not set: data segment
        NotTaskStateSegment = 1 << 4,
        Usermode = 0b11 << 5,
        Present = 1 << 7,
    }
}

bitflags! {
    #[repr(u8)]
    pub enum EntryFlags {
        ProtectedMode32Bit = 1 << 6, // not set: 16-bit protected mode
        PageGranularity = 1 << 7, // not set: byte granularity
    }
}

#[repr(C, packed)]
pub struct Entry {
    limit_0_15: u16,
    base_0_15: u16,
    base_16_23: u8,
    access_byte: BitFlags<u8, AccessByte>,
    flags_limit_16_19: BitFlags<u8, EntryFlags>,
    base_24_31: u8,
}

impl Entry {
    pub fn new(base: u32, limit: u32, access_byte: u8, flags: u8) -> Self {
        assert_eq!(limit >> 20, 0, "limit must be 20 bits wide");
        Entry {
            limit_0_15: limit as u16,
            base_0_15: base as u16,
            base_16_23: (base >> 16) as u8,
            access_byte: BitFlags::new(access_byte),
            flags_limit_16_19: BitFlags::new(flags | (limit >> 16) as u8 & 0xF),
            base_24_31: (base >> 24) as u8,
        }
    }

    pub fn set_base(&mut self, new_base: u32) {
        self.base_0_15 = new_base as u16;
        self.base_16_23 = (new_base >> 16) as u8;
        self.base_24_31 = (new_base >> 24) as u8;
    }

    fn missing() -> Self {
        Entry {
            limit_0_15: 0,
            base_0_15: 0,
            base_16_23: 0,
            access_byte: BitFlags::new(0),
            flags_limit_16_19: BitFlags::new(0),
            base_24_31: 0,
        }
    }

    fn is_null(&self) -> bool {
        self.limit_0_15 == 0
            && self.base_0_15 == 0
            && self.base_16_23 == 0
            && self.access_byte.value == 0
            && self.flags_limit_16_19.value == 0
            && self.base_24_31 == 0
    }
}

impl Default for Entry {
    fn default() -> Self {
        Entry::missing()
    }
}

#[allow(dead_code)]
#[repr(C, packed)]
pub struct TaskStateSegment {
    link: u16,
    _reserved_link: u16,
    pub esp0: u32,
    pub ss0: u16,
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

#[repr(C, packed)]
pub struct GlobalDescriptorTable(pub [Entry; 32]);

impl GlobalDescriptorTable {
    fn new() -> Self {
        GlobalDescriptorTable(Default::default())
    }

    fn descriptor(&self) -> GdtDescriptor {
        GdtDescriptor {
            size: (self.num_segments() * size_of::<Entry>()) as u16 - 1,
            offset: &self.0 as *const _ as u32,
        }
    }

    pub unsafe fn load(&mut self) {
        // Place the GDT descriptor in the null segment.
        let null_segment = &mut self.0[0] as *mut Entry;
        *null_segment = self.descriptor().into();

        // And load it.
        let descriptor = null_segment as *const GdtDescriptor;
        load_gdt(descriptor);
    }

    fn num_segments(&self) -> usize {
        let mut num_segments = 0;
        for (i, segment) in self.0.iter().enumerate() {
            if i != 0 && segment.is_null() {
                num_segments = i;
                break;
            } else if i == self.0.len() {
                num_segments = i;
            }
        }
        assert_ne!(
            num_segments, 0,
            "there are no null entries at the end of the GDT",
        );
        num_segments
    }
}

#[repr(C, packed)]
struct GdtDescriptor {
    size: u16,
    offset: u32,
}

impl Into<Entry> for GdtDescriptor {
    fn into(self) -> Entry {
        Entry {
            limit_0_15: self.size,
            base_0_15: self.offset as u16,
            base_16_23: (self.offset >> 16) as u8,
            access_byte: BitFlags::new((self.offset >> 24) as u8),
            flags_limit_16_19: BitFlags::new(0),
            base_24_31: 0,
        }
    }
}

pub static mut TSS: TaskStateSegment = TaskStateSegment::new();

pub const KERNEL_CODE_IDX: usize = 1;
pub const KERNEL_DATA_IDX: usize = 2;
pub const USERMODE_CODE_IDX: usize = 3;
pub const USERMODE_DATA_IDX: usize = 4;
pub const TSS_IDX: usize = 5;
pub const TLS_IDX: usize = 6;

pub const KERNEL_CODE_SEG: u16 = 8 * KERNEL_CODE_IDX as u16;
pub const KERNEL_DATA_SEG: u16 = 8 * KERNEL_DATA_IDX as u16;
pub const USERMODE_CODE_SEG: u16 = 8 * USERMODE_CODE_IDX as u16;
pub const USERMODE_DATA_SEG: u16 = 8 * USERMODE_DATA_IDX as u16;
pub const TSS_SEG: u16 = 8 * TSS_IDX as u16;
pub const TLS_SEG: u16 = 8 * TLS_IDX as u16;

kernel_static! {
    pub static ref GDT: Mutex<GlobalDescriptorTable> = Mutex::new({
        let mut gdt = GlobalDescriptorTable::new();

        // Code segment.
        gdt.0[KERNEL_CODE_IDX] = Entry::new(
            0x0000_0000,
            0xFFFFF,
            (BitFlags::new(0)
                | AccessByte::Present
                | AccessByte::NotTaskStateSegment
                | AccessByte::Executable
                | AccessByte::ReadableWritable
            ).value,
            (BitFlags::new(0)
             | EntryFlags::ProtectedMode32Bit
             | EntryFlags::PageGranularity
            ).value,
        );

        // Data segment.
        gdt.0[KERNEL_DATA_IDX] = Entry::new(
            0x0000_0000,
            0xFFFFF,
            (BitFlags::new(0)
                | AccessByte::Present
                | AccessByte::NotTaskStateSegment
                | AccessByte::ReadableWritable
            ).value,
            (BitFlags::new(0)
             | EntryFlags::ProtectedMode32Bit
             | EntryFlags::PageGranularity
            ).value,
        );

        // Usermode code segment.
        gdt.0[USERMODE_CODE_IDX] = Entry::new(
            0x0000_0000,
            0xFFFFF,
            (BitFlags::new(0)
                | AccessByte::Present
                | AccessByte::Usermode
                | AccessByte::NotTaskStateSegment
                | AccessByte::Executable
                | AccessByte::ReadableWritable)
                .value,
            (BitFlags::new(0)
                | EntryFlags::ProtectedMode32Bit
                | EntryFlags::PageGranularity)
                .value,
        );

        // Usermode data segment.
        gdt.0[USERMODE_DATA_IDX] = Entry::new(
            0x0000_0000,
            0xFFFFF,
            (BitFlags::new(0)
                | AccessByte::Present
                | AccessByte::Usermode
                | AccessByte::NotTaskStateSegment
                | AccessByte::ReadableWritable)
                .value,
            (BitFlags::new(0)
                | EntryFlags::ProtectedMode32Bit
                | EntryFlags::PageGranularity)
                .value,
        );

        // Task state segment.
        gdt.0[TSS_IDX] = Entry::new(
            unsafe { &TSS as *const _ as u32 },
            size_of::<TaskStateSegment>() as u32,
            (BitFlags::new(0)
                | AccessByte::Present
                | AccessByte::Executable
                | AccessByte::Accessed)
                .value,
            (BitFlags::new(0) | EntryFlags::PageGranularity).value,
        );

        // Thread local storage.
        gdt.0[TLS_IDX] = Entry::new(
            0xDEADBEEF,
            7 * 4, // see mlibc/options/internal/include/mlibc/tcb.hpp
            (BitFlags::new(0)
                | AccessByte::Present
                | AccessByte::NotTaskStateSegment
                | AccessByte::Usermode
                | AccessByte::ReadableWritable)
                .value,
            (BitFlags::new(0)
                | EntryFlags::ProtectedMode32Bit)
                .value,
        );

        gdt
    });
}

pub fn init() {
    unsafe {
        GDT.lock().load();
    }
}
