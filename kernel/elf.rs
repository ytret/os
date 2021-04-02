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

use alloc::boxed::Box;
use alloc::string::String;
use alloc::vec::Vec;
use core::mem::size_of;

#[repr(C, packed)]
#[derive(Clone, Copy, Debug)]
pub struct ElfHeader {
    ident: Ident,
    _type: Type,
    machine: Machine,
    version: u32,
    entry: u32,
    phoff: u32,
    shoff: u32,
    flags: u32,
    ehsize: u16,
    phentsize: u16,
    phnum: u16,
    shentsize: u16,
    shnum: u16,
    shstrndx: u16,
}

#[derive(Debug)]
pub enum ElfHeaderErr {
    NotElf,
    UnsupportedArch(u8),
    UnsupportedByteOrder(u8),
    UnsupportedElfVersion(u8),
    InvalidType(u16),
    UnsupportedMachine(u16),
}

impl ElfHeader {
    unsafe fn from_bytes(bytes: &[u8]) -> Result<Self, ElfHeaderErr> {
        let (head, body, _tail) = bytes.align_to::<ElfHeader>();
        assert!(head.is_empty(), "improper alignment of bytes");
        assert!(!body.is_empty(), "improper size of bytes");
        let header = body[0];

        if header.ident.must_be_0x7f != 0x7f
            || header.ident.must_be_0x45 != 0x45
            || header.ident.must_be_0x4c != 0x4C
            || header.ident.must_be_0x46 != 0x46
        {
            return Err(ElfHeaderErr::NotElf);
        }
        if header.ident.arch != Arch::Bit32 {
            return Err(ElfHeaderErr::UnsupportedArch(header.ident.arch as u8));
        }
        if header.ident.byte_order != ByteOrder::LittleEndian {
            return Err(ElfHeaderErr::UnsupportedByteOrder(
                header.ident.byte_order as u8,
            ));
        }
        if header.ident.elf_version != ELF_VERSION {
            return Err(ElfHeaderErr::UnsupportedElfVersion(
                header.ident.elf_version as u8,
            ));
        }

        if { header._type } != Type::ExecutableFile {
            return Err(ElfHeaderErr::InvalidType(header._type as u16));
        }
        if { header.machine } != Machine::X86 {
            return Err(ElfHeaderErr::UnsupportedMachine(
                header.machine as u16,
            ));
        }

        Ok(header)
    }

    fn section_header_idx(&self, section_num: usize) -> usize {
        self.shoff as usize + section_num * size_of::<SectionHeader>()
    }

    fn program_header_idx(&self, ph_num: usize) -> usize {
        self.phoff as usize + ph_num * size_of::<ProgHeader>()
    }
}

#[repr(C, packed)]
#[derive(Clone, Copy, Debug)]
struct Ident {
    must_be_0x7f: u8,
    must_be_0x45: u8, // E
    must_be_0x4c: u8, // L
    must_be_0x46: u8, // F
    arch: Arch,
    byte_order: ByteOrder,
    elf_version: u8,
    osabi: u8,
    abiversion: u8,
    padding: [u8; 7],
}

#[allow(dead_code)]
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq)]
enum Arch {
    Bit32 = 1,
    Bit64 = 2,
}

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq)]
enum ByteOrder {
    LittleEndian = 1,
}

const ELF_VERSION: u8 = 1;

#[allow(dead_code)]
#[repr(u16)]
#[derive(Clone, Copy, Debug, PartialEq)]
enum Type {
    None = 0,
    RelocatableFile = 1,
    ExecutableFile = 2,
}

#[repr(u16)]
#[derive(Clone, Copy, Debug, PartialEq)]
enum Machine {
    X86 = 3,
}

#[repr(C, packed)]
#[derive(Clone, Copy, Debug)]
struct SectionHeader {
    name: u32,
    _type: SectionType,
    flags: SectionAttr,
    addr: u32,
    offset: u32,
    size: u32,
    link: u32,
    info: u32,
    addr_align: u32,
    entry_size: u32,
}

impl SectionHeader {
    unsafe fn from_bytes(bytes: &[u8]) -> Self {
        let (head, body, _tail) = bytes.align_to::<SectionHeader>();
        assert!(head.is_empty(), "improper alignment of bytes");
        assert!(!body.is_empty(), "improper size of bytes");
        body[0]
    }
}

#[allow(dead_code)]
#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq)]
enum SectionType {
    Null = 0,
    ProgBits = 1,
    SymbolTable = 2,
    StringTable = 3,
    RelocationWithAddend = 4,
    NoBits = 8,
    Relocation = 9,
}

bitflags! {
    #[repr(u32)]
    enum SectionAttr {
        Writable = 1,
        Alloc = 2,
    }
}

#[repr(C, packed)]
#[derive(Clone, Copy, Debug)]
struct ProgHeader {
    _type: ProgHeaderType,
    offset: u32,
    vaddr: u32,
    _skip: u32,
    filesz: u32,
    memsz: u32,
    flags: u32,
    align: u32,
}

impl ProgHeader {
    unsafe fn from_bytes(bytes: &[u8]) -> Self {
        let (head, body, _tail) = bytes.align_to::<ProgHeader>();
        assert!(head.is_empty(), "improper alignment of bytes");
        assert!(!body.is_empty(), "improper size of bytes");
        body[0]
    }
}

#[allow(dead_code)]
#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq)]
enum ProgHeaderType {
    Null = 0,
    Load = 1,
    Dynamic = 2,
    Interp = 3,
    Note = 4,
    Tls = 7,
}

// Above are standard ELF structures.  Below are structures used by the kernel.

#[derive(Clone, Debug)]
pub struct ElfObj {
    pub sections: Vec<SectionInfo>,
    pub program_segments: Vec<ProgSegment>,
    pub entry_point: usize,
}

#[derive(Debug)]
pub enum ElfObjErr {
    ElfHeaderErr(ElfHeaderErr),
}

impl From<ElfHeaderErr> for ElfObjErr {
    fn from(e: ElfHeaderErr) -> Self {
        ElfObjErr::ElfHeaderErr(e)
    }
}

impl ElfObj {
    /// Constructs an ELF object using the giving byte `feeder`.
    ///
    /// The feeder's first argument is a byte offset in the raw ELF, the second
    /// argument is the number of bytes to read.  If the second argument is
    /// zero, it means reading until a null byte.
    pub unsafe fn from_feeder<F>(feeder: F) -> Result<Self, ElfObjErr>
    where
        F: Fn(usize, usize) -> Box<[u8]>,
    {
        let elf_header =
            ElfHeader::from_bytes(&feeder(0, size_of::<ElfHeader>()))?;

        let names_section = SectionHeader::from_bytes(&feeder(
            elf_header.section_header_idx(elf_header.shstrndx as usize),
            size_of::<SectionHeader>(),
        ));
        let names_section_start = names_section.offset as usize;

        Ok(ElfObj {
            sections: {
                let mut vec = Vec::new();
                for i in 0..elf_header.shnum as usize {
                    let sh = SectionHeader::from_bytes(&feeder(
                        elf_header.section_header_idx(i),
                        size_of::<SectionHeader>(),
                    ));

                    vec.push(SectionInfo {
                        name: if elf_header.shstrndx != 0 && sh.name != 0 {
                            let name_start =
                                names_section_start + sh.name as usize;
                            let name_bytes = feeder(name_start, 0);
                            Some(
                                String::from_utf8(name_bytes.to_vec()).unwrap(),
                            )
                        } else {
                            None
                        },
                        offset: sh.offset as usize,
                        size: sh.size as usize,
                    });
                }
                vec
            },
            program_segments: {
                let mut vec = Vec::new();
                for i in 0..elf_header.phnum as usize {
                    let ph = ProgHeader::from_bytes(&feeder(
                        elf_header.program_header_idx(i),
                        size_of::<ProgHeader>(),
                    ));
                    vec.push(ProgSegment::from_prog_header(&ph));
                }
                vec
            },
            entry_point: elf_header.entry as usize,
        })
    }

    // pub unsafe fn from_bytes(data: &[u8]) -> Result<Self, ElfObjErr> {
    //     let elf_header = ElfHeader::from_bytes(data)?;
    //     Ok(ElfObj {
    //         sections: {
    //             let mut vec = Vec::new();
    //             for i in 0..elf_header.shnum as usize {
    //                 vec.push(SectionInfo::from_bytes(data, &elf_header, i));
    //             }
    //             vec
    //         },
    //         program_segments: {
    //             let mut vec = Vec::new();
    //             for i in 0..elf_header.phnum as usize {
    //                 vec.push(ProgSegment::from_bytes(data, &elf_header, i));
    //             }
    //             vec
    //         },
    //         entry_point: elf_header.entry as usize,
    //     })
    // }
}

#[derive(Clone, Debug)]
pub struct SectionInfo {
    name: Option<String>,
    offset: usize,
    size: usize,
}

#[derive(Clone, Debug)]
pub struct ProgSegment {
    pub _type: ProgSegmentType,

    pub in_file_at: usize,
    pub in_file_size: usize,

    pub in_mem_at: usize,
    pub in_mem_size: usize,
}

impl ProgSegment {
    unsafe fn from_prog_header(ph: &ProgHeader) -> Self {
        let _type = { ph._type };
        ProgSegment {
            _type: match _type {
                ProgHeaderType::Load => ProgSegmentType::Load,
                ProgHeaderType::Tls => ProgSegmentType::Tls,
                _ => unimplemented!("ProgHeaderType::{:?}", _type as u32),
            },

            in_file_at: ph.offset as usize,
            in_file_size: ph.filesz as usize,

            in_mem_at: ph.vaddr as usize,
            in_mem_size: ph.memsz as usize,
        }
    }
}

#[derive(PartialEq, Clone, Debug)]
pub enum ProgSegmentType {
    Load,
    Tls,
}
