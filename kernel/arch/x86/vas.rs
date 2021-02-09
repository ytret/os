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

use alloc::alloc::{alloc, Layout};
use core::mem::align_of;
use core::ptr;

use crate::bitflags::BitFlags;
use crate::kernel_static::Mutex;

bitflags! {
    #[repr(u32)]
    pub enum PdeFlags {
        Present = 1 << 0,             // not set: not present
        ReadWrite = 1 << 1,           // not set: read-only
        AnyDpl = 1 << 2,              // not set: must be DPL 0 to access
        WriteThroughCaching = 1 << 3, // not set: write-back caching
        NoCaching = 1 << 4,           // not set: enable caching
        Accessed = 1 << 5,            // not set: not accessed
        // Bit 6 must be zero.
        PageSizeIs4Mib = 1 << 7,      // not set: page size is 4 KiB
        // Bit 8 is ignored.
    }
}

bitflags! {
    #[repr(u32)]
    pub enum PteFlags {
        Present = 1 << 0,             // not set: not present
        ReadWrite = 1 << 1,           // not set: read-only
        AnyDpl = 1 << 2,              // not set: must be DPL 0 to access
        WriteThroughCaching = 1 << 3, // not set: write-back caching
        NoCaching = 1 << 4,           // not set: enable caching
        Accessed = 1 << 5,            // not set: not accessed
        Dirty = 1 << 6,               // not set: not dirty (not written to)
        // Bit 7 must be zero if PAT is not supported.
        Global = 1 << 8,              // not set: not invalidated on CR3 reset (set CR4)
    }
}

pub struct VirtAddrSpace {
    pgdir_virt: *mut Directory, // relative to the kernel VAS
    pgdir_phys: u32,

    pgtbls_virt: *mut *mut Table, // same
    pgtbls_phys: *mut u32,
}

impl VirtAddrSpace {
    pub unsafe fn new_identity_mapped(
        pgdir: &mut Directory,
        pgtbls: &mut [Table],
        pgtbls_ptrs: (*mut *mut Table, *mut u32),
    ) -> Self {
        for i in 0..pgtbls.len() {
            for j in 0..pgtbls[i].0.len() {
                let entry = &mut pgtbls[i].0[j];
                entry.set_addr((i << 22 | j << 12) as u32);
                entry.set_flag(PteFlags::Present);
                entry.set_flag(PteFlags::ReadWrite);
                entry.set_flag(PteFlags::AnyDpl);
            }

            pgdir.0[i].set_addr(&pgtbls[i] as *const _ as u32);
            pgdir.0[i].set_flag(PdeFlags::Present);
            pgdir.0[i].set_flag(PdeFlags::ReadWrite);
            pgdir.0[i].set_flag(PdeFlags::AnyDpl);

            *pgtbls_ptrs.0.add(i) = &mut pgtbls[i] as *mut Table;
            *pgtbls_ptrs.1.add(i) = &pgtbls[i] as *const _ as u32;
        }

        VirtAddrSpace {
            pgdir_virt: pgdir as *mut Directory,
            pgdir_phys: pgdir as *const _ as u32,

            pgtbls_virt: pgtbls_ptrs.0,
            pgtbls_phys: pgtbls_ptrs.1,
        }
    }

    pub unsafe fn kvas_copy_on_heap() -> Self {
        // This should be used only in the kernel VAS because it uses the kernel
        // PD to translate virtual addresses (of heap allocations) to physical
        // ones.
        let kvas = KERNEL_VAS.lock();

        // Allocate space on the heap.
        let heap_pgdir = alloc(Layout::from_size_align(4096, 4096).unwrap());
        let heap_pgtbls_virt = alloc(
            Layout::from_size_align(4096, align_of::<*mut Table>()).unwrap(),
        );
        let heap_pgtbls_phys = alloc(
            Layout::from_size_align(4096, align_of::<*mut Table>()).unwrap(),
        );
        ptr::write_bytes(heap_pgdir, 0, 4096);
        ptr::write_bytes(heap_pgtbls_virt, 0, 4096);
        ptr::write_bytes(heap_pgtbls_phys, 0, 4096);

        let mut vas = VirtAddrSpace {
            pgdir_virt: heap_pgdir as *mut Directory,
            pgdir_phys: (*kvas).virt_to_phys(heap_pgdir as u32).unwrap(),

            pgtbls_virt: heap_pgtbls_virt as *mut *mut Table,
            pgtbls_phys: heap_pgtbls_phys as *mut u32,
        };

        // Copy the kernel VAS.
        let kpd = (*kvas).pgdir_virt.as_mut().unwrap();
        for i in 0..1024 {
            let kpde = &kpd.0[i];
            if kpde.flags().has_set(PdeFlags::Present) {
                // Copy the corresponding page table.
                let src = kpde.addr() as *mut u8;
                let dest = alloc(Layout::from_size_align(4096, 4096).unwrap());
                ptr::copy_nonoverlapping(src, dest, 4096);
                *vas.pgtbls_virt.add(i) = dest as *mut Table;
                *vas.pgtbls_phys.add(i) =
                    (*kvas).virt_to_phys(dest as u32).unwrap();

                // Change the flags of all PTEs.
                let pgtbl = (*vas.pgtbls_virt.add(i)).as_mut().unwrap();
                for j in 0..1024 {
                    if pgtbl.0[j].flags().has_set(PteFlags::Present) {
                        pgtbl.0[j] = TableEntry::new(pgtbl.0[j].addr());
                        pgtbl.0[j].set_flag(PteFlags::Present);
                        pgtbl.0[j].set_flag(PteFlags::ReadWrite);
                        pgtbl.0[j].set_flag(PteFlags::AnyDpl);
                    }
                }

                // Set the PDE.
                let pgdir = vas.pgdir_virt.as_mut().unwrap();
                pgdir.0[i].set_addr(*vas.pgtbls_phys.add(i));
                pgdir.0[i].set_flag(PdeFlags::Present);
                pgdir.0[i].set_flag(PdeFlags::ReadWrite);
                pgdir.0[i].set_flag(PdeFlags::AnyDpl);
            }
        }

        vas
    }

    pub unsafe fn load(&self) {
        asm!("movl {}, %cr3", in(reg) self.pgdir_phys, options(att_syntax));
    }

    pub unsafe fn virt_to_phys(&self, virt: u32) -> Option<u32> {
        let pgtbl_virt = self.pgtbl_virt_of(virt);
        if !pgtbl_virt.is_null() {
            let pte_idx = ((virt >> 12) & 0x3FF) as usize;
            Some((*pgtbl_virt).0[pte_idx].addr() + (virt & 0xFFF))
        } else {
            None
        }
    }

    unsafe fn pgtbl_virt_of(&self, virt: u32) -> *mut Table {
        let pde_idx = (virt >> 22) as usize;
        *self.pgtbls_virt.add(pde_idx)
    }
}

#[derive(Clone, Copy)]
#[repr(transparent)]
struct Entry<F: Into<u32> + Copy>(BitFlags<u32, F>);

impl<F: Into<u32> + Copy> Entry<F> {
    fn new(addr: u32) -> Self {
        Entry(BitFlags::new(addr))
    }

    fn missing() -> Self {
        Self::new(0)
    }

    fn addr(&self) -> u32 {
        self.0.value & !0xFFF
    }

    fn flags(&self) -> BitFlags<u32, F> {
        BitFlags::new(self.0.value & 0xFFF)
    }

    fn set_addr(&mut self, addr: u32) {
        assert_eq!(addr & 0xFFF, 0, "addr must be page-aligned");
        self.0.value = addr | self.flags().value;
    }

    fn set_flag(&mut self, flag: F) {
        self.0.set_flag(flag);
    }
}

type DirEntry = Entry<PdeFlags>;
type TableEntry = Entry<PteFlags>;

#[repr(align(4096))]
pub struct Directory([DirEntry; 1024]);

#[derive(Clone, Copy)]
#[repr(align(4096))]
pub struct Table([TableEntry; 1024]);

impl Directory {
    fn new() -> Self {
        Directory([Entry::missing(); 1024])
    }
}

impl Table {
    fn new() -> Self {
        Table([Entry::missing(); 1024])
    }
}

kernel_static! {
    static ref KERNEL_PGDIR: Mutex<Directory> = Mutex::new(Directory::new());
    static ref KERNEL_PGTBLS: Mutex<[Table; 3]> = Mutex::new([Table::new(); 3]);
    static ref KERNEL_PGTBLS_VIRT: Mutex<[*mut Table; 1024]> = Mutex::new([ptr::null_mut(); 1024]);
    static ref KERNEL_PGTBLS_PHYS: Mutex<[u32; 1024]> = Mutex::new([0; 1024]);

    pub static ref KERNEL_VAS: Mutex<VirtAddrSpace> = Mutex::new(unsafe {
        VirtAddrSpace::new_identity_mapped(
            &mut *KERNEL_PGDIR.lock(),
            &mut *KERNEL_PGTBLS.lock(),
            (KERNEL_PGTBLS_VIRT.lock().as_mut_ptr(), KERNEL_PGTBLS_PHYS.lock().as_mut_ptr()),
        )
    });
}
