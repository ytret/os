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

use crate::bitflags::BitFlags;

// These are entry flags common to directory and table entries.
macro_rules! entry_flags {
    ($N:ident { $($V:ident = $E:expr,)+ }) => {
        bitflags! {
            #[repr(u32)]
            enum $N {
                Present = 1 << 0,             // not set: not present
                ReadWrite = 1 << 1,           // not set: read-only
                AnyDpl = 1 << 2,              // not set: must be DPL 0 to access
                WriteThroughCaching = 1 << 3, // not set: write-back caching
                NoCaching = 1 << 4,           // not set: enable caching
                Accessed = 1 << 5,            // not set: not accessed
            }
        }
    };
}

entry_flags! {
    DirectoryEntryFlags {
        // Bit 6 must be zero.
        PageSizeIs4Mib = 1 << 7, // not set: page size is 4 KiB
        // Bit 8 is ignored.
    }
}

entry_flags! {
    TableEntryFlags {
        Dirty = 1 << 6, // not set: not dirty (not written to)
        // Bit 7 must be zero if PAT is not supported.
        Global = 1 < 8, // not set: not invalidated on CR3 reset (set CR4)
    }
}

#[derive(Clone, Copy)]
#[repr(transparent)]
struct Entry<F: Into<u32>>(BitFlags<u32, F>);

impl<F: Into<u32>> Entry<F> {
    fn new(addr: *const u32) -> Self {
        let addr = addr as u32;
        assert_eq!(addr & 0xFFF, 0, "new: addr must be page-aligned");
        Entry(BitFlags::new(addr))
    }

    fn missing() -> Self {
        Self::new(core::ptr::null())
    }

    fn set_addr(&mut self, addr: *const u32) {
        let addr = addr as u32;
        assert_eq!(addr & 0xFFF, 0, "set_addr: addr must be page-aligned");
        self.0.value = addr | self.0.value;
    }

    fn set_flag(&mut self, flag: F) {
        self.0.set_flag(flag);
    }
}

/*
impl<'a, T, F> Into<u32> for Entry<'a, T, F>
where
    F: Into<u32>,
{
    fn into(self) -> u32 {
        assert_eq!(self.addr.into() & 0xFFF, 0);
        assert_eq!(self.flags.value & !(0xFFF), 0);
        self.addr.into() | self.flags.value
    }
}
*/

#[repr(align(4096))]
pub struct Directory([Entry<DirectoryEntryFlags>; 1024]);

impl Directory {
    fn new() -> Self {
        Directory([Entry::missing(); 1024])
    }

    unsafe fn load(&self) {
        asm!("movl {}, %cr3", in(reg) &self.0, options(att_syntax))
    }
}

#[derive(Clone, Copy)]
#[repr(align(4096))]
pub struct Table([Entry<TableEntryFlags>; 1024]);

impl Table {
    fn new() -> Self {
        Table([Entry::missing(); 1024])
    }
}

kernel_static! {
    static ref KERNEL_PAGE_DIR: Directory = {
        let mut kpd = Directory::new();
        kpd.0[0].set_addr(&*KERNEL_PAGE_TABLE as *const _ as *const u32);
        kpd.0[0].set_flag(DirectoryEntryFlags::Present);
        kpd
    };

    static ref KERNEL_PAGE_TABLE: Table = {
        // Identity map the first 4 MiB.
        let mut kpt = Table::new();
        let mut i = 0;
        while i < kpt.0.len() {
            let entry = &mut kpt.0[i];
            entry.set_addr((i << 12) as *const u32);
            entry.set_flag(TableEntryFlags::Present);
            i += 1;
        }
        kpt
    };
}

pub fn init(kernel_size: u32) {
    let kernel_size_mib = kernel_size as f64 / 1024.0 / 1024.0;
    if kernel_size_mib >= 3.0 {
        panic!(
            "Kernel size has exceeded 3 MiB ({} MiB). \
             Please modify paging code.",
            kernel_size_mib
        );
    }

    unsafe {
        KERNEL_PAGE_DIR.load();
        asm!("movl %cr0, %eax
              orl $0x80000001, %eax
              movl %eax, %cr0",
             out("eax") _,
             options(att_syntax));
    }
}