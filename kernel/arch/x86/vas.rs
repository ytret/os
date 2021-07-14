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

use crate::arch::pmm_stack::PMM_STACK;
use crate::KERNEL_INFO;

use crate::arch::interrupts::InterruptStackFrame;
use crate::kernel_static::Mutex;
use crate::memory_region::Region;

bitflags_new! {
    pub struct DirEntry: u32 {
        const PRESENT = 1 << 0;               // not set: not PRESENT
        const READ_WRITE = 1 << 1;            // not set: read-only
        const ANY_DPL = 1 << 2;               // not set: must be DPL 0 to access
        const WRITE_THROUGH_CACHING = 1 << 3; // not set: write-back caching
        const NO_CACHING = 1 << 4;            // not set: enable caching
        const ACCESSED = 1 << 5;              // not set: not accessed
        // Bit 6 must be zero.
        const PAGE_SIZE_IS_4_MIB = 1 << 7;    // not set: page size is 4 KiB
        // Bit 8 is ignored.
    }
}

bitflags_new! {
    pub struct TableEntry: u32 {
        const PRESENT = 1 << 0;               // not set: not PRESENT
        const READ_WRITE = 1 << 1;            // not set: read-only
        const ANY_DPL = 1 << 2;               // not set: must be DPL 0 to access
        const WRITE_THROUGH_CACHING = 1 << 3; // not set: write-back caching
        const NO_CACHING = 1 << 4;            // not set: enable caching
        const ACCESSED = 1 << 5;              // not set: not accessed
        const DIRTY = 1 << 6;                 // not set: not dirty (not written to)
        // Bit 7 must be zero if PAT is not supported.
        const GLOBAL = 1 << 8;                // not set: not invalidated on CR3 reset (set CR4)

        // OS-specific:
        const GUARD_PAGE = 1 << 9;
        const WAS_PRESENT = 1 << 10;
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
                entry.insert(TableEntry::PRESENT);
                entry.insert(TableEntry::READ_WRITE);
                entry.insert(TableEntry::ANY_DPL);
            }

            pgdir.0[i].set_addr(&pgtbls[i] as *const _ as u32);
            pgdir.0[i].insert(DirEntry::PRESENT);
            pgdir.0[i].insert(DirEntry::READ_WRITE);
            pgdir.0[i].insert(DirEntry::ANY_DPL);

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
            if pde.contains(DirEntry::PRESENT) {
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
                    if pgtbl.0[j].contains(TableEntry::PRESENT) {
                        pgtbl.0[j] = TableEntry::with_addr(pgtbl.0[j].addr());
                        pgtbl.0[j].insert(TableEntry::PRESENT);
                        pgtbl.0[j].insert(TableEntry::READ_WRITE);
                        pgtbl.0[j].insert(TableEntry::ANY_DPL);
                    }
                }

                // Set the PDE.
                let pgdir = vas.pgdir_virt.as_mut().unwrap();
                pgdir.0[i].set_addr(*vas.pgtbls_phys.add(i));
                pgdir.0[i].insert(DirEntry::PRESENT);
                pgdir.0[i].insert(DirEntry::READ_WRITE);
                pgdir.0[i].insert(DirEntry::ANY_DPL);
            }
        }

        vas
    }

    pub unsafe fn copy(&self) -> Self {
        let new_pgdir_virt = alloc(Layout::from_size_align(4096, 4096).unwrap())
            as *mut Directory;
        let new_pgdir_phys = self.virt_to_phys(new_pgdir_virt as u32).unwrap();

        let new_vas = VirtAddrSpace {
            pgdir_virt: new_pgdir_virt,
            pgdir_phys: new_pgdir_phys,

            pgtbls_virt: alloc(Layout::from_size_align(4096, 4096).unwrap())
                .cast(),
            pgtbls_phys: alloc(Layout::from_size_align(4096, 4096).unwrap())
                .cast(),

            usermode: self.usermode,
        };

        new_vas.pgdir_virt.write_bytes(0, 1);
        new_vas.pgtbls_virt.write_bytes(0, 1024);
        new_vas.pgtbls_phys.write_bytes(0, 1024);

        let pgdir = self.pgdir_virt.as_ref().unwrap();
        let new_pgdir = new_pgdir_virt.as_mut().unwrap();

        // Allocate a page on the heap and use it for copying physical pages
        // from one VAS to another.  FIXME: this is dirty and slow.
        let copying_virt =
            alloc(Layout::from_size_align(4096, 4096).unwrap()) as u32;
        let initial_mapping = self.pgtbl_entry(copying_virt).addr();

        for (pde_idx, pde) in
            self.pgdir_virt.as_ref().unwrap().0.iter().enumerate()
        {
            // println!("pde_idx = {}", pde_idx);
            // println!("pde = 0x{:08X}", pde as *const _ as u32);

            if pde.contains(DirEntry::PRESENT) {
                // println!("- is PRESENT");

                let pgtbl_virt = self.pgtbl_virt_of((pde_idx as u32) << 22);
                let new_pgtbl_virt =
                    alloc(Layout::from_size_align(4096, 4096).unwrap())
                        as *mut Table;
                new_pgtbl_virt.write_bytes(0, 1);

                let pgtbl = pgtbl_virt.as_ref().unwrap();
                let new_pgtbl = new_pgtbl_virt.as_mut().unwrap();

                new_pgdir.0[pde_idx] = pgdir.0[pde_idx];
                new_vas.set_pde_phys_virt(
                    pde_idx,
                    KERNEL_VAS
                        .lock()
                        .virt_to_phys(new_pgtbl_virt as u32)
                        .unwrap(),
                    new_pgtbl_virt,
                );

                for (pte_idx, pte) in pgtbl.0.iter().enumerate() {
                    // println!(" - pte_idx = {}", pte_idx);
                    // println!(" - pte = 0x{:08X}", pte as *const _ as u32);
                    if pte.contains(TableEntry::PRESENT) {
                        let copy_from =
                            ((pde_idx << 22) | (pte_idx << 12)) as u32;

                        let acpi_region = KERNEL_INFO
                            .arch
                            .hpet_region
                            .unwrap_or(Region { start: 0, end: 0 });

                        // If this page is within the kernel or ACPI region,
                        // retain the mapping so that the kernel and ACPI memory
                        // are mapped the same way across different VASes.
                        if KERNEL_REGION.contains(&(copy_from as usize))
                            || acpi_region.contains(&(copy_from as usize))
                        {
                            new_pgtbl.0[pte_idx] = pgtbl.0[pte_idx];
                            continue;
                        }

                        // Otherwise, allocate a new physical page and copy the
                        // original page contents into it via `copying_virt'.

                        let phys = PMM_STACK.lock().pop_page();

                        new_pgtbl.0[pte_idx] = pgtbl.0[pte_idx];
                        new_pgtbl.0[pte_idx].set_addr(phys);

                        self.pgtbl_entry(copying_virt).set_addr(phys);
                        self.invalidate_cache(copying_virt);

                        assert_ne!(copy_from, copying_virt);

                        // print!("Copying from 0x{:08X} to 0x{:08X}... ", copy_from, copying_virt);
                        ptr::copy_nonoverlapping(
                            copy_from as *const u8,
                            copying_virt as *mut u8,
                            4096,
                        );
                        // println!("done");
                    }
                }
            }
        }

        // Restore the original mapping of the copying page.
        self.pgtbl_entry(copying_virt).set_addr(initial_mapping);

        new_vas
    }

    pub unsafe fn load(&self) {
        asm!("movl {}, %cr3", in(reg) self.pgdir_phys, options(att_syntax));
    }

    pub unsafe fn map_page(&self, virt: u32, phys: u32) {
        assert_eq!(virt & 0xFFF, 0, "virt must be page-aligned");
        assert_eq!(phys & 0xFFF, 0, "phys must be page-aligned");

        let entry = self.pgtbl_entry(virt);
        entry.set_addr(phys);
        entry.insert(TableEntry::PRESENT);
        entry.insert(TableEntry::READ_WRITE);
        if self.usermode {
            entry.insert(TableEntry::ANY_DPL);
        }

        self.invalidate_cache(virt);
    }

    pub fn is_mapped(&self, virt: u32) -> bool {
        unsafe { self.virt_to_phys(virt).is_some() }
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

        if entry.contains(TableEntry::PRESENT) {
            entry.insert(TableEntry::PRESENT);
            entry.insert(TableEntry::WAS_PRESENT);
        }
        entry.insert(TableEntry::GUARD_PAGE);

        asm!("invlpg ({})", in(reg) at, options(att_syntax));
        println!("[VAS] Placed a guard page at 0x{:08X}.", at);
    }

    pub unsafe fn remove_guard_page(&mut self, from: u32) {
        assert_eq!(from & 0xFFF, 0, "from must be page-aligned");
        let entry = self.pgtbl_entry(from);

        if entry.contains(TableEntry::WAS_PRESENT) {
            entry.insert(TableEntry::WAS_PRESENT);
            entry.insert(TableEntry::PRESENT);
        }
        entry.insert(TableEntry::GUARD_PAGE);

        asm!("invlpg ({})", in(reg) from, options(att_syntax));
        println!("[VAS] Removed a guard page from 0x{:08X}.", from);
    }

    /// Sets up a page directory entry with the specified index with the
    /// physical mapping of the specified virtual address and the default flags.
    ///
    /// # Default flags
    /// See [set_pde_phys_virt](Self::set_pde_phys_virt) for the default flags.
    ///
    /// # Panics
    /// This method panics if it cannot resolve the specified virtual address to
    /// a physical one within the address space being modified.
    pub unsafe fn set_pde_virt(&self, pde_idx: usize, pgtbl_virt: *mut Table) {
        assert!(pde_idx < 1024, "pde_idx must be less than 1024");
        assert_eq!(
            pgtbl_virt as usize % 4096,
            0,
            "pgtbl_virt must be page-aligned",
        );

        let pgtbl_phys = self
            .virt_to_phys(pgtbl_virt as u32)
            .expect("set_pde_addr: virt_to_phys failed");
        self.set_pde_phys_virt(pde_idx, pgtbl_phys, pgtbl_virt);
    }

    /// Sets up a page directory entry with the specified index with the
    /// specified physical address of a page table and the default flags.
    ///
    /// Unlike [set_pde_virt](Self::set_pde_virt), this method does not try to
    /// resolve any mappings and thus does not panic.
    ///
    /// The virtual address of the page table is saved internally.
    ///
    /// # Default flags
    /// The default flags are:
    /// * [PRESENT](DirEntry::PRESENT),
    /// * [readable and writable](DirEntry::READ_WRITE),
    /// * [any DPL](DirEntry::ANY_DPL) (if [VirtAddrSpace::usermode] is `true`).
    unsafe fn set_pde_phys_virt(
        &self,
        pde_idx: usize,
        pgtbl_phys: u32,
        pgtbl_virt: *mut Table,
    ) {
        assert!(pde_idx < 1024, "pde_idx must be less than 1024");
        assert_eq!(pgtbl_phys % 4096, 0, "pgtbl_phys must be page-aligned");

        *self.pgtbls_virt.add(pde_idx) = pgtbl_virt;
        *self.pgtbls_phys.add(pde_idx) = pgtbl_phys;

        let pgdir = self.pgdir_virt.as_mut().unwrap();
        pgdir.0[pde_idx].set_addr(pgtbl_phys);
        pgdir.0[pde_idx].insert(DirEntry::PRESENT);
        pgdir.0[pde_idx].insert(DirEntry::READ_WRITE);
        if self.usermode {
            pgdir.0[pde_idx].insert(DirEntry::ANY_DPL);
        }
    }

    pub unsafe fn virt_to_phys(&self, virt: u32) -> Option<u32> {
        let pgtbl_virt = self.pgtbl_virt_of(virt);
        if !pgtbl_virt.is_null() {
            let pte = self.pgtbl_entry(virt);
            if pte.contains(TableEntry::PRESENT) {
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

    fn invalidate_cache(&self, virt: u32) {
        assert_eq!(virt % 4096, 0, "virt must be page-aligned");
        unsafe {
            asm!("invlpg ({})", in(reg) virt, options(att_syntax));
        }
    }
}

impl DirEntry {
    fn addr(&self) -> u32 {
        self.bits() & !0xFFF
    }

    fn set_addr(&mut self, addr: u32) {
        assert_eq!(addr % 4096, 0, "addr must be page-aligned");
        self.0 = addr | self.bits() & 0xFFF;
    }
}

impl TableEntry {
    fn with_addr(addr: u32) -> Self {
        let mut entry = Self::empty();
        entry.set_addr(addr);
        entry
    }

    fn addr(&self) -> u32 {
        self.bits() & !0xFFF
    }

    fn set_addr(&mut self, addr: u32) {
        assert_eq!(addr % 4096, 0, "addr must be page-aligned");
        self.0 = addr | self.bits() & 0xFFF;
    }
}

#[repr(align(4096))]
pub struct Directory([DirEntry; 1024]);

#[derive(Clone, Copy)]
#[repr(align(4096))]
pub struct Table([TableEntry; 1024]);

impl Directory {
    fn new() -> Self {
        Directory([DirEntry::empty(); 1024])
    }
}

impl Table {
    fn new() -> Self {
        Table([TableEntry::empty(); 1024])
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

const KERNEL_REGION: Region<usize> = Region {
    start: 0x00000000,
    end: 0x08000000, // 128 MiB
};

pub const USERMODE_REGION: Region<usize> = Region {
    start: 128 * 1024 * 1024,                      // 128 MiB
    end: 3 * 1024 * 1024 * 1024 + 4 * 1024 * 1024, // 3 GiB + 4 MiB
};

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
            if entry.contains(TableEntry::GUARD_PAGE) {
                println!("There is a guard page at 0x{:08X}.", page);
            }
        }
    } else {
        println!("Unable to lock the kernel VAS.");
    }

    panic!("Unhandled page fault.");
}
