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

#[repr(C, packed)]
pub struct GlobalDescriptorTable([Entry; 32]);

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

    pub fn add_segment(&mut self, entry: Entry) -> u16 {
        let idx = self.num_segments();
        assert!(idx != self.0.len(), "no place in the GDT for a new entry");
        self.0[idx] = entry;
        idx as u16 * 8
    }

    pub unsafe fn load(&mut self) {
        // Place the GDT descriptor in the null segment.
        let null_segment = &mut self.0[0] as *mut Entry;
        *null_segment = self.descriptor().into();

        // And load it.
        let descriptor = null_segment as *const GdtDescriptor;
        load_gdt(descriptor);
    }

    pub fn kernel_data_segment(&self) -> u16 {
        0x10
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
        assert!(num_segments != 0, "no null entries in the end of GDT");
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

kernel_static! {
    pub static ref GDT: Mutex<GlobalDescriptorTable> = Mutex::new({
        let mut gdt = GlobalDescriptorTable::new();

        // Code segment
        gdt.0[1] = Entry::new(
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

        // Data segment
        gdt.0[2] = Entry::new(
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

        gdt
    });
}

pub fn init() {
    unsafe {
        GDT.lock().load();
    }
}
