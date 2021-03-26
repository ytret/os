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

#![allow(dead_code)]

use core::fmt;
use core::mem;
use core::slice;
use core::str;

use crate::arch::acpi::{hpet, sdt};
use crate::memory_region;
use crate::KERNEL_INFO;

macro_rules! type_enum {
    (#[repr($REPR:ident)] enum $N:ident { Reserved = $R:literal, $($V:ident = $D:literal,)* }) => {
        #[repr($REPR)]
        enum $N {
            Reserved = $R,
            $($V = $D,)*
        }

        impl From<$REPR> for $N {
            fn from(raw: $REPR) -> Self {
                match raw {
                    $($D => $N::$V,)*
                    _ => $N::Reserved,
                }
            }
        }

        impl fmt::Display for $N {
            fn fmt(&self, f: &mut fmt::Formatter) -> core::fmt::Result {
                write!(
                    f,
                    "{}",
                    match self {
                        Self::Reserved => "Reserved",
                        $(Self::$V => stringify!($V),)*
                    }
                )
            }
        }
    }
}

struct VariedSizeField;

// The tags are defined in the same order as in the standard.

#[repr(C, packed)]
pub struct BootInfo {
    total_size: u32,
    reserved: u32,
}

#[repr(C, packed)]
struct BasicMemoryInfo {
    tag_type: u32, // 4
    tag_size: u32,
    mem_lower: u32,
    mem_upper: u32,
}

#[repr(C, packed)]
struct BiosBootDevice {
    tag_type: u32, // 5
    tag_size: u32,
    bios_dev: u32,
    partition: u32,
    subpartition: u32,
}

#[repr(C, packed)]
struct BootCommandLine {
    tag_type: u32, // 1
    tag_size: u32,
    string: [u8; 0],
}

#[repr(C, packed)]
struct Module {
    tag_type: u32, // 3
    tag_size: u32,
    mod_start: u32,
    mod_end: u32,
    string: [u8; 0],
}

#[repr(C, packed)]
struct ElfSymbols {
    tag_type: u32, // 9
    tag_size: u32,
    num: u16,
    entsize: u16,
    shndx: u16,
    reserved: u16,
    section_headers: VariedSizeField,
}

#[repr(C, packed)]
struct MemoryMap {
    tag_type: u32, // 6
    tag_size: u32,
    entry_size: u32,
    entry_version: u32,
    entries: VariedSizeField,
}

#[repr(C, packed)]
struct MemoryMapEntry {
    base_addr: u64,
    length: u64,
    region_type: u32, // see MemoryMapRegionType below
    reserved: u32,
}

type_enum! {
    #[repr(u32)]
    enum MemoryMapRegionType {
        Reserved = 4,
        Available = 1,
        AcpiInfo = 3,
        Defective = 5,
    }
}

#[repr(C, packed)]
struct BootloaderName {
    tag_type: u32, // 2
    tag_size: u32,
    string: [u8; 0],
}

#[repr(C, packed)]
struct ApmTable {
    tag_type: u32, // 10
    tag_size: u32,
    version: u16,
    cseg: u16,
    offset: u32,
    cseg_16: u16,
    dseg: u16,
    flags: u16,
    cseg_len: u16,
    cseg_16_len: u16,
    dseg_len: u16,
}

#[repr(C, packed)]
struct VbeInfo {
    tag_type: u32, // 7
    tag_size: u32,
    mode: u16,
    interface_seg: u16,
    interface_off: u16,
    interface_len: u16,
    control_info: [u8; 512],
    mode_info: [u8; 256],
}

#[repr(C, packed)]
struct FramebufferInfo {
    tag_type: u32, // 8
    tag_size: u32,
    addr: u64,
    pitch: u32,
    width: u32,
    height: u32,
    bpp: u8,
    _type: u8,
    reserved: u8,
    color_info: VariedSizeField,
}

type_enum! {
    #[repr(u8)]
    enum FramebufferType {
        Reserved = 3,
        IndexedColor = 0,
        RgbColor = 1,
        EgaText = 2,
    }
}

#[repr(C, packed)]
struct FramebufferIndexedColorInfo {
    palette_num_colors: u32,
    palette: VariedSizeField,
}

#[repr(C, packed)]
struct FramebufferPaletteColorDescriptor {
    red_value: u8,
    green_value: u8,
    blue_value: u8,
}

#[repr(C, packed)]
struct FramebufferRgbColorInfo {
    red_field_pos: u8,
    red_mask_size: u8,
    green_field_pos: u8,
    green_mask_size: u8,
    blue_field_pos: u8,
    blue_mask_size: u8,
}

#[repr(C, packed)]
struct Efi32BitSystemTablePointer {
    tag_type: u32, // 11
    tag_size: u32,
    pointer: u32,
}

#[repr(C, packed)]
struct Efi64BitSystemTablePointer {
    tag_type: u32, // 12
    tag_size: u32,
    pointer: u64,
}

#[repr(C, packed)]
struct SmbiosTables {
    tag_type: u32, // 13
    tag_size: u32,
    major_version: u8,
    minor_version: u8,
    reserved: [u8; 6],
    smbios_tables: VariedSizeField,
}

#[repr(C, packed)]
struct AcpiOldRsdp {
    tag_type: u32, // 14
    tag_size: u32,
    rsdpv1: VariedSizeField,
}

#[repr(C, packed)]
struct AcpiNewRsdp {
    tag_type: u32, // 15
    tag_size: u32,
    rsdpv2: VariedSizeField,
}

#[repr(C, packed)]
struct NetworkingInformation {
    tag_type: u32, // 16
    tag_size: u32,
    dchp_ack: VariedSizeField,
}

#[repr(C, packed)]
struct EfiMemoryMap {
    tag_type: u32, // 17
    tag_size: u32,
    descriptor_size: u32,
    descriptor_version: u32,
    efi_memory_map: VariedSizeField,
}

#[repr(C, packed)]
struct EfiBootServicesNotTerminated {
    tag_type: u32, // 18
    tag_size: u32,
}

#[repr(C, packed)]
struct Efi32BitImageHandlePointer {
    tag_type: u32, // 19
    tag_size: u32,
    pointer: u32,
}

#[repr(C, packed)]
struct Efi64BitImageHandlePointer {
    tag_type: u32, // 20
    tag_size: u32,
    pointer: u64,
}

#[repr(C, packed)]
struct ImageLoadBasePhysicalAddress {
    tag_type: u32, // 21
    tag_size: u32,
    load_base_addr: u32,
}

fn str_from_ascii(ptr: &[u8], size: u32) -> &str {
    let slice = unsafe {
        slice::from_raw_parts(ptr as *const _ as *const u8, size as usize - 1)
    };
    for (pos, ch) in slice.iter().enumerate() {
        if ch & (1 << 7) != 0 {
            panic!("str_from_ascii: non-ASCII character at {}", pos);
        }
    }
    str::from_utf8(slice).unwrap()
}

pub unsafe fn parse(boot_info: *const BootInfo) {
    let mut ptr = boot_info as *const u8;

    let bi = &*(ptr as *const BootInfo);
    println!(
        "Multiboot information is at 0x{:08X}, total size: {} bytes",
        ptr as u32, bi.total_size,
    );
    ptr = ptr.offset(8);

    let mut num_tags = 0;
    loop {
        assert!(num_tags < 32, "too many tags");
        assert_eq!(ptr as u32 % 8, 0, "tag address is not aligned at 8 bytes");

        let tag_type: u32 = *ptr.cast();
        let tag_size: u32 = *ptr.cast::<u32>().offset(1);
        assert!(tag_size >= 8, "tag_size is less than 8 bytes");

        // Break here so the type and size don't get printed.
        if tag_type == 0 && tag_size == 8 {
            break;
        }

        print!("<{:02}:", tag_type);
        match tag_size {
            size if size < 1000 => {
                print!("{:03}> ", size);
            }
            size if 1000 <= size && size < 2 * 1024 => {
                print!(" 1K> ");
            }
            size if 2 * 1024 <= size => {
                print!("{:2}K> ", size / 1024);
            }
            _ => unreachable!(),
        }

        match tag_type {
            1 => {
                let tag = &*(ptr as *const BootCommandLine);
                println!(
                    "Boot command line: {:?}",
                    str_from_ascii(&tag.string, tag.tag_size - 8)
                );
            }
            2 => {
                let tag = &*(ptr as *const BootloaderName);
                println!(
                    "Bootloader name: {}",
                    str_from_ascii(&tag.string, tag.tag_size - 8)
                );
            }
            3 => {
                let tag = &*(ptr as *const Module);
                println!(
                    "Module: {}: start: 0x{:08X}, end: 0x{:08X}",
                    str_from_ascii(&tag.string, tag.tag_size - 16),
                    tag.mod_start,
                    tag.mod_end,
                );
            }
            4 => {
                let tag = &*(ptr as *const BasicMemoryInfo);
                println!(
                    "Basic memory info: lower: {} KiB, upper: {} KiB",
                    tag.mem_lower, tag.mem_upper,
                );
            }
            5 => {
                let tag = &*(ptr as *const BiosBootDevice);
                println!(
                    "BIOS boot device: drive num {}, partition: {}, \
                     subpartition: {}",
                    tag.bios_dev, tag.partition as i32, tag.subpartition as i32,
                );
            }
            6 => {
                let tag = &*(ptr as *const MemoryMap);
                let num_entries = (tag.tag_size - 16) / tag.entry_size;
                println!(
                    "Memory map: entry size: {}, entry version: {}, \
                     entries: {}",
                    tag.entry_size, tag.entry_version, num_entries,
                );
                let mut i = 0;
                let mut added_to_info = 0;
                while i < num_entries {
                    let entry = &*((&tag.entries as *const _ as *const u8)
                        .add((i * tag.entry_size) as usize)
                        as *const MemoryMapEntry);
                    let start = entry.base_addr;
                    let length = entry.length;
                    let _type = MemoryMapRegionType::from(entry.region_type);
                    print!(
                        "         0x{:08X}_{:08X}..0x{:08X}_{:08X}: {}",
                        (start >> 32) & 0xFFFFFFFF,
                        (start >> 00) & 0xFFFFFFFF,
                        ((start + length) >> 32) & 0xFFFFFFFF,
                        ((start + length) >> 00) & 0xFFFFFFFF,
                        _type,
                    );
                    if start >> 32 != 0 || (start + length) >> 32 != 0 {
                        println!(", ignored");
                        i += 1;
                        continue;
                    }
                    match _type {
                        MemoryMapRegionType::Available
                            if added_to_info
                                < KERNEL_INFO
                                    .available_memory_regions
                                    .len() =>
                        {
                            KERNEL_INFO.available_memory_regions
                                [added_to_info] = memory_region::Region {
                                start: start as usize,
                                end: start as usize + length as usize,
                            };
                            added_to_info += 1;
                        }
                        _ => {}
                    }
                    println!("");
                    i += 1;
                }
            }
            7 => {
                let tag = &*(ptr as *const VbeInfo);
                println!(
                    "VBE info: mode: {}, interface seg: {}, \
                     interface off: {}, interface len: {}",
                    tag.mode,
                    tag.interface_seg,
                    tag.interface_off,
                    tag.interface_len,
                );
            }
            8 => {
                let tag = &*(ptr as *const FramebufferInfo);
                println!(
                    "Framebuffer info: at phys: 0x{:08X}, pitch: {}, \
                     {}x{}, bpp: {}, type: {}",
                    tag.addr as u32,
                    tag.pitch,
                    tag.width,
                    tag.height,
                    tag.bpp,
                    FramebufferType::from(tag._type),
                );
            }
            9 => {
                let tag = &*(ptr as *const ElfSymbols);
                println!(
                    "ELF symbols at 0x{:08X}: num: {}, entsize: {}, shndx: {}",
                    tag as *const _ as u32, tag.num, tag.entsize, tag.shndx,
                );
            }
            10 => {
                let tag = &*(ptr as *const ApmTable);
                println!(
                    "APM table: v{}, cseg: 0x{:04X}, offset: 0x{:08X}, \
                     flags: {}, len: {}",
                    tag.version, tag.cseg, tag.offset, tag.flags, tag.cseg_len
                );
            }
            11 => {
                let tag = &*(ptr as *const Efi32BitSystemTablePointer);
                println!(
                    "EFI 32-bit system table pointer: 0x{:08X}",
                    tag.pointer,
                );
            }
            12 => {
                let tag = &*(ptr as *const Efi64BitSystemTablePointer);
                println!(
                    "EFI 64-bit system table pointer: 0x{:08X}_{:08X}",
                    (tag.pointer >> 32) & 0xFFFFFFFF,
                    (tag.pointer >> 00) & 0xFFFFFFFF,
                );
            }
            13 => {
                let tag = &*(ptr as *const SmbiosTables);
                println!(
                    "SMBIOS tables: v{}.{}",
                    tag.major_version, tag.minor_version,
                );
            }
            14 => {
                let tag = &*(ptr as *const AcpiOldRsdp);
                println!("ACPI old RSDP");
                assert_eq!(
                    (tag.tag_size - 8) as usize,
                    mem::size_of::<sdt::OldRsdp>(),
                );

                let rsdp = (&tag.rsdpv1 as *const _ as *const sdt::OldRsdp)
                    .read_unaligned();
                // println!("{:#X?}", rsdp);
                assert!(rsdp.is_valid(), "invalid RSDP");

                let rsdt =
                    (rsdp.rsdt_phys_addr as *const sdt::Sdt).read_unaligned();
                // println!("{:#X?}", rsdt);

                let num_sdts =
                    (rsdt.length as usize - mem::size_of::<sdt::Sdt>()) / 4;
                let sdt_ptrs = core::slice::from_raw_parts(
                    (rsdp.rsdt_phys_addr as usize + mem::size_of::<sdt::Sdt>())
                        as *const *const sdt::Sdt,
                    num_sdts,
                );

                let rsdt_sum = rsdt.sum_fields()
                    + sdt_ptrs.iter().fold(0, |acc, x| {
                        acc + ((*x as u32 >> 0) & 0xFF) as usize
                            + ((*x as u32 >> 8) & 0xFF) as usize
                            + ((*x as u32 >> 16) & 0xFF) as usize
                            + ((*x as u32 >> 24) & 0xFF) as usize
                    });
                assert_eq!(rsdt_sum as u8, 0, "invalid RSDT");

                for sdt_ptr in sdt_ptrs {
                    let sdt = sdt_ptr.read_unaligned();
                    let name = core::str::from_utf8(&sdt.signature).unwrap();
                    println!(
                        "{} at 0x{:08X}, length: {} bytes",
                        name, *sdt_ptr as usize, sdt.length,
                    );

                    if name == "HPET" {
                        let hpet_dt = sdt_ptr
                            .add(1)
                            .cast::<hpet::HpetDt>()
                            .read_unaligned();
                        KERNEL_INFO.arch_init_info.hpet_dt = Some(hpet_dt);
                    }
                }

                // KERNEL_INFO.arch_init_info.old_rsdp = Some();
            }
            15 => {
                //let tag = &*(ptr as *const AcpiNewRsdp);
                println!("ACPI new RSDP");
            }
            16 => {
                //let tag = &*(ptr as *const NetworkingInformation);
                println!("Networking information");
            }
            17 => {
                let tag = &*(ptr as *const EfiMemoryMap);
                println!(
                    "EFI memory map: descriptor size: {}, \
                     descriptor version: {}",
                    tag.descriptor_size, tag.descriptor_version,
                );
            }
            18 => {
                //let tag = &*(ptr as *const EfiBootServicesNotTerminated);
                println!("EFI boot services not terminated");
            }
            19 => {
                let tag = &*(ptr as *const Efi32BitImageHandlePointer);
                println!(
                    "EFI 32-bit image handle pointer: 0x{:08X}",
                    tag.pointer,
                );
            }
            20 => {
                let tag = &*(ptr as *const Efi64BitImageHandlePointer);
                println!(
                    "EFI 64-bit image handle pointer: 0x{:08X}_{:08X}",
                    (tag.pointer >> 32) & 0xFFFFFFFF,
                    (tag.pointer >> 00) & 0xFFFFFFFF,
                );
            }
            21 => {
                let tag = &*(ptr as *const ImageLoadBasePhysicalAddress);
                println!(
                    "Image load base physical address: 0x{:08X}",
                    tag.load_base_addr,
                );
            }
            _ => {
                println!("Ignoring unknown tag");
            }
        }

        ptr = ptr.add(tag_size as usize);
        ptr = ptr.add(ptr.align_offset(8)); // 8 bytes
        num_tags += 1;
    }

    let actual_size = ptr as u32 + 8 - boot_info as u32; // 8 is for the end tag
    println!("Actual MBI size: {} bytes", actual_size);
    assert_eq!(
        bi.total_size, actual_size,
        "declared and actual MBI sizes are different"
    );
}
