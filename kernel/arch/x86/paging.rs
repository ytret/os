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
use crate::kernel_static::{Mutex, MutexWrapper};

use crate::arch::pmm_stack::PMM_STACK;

extern "C" {
    fn memset(s: *mut u8, c: i32, n: usize) -> *mut u8;
}

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
    fn new(addr: u32) -> Self {
        assert_eq!(addr & 0xFFF, 0, "addr must be page-aligned");
        Entry(BitFlags::new(addr))
    }

    fn missing() -> Self {
        Self::new(0)
    }

    fn set_addr(&mut self, addr: u32) {
        assert_eq!(addr & 0xFFF, 0, "addr must be page-aligned");
        self.0.value = addr | self.flags().value;
    }

    fn set_flag(&mut self, flag: F) {
        self.0.set_flag(flag);
    }

    fn addr(&self) -> u32 {
        self.0.value & !0xFFF
    }

    fn flags(&self) -> BitFlags<u32, F> {
        BitFlags::new(self.0.value & 0xFFF)
    }
}

type DirectoryEntry = Entry<DirectoryEntryFlags>;

#[repr(align(4096))]
pub struct Directory([DirectoryEntry; 1024]);

impl Directory {
    fn new() -> Self {
        Directory([Entry::missing(); 1024])
    }

    unsafe fn load(&self) {
        asm!("movl {}, %cr3", in(reg) &self.0, options(att_syntax))
    }
}

type TableEntry = Entry<TableEntryFlags>;

#[derive(Clone, Copy)]
#[repr(align(4096))]
pub struct Table([TableEntry; 1024]);

impl Table {
    fn new() -> Self {
        Table([Entry::missing(); 1024])
    }
}

kernel_static! {
    pub static ref KERNEL_PAGE_DIR: Mutex<Directory> = Mutex::new({
        let mut kpd = Directory::new();
        kpd.0[0].set_addr(&(&*KERNEL_PAGE_TABLES)[0] as *const _ as u32);
        kpd.0[0].set_flag(DirectoryEntryFlags::Present);
        kpd.0[1].set_addr(&(&*KERNEL_PAGE_TABLES)[1] as *const _ as u32);
        kpd.0[1].set_flag(DirectoryEntryFlags::Present);
        kpd
    });

    static ref KERNEL_PAGE_TABLES: [Table; 2] = {
        // Identity map the first 8 MiB.
        let mut tables = [Table::new(); 2];
        for i in 0..tables.len() {
            for j in 0..tables[i].0.len() {
                let entry = &mut tables[i].0[j];
                entry.set_addr((i << 22 | j << 12) as u32);
                entry.set_flag(TableEntryFlags::Present);
            }
        }
        tables
    };
}

pub fn init(kernel_size: u32) {
    let kernel_size_mib = kernel_size as f64 / 1024.0 / 1024.0;
    if kernel_size_mib >= 7.0 {
        panic!(
            "Kernel size has exceeded 7 MiB ({} MiB). \
             Please modify paging code.",
            kernel_size_mib
        );
    }

    unsafe {
        KERNEL_PAGE_DIR.lock().load();
        asm!("movl %cr0, %eax
              orl $0x80000001, %eax
              movl %eax, %cr0",
             out("eax") _,
             options(att_syntax));
    }
}

fn invlpg(virt: u32) {
    unsafe {
        asm!("invlpg ({})", in(reg) virt, options(att_syntax));
    }
}

pub fn map_page(virt: u32, phys: u32) {
    assert_eq!(virt & 0xFFF, 0, "virt must be page-aligned");
    assert_eq!(phys & 0xFFF, 0, "phys must be page-aligned");

    let mut kpd: MutexWrapper<Directory> = KERNEL_PAGE_DIR.lock();
    let kpd_idx = (virt >> 22) as usize;
    let kpt_idx = ((virt >> 12) & 0x3FF) as usize;

    let page_table: *mut Table;

    // If there's no such page dir entry, we allocate a physical page to store
    // the page table there.
    if (kpd.0[kpd_idx].0 & DirectoryEntryFlags::Present).value == 0 {
        if kpd.0[kpd_idx].0.value != 0 {
            unimplemented!("KPD entry is not present, but also not empty");
        }
        page_table = PMM_STACK.lock().pop_page() as *mut Table;
        unsafe {
            memset(
                page_table as *mut u8,
                0,
                (*page_table).0.len() * core::mem::size_of::<TableEntry>(),
            );
        }
        let entry = &mut (*kpd).0[kpd_idx];
        (*entry).set_addr(page_table as *const u32 as u32);
        (*entry).set_flag(DirectoryEntryFlags::Present);
    } else {
        page_table = kpd.0[kpd_idx].addr() as *mut Table;
    }

    unsafe {
        let entry = &mut (*page_table).0[kpt_idx];
        (*entry).set_addr(phys);
        (*entry).set_flag(TableEntryFlags::Present);
        invlpg(virt);
    }
}
