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

pub mod sdt;
pub mod hpet;

use crate::arch::vas::{ACPI_PGTBL, KERNEL_VAS};
use crate::KERNEL_INFO;

use crate::arch::vas::Table;
use crate::memory_region::Region;

#[derive(Clone, Copy, Debug)]
#[repr(C, packed)]
pub struct AcpiAddr {
    pub addr_space_id: u8,
    pub register_bit_width: u8,
    pub register_bit_offset: u8,
    _reserved: u8,
    pub address: u64,
}

/// Maps the HPET ACPI memory range if an HPET DT was found in the RSDT/XSDT,
/// i.e. if [`ArchInitInfo::hpet_dt`](crate::arch::ArchInitInfo::hpet_dt) is
/// `Some`.
pub fn init() {
    let aif = unsafe { &mut KERNEL_INFO.arch };
    let hpet_region = &mut aif.hpet_region;

    let hpet_phys_region = if let Some(hpet_dt) = aif.hpet_dt {
        println!("[ACPI] Mapping HPET memory.");
        hpet_dt.region_to_map()
    } else {
        println!("[ACPI] No ACPI info region is mapped.");
        return;
    };

    assert_ne!(hpet_phys_region.size(), 0);
    assert_eq!(hpet_phys_region.start % 4096, 0);
    assert_eq!(hpet_phys_region.end % 4096, 0);

    // Ensure that the pages correspond to the same page table.
    assert_eq!(
        hpet_phys_region.start / 4096 / 1024,
        (hpet_phys_region.end / 4096 - 1) / 1024,
        "HPET physical memory region spans across at least one 4 MiB boundary",
    );

    // Place the ACPI region right after the kernel's page table.
    *hpet_region = Some(Region {
        start: (aif.kernel_region.end + 0x400_000 - 1) & !(0x400_000 - 1),
        end: ((aif.kernel_region.end + 0x400_000 - 1) & !(0x400_000 - 1))
            + 0x400_000,
    });
    println!("[ACPI] ACPI region: {:?}", hpet_region.unwrap());

    let kvas = KERNEL_VAS.lock();

    unsafe {
        let pde_idx = (hpet_region.unwrap().start / 4096 / 1024) as usize;
        let pgtbl_virt = &mut *ACPI_PGTBL.lock() as *mut Table;
        kvas.set_pde_addr(pde_idx, pgtbl_virt);
    }

    let start_page = hpet_phys_region.start / 4096;
    let end_page = (hpet_phys_region.end - 1) / 4096 + 1;

    for (i, page) in (start_page..end_page).enumerate() {
        let virt = hpet_region.unwrap().start + i * 4096;
        let phys = page << 12;
        println!("[ACPI] Mapping page 0x{:08X} -> 0x{:08X}.", virt, phys);
        unsafe {
            kvas.map_page(virt as u32, phys as u32);
        }
    }
}
