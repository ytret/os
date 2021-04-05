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

use alloc::alloc::{alloc, Layout};
use core::mem::align_of;
use core::ptr;

use crate::arch::interrupts::InterruptStackFrame;
use crate::arch::pmm_stack::PMM_STACK;
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

        // OS-specific:
        GuardPage = 1 << 9,
        WasPresent = 1 << 10,
    }
}

// It is the user's obligation to ensure that the VAS is consistent, meaning
// that the PDEs and PT pointers point to the same PTs.  Otherwise it is
// undefined behavior.
#[derive(Clone)]
pub struct VirtAddrSpace {
    pgdir_virt: *mut Directory, // relative to the kernel VAS
    pub pgdir_phys: u32,

    pgtbls_virt: *mut *mut Table, // same
    pgtbls_phys: *mut u32,

    usermode: bool,
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

            usermode: false,
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

        let vas = VirtAddrSpace {
            pgdir_virt: heap_pgdir as *mut Directory,
            pgdir_phys: (*kvas).virt_to_phys(heap_pgdir as u32).unwrap(),

            pgtbls_virt: heap_pgtbls_virt as *mut *mut Table,
            pgtbls_phys: heap_pgtbls_phys as *mut u32,

            usermode: true,
        };

        // Copy the kernel VAS.
        let pgdir = (*kvas).pgdir_virt.as_mut().unwrap();
        for i in 0..1024 {
            let pde = &pgdir.0[i];
            if pde.flags().has_set(PdeFlags::Present) {
                // Copy the corresponding page table.
                let src = pde.addr() as *mut u8;
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

    pub unsafe fn map_page(&self, virt: u32, phys: u32) {
        assert_eq!(virt & 0xFFF, 0, "virt must be page-aligned");
        assert_eq!(phys & 0xFFF, 0, "phys must be page-aligned");

        let entry = self.pgtbl_entry(virt);
        entry.set_addr(phys);
        entry.set_flag(PteFlags::Present);
        entry.set_flag(PteFlags::ReadWrite);
        if self.usermode {
            entry.set_flag(PteFlags::AnyDpl);
        }

        asm!("invlpg ({})", in(reg) virt, options(att_syntax));
    }

    /// Maps the specified region to pages given by the [PMM
    /// stack](static@super::pmm_stack::PMM_STACK).
    pub unsafe fn allocate_pages_from_stack(&self, start: u32, end: u32) {
        assert_eq!(start & 0xFFF, 0, "start must be page-aligned");
        assert_eq!(end & 0xFFF, 0, "end must be page-aligned");
        for virt in (start..end).step_by(4096) {
            let phys = PMM_STACK.lock().pop_page();
            self.map_page(virt, phys);
        }
    }

    pub unsafe fn place_guard_page(&mut self, at: u32) {
        assert_eq!(at & 0xFFF, 0, "at must be page-aligned");
        let entry = self.pgtbl_entry(at);

        if entry.flags().has_set(PteFlags::Present) {
            entry.unset_flag(PteFlags::Present);
            entry.set_flag(PteFlags::WasPresent);
        }
        entry.set_flag(PteFlags::GuardPage);

        asm!("invlpg ({})", in(reg) at, options(att_syntax));
        println!("[VAS] Placed a guard page at 0x{:08X}.", at);
    }

    pub unsafe fn remove_guard_page(&mut self, from: u32) {
        assert_eq!(from & 0xFFF, 0, "from must be page-aligned");
        let entry = self.pgtbl_entry(from);

        if entry.flags().has_set(PteFlags::WasPresent) {
            entry.unset_flag(PteFlags::WasPresent);
            entry.set_flag(PteFlags::Present);
        }
        entry.unset_flag(PteFlags::GuardPage);

        asm!("invlpg ({})", in(reg) from, options(att_syntax));
        println!("[VAS] Removed a guard page from 0x{:08X}.", from);
    }

    pub unsafe fn set_pde_addr(
        &self,
        pde_idx: usize,
        pgtbl_virt: *mut Table,
    ) -> u32 {
        assert_eq!(
            pgtbl_virt as usize & 0xFFF,
            0,
            "pgtbl_virt must be page-aligned",
        );
        assert!(pde_idx < 1024, "pde_idx must be less than 1024");

        let pgtbl_phys = self.virt_to_phys(pgtbl_virt as u32).unwrap();

        *self.pgtbls_virt.add(pde_idx) = pgtbl_virt;
        *self.pgtbls_phys.add(pde_idx) = pgtbl_phys;

        let pgdir = self.pgdir_virt.as_mut().unwrap();
        pgdir.0[pde_idx].set_addr(pgtbl_phys);
        pgdir.0[pde_idx].set_flag(PdeFlags::Present);
        pgdir.0[pde_idx].set_flag(PdeFlags::ReadWrite);
        if self.usermode {
            pgdir.0[pde_idx].set_flag(PdeFlags::AnyDpl);
        }

        pgtbl_phys
    }

    pub unsafe fn virt_to_phys(&self, virt: u32) -> Option<u32> {
        let pgtbl_virt = self.pgtbl_virt_of(virt);
        if !pgtbl_virt.is_null() {
            let pte = self.pgtbl_entry(virt);
            if pte.flags().has_set(PteFlags::Present) {
                Some(pte.addr())
            } else {
                None
            }
        } else {
            None
        }
    }

    pub unsafe fn pgtbl_entry(&self, virt: u32) -> &mut TableEntry {
        let pgtbl_virt = self.pgtbl_virt_of(virt);
        assert!(!pgtbl_virt.is_null(), "page table does not exist");

        let pte_idx = ((virt >> 12) & 0x3FF) as usize;
        &mut (*pgtbl_virt).0[pte_idx]
    }

    pub unsafe fn pgtbl_virt_of(&self, virt: u32) -> *mut Table {
        let pde_idx = (virt >> 22) as usize;
        *self.pgtbls_virt.add(pde_idx)
    }
}

#[derive(Clone, Copy)]
#[repr(transparent)]
pub struct Entry<F: Into<u32> + Copy>(BitFlags<u32, F>);

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

    fn unset_flag(&mut self, flag: F) {
        self.0.unset_flag(flag);
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
    static ref KERNEL_PGTBLS: Mutex<[Table; 2]> = Mutex::new([Table::new(); 2]);
    static ref KERNEL_PGTBLS_VIRT: Mutex<[*mut Table; 1024]> = Mutex::new([ptr::null_mut(); 1024]);
    static ref KERNEL_PGTBLS_PHYS: Mutex<[u32; 1024]> = Mutex::new([0; 1024]);

    pub static ref ACPI_PGTBL: Mutex<Table> = Mutex::new(Table::new());

    pub static ref KERNEL_HEAP_PGTBL: Mutex<Table> = Mutex::new(Table::new());

    pub static ref KERNEL_VAS: Mutex<VirtAddrSpace> = Mutex::new(unsafe {
        VirtAddrSpace::new_identity_mapped(
            &mut *KERNEL_PGDIR.lock(),
            &mut *KERNEL_PGTBLS.lock(),
            (KERNEL_PGTBLS_VIRT.lock().as_mut_ptr(), KERNEL_PGTBLS_PHYS.lock().as_mut_ptr()),
        )
    });
}

#[no_mangle]
pub extern "C" fn page_fault_handler(
    int_num: u32,
    err_code: u32,
    stack_frame: &InterruptStackFrame,
) {
    assert_eq!(int_num, 14);
    println!("A page fault has occurred.");
    println!(
        " error code: {:08b}_{:08b}_{:08b}_{:08b} (0x{:08X})",
        (err_code >> 24) & 0xF,
        (err_code >> 16) & 0xF,
        (err_code >> 08) & 0xF,
        (err_code >> 00) & 0xF,
        err_code
    );

    let eip = stack_frame.eip;
    println!(" eip: 0x{:08X}", eip);

    let cr2: u32;
    unsafe {
        asm!("movl %cr2, %eax", out("eax") cr2, options(att_syntax));
    }
    println!(" cr2: 0x{:08X}", cr2);

    print!("Details: ");
    match (err_code >> 0) & 1 {
        0 => print!("non-present page, "),
        _ => print!("page-protection violation, "),
    }
    match (err_code >> 1) & 1 {
        0 => print!("read, "),
        _ => print!("write, "),
    }
    match (err_code >> 2) & 1 {
        0 => print!("kernel"),
        _ => print!("userspace"),
    }
    match (err_code >> 3) & 1 {
        0 => {}
        _ => print!(", instruction fetch"),
    }
    println!(".");

    if let Some(kvas) = KERNEL_VAS.try_lock() {
        let page = cr2 & !0xFFF;
        let pgtbl_virt = unsafe { kvas.pgtbl_virt_of(page) };
        if pgtbl_virt.is_null() {
            println!("No page table for 0x{:08X}.", cr2);
        } else {
            let entry = unsafe { kvas.pgtbl_entry(page) };
            if entry.flags().has_set(PteFlags::GuardPage) {
                println!("There is a guard page at 0x{:08X}.", page);
            }
        }
    } else {
        println!("Unable to lock the kernel VAS.");
    }

    panic!("Unhandled page fault.");
}
