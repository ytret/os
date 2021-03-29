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

#[derive(Clone, Copy, Debug)]
#[repr(C, packed)]
pub struct OldRsdp {
    signature: [u8; 8],
    checksum: u8,
    oemid: [u8; 6],
    revision: u8,
    pub rsdt_phys_addr: u32,
}

impl OldRsdp {
    pub fn is_valid(&self) -> bool {
        if &self.signature != "RSD PTR ".as_bytes() {
            return false;
        }
        if self.sum_fields() as u8 != 0 {
            return false;
        }
        true
    }

    fn sum_fields(&self) -> usize {
        self.signature.iter().fold(0, |acc, x| acc + *x as usize)
            + self.checksum as usize
            + self.oemid.iter().fold(0, |acc, x| acc + *x as usize)
            + self.revision as usize
            + ((self.rsdt_phys_addr >> 0) & 0xFF) as usize
            + ((self.rsdt_phys_addr >> 8) & 0xFF) as usize
            + ((self.rsdt_phys_addr >> 16) & 0xFF) as usize
            + ((self.rsdt_phys_addr >> 24) & 0xFF) as usize
    }
}

#[derive(Clone, Copy, Debug)]
#[repr(C, packed)]
pub struct NewRsdp {
    old_rsdp: OldRsdp,
    pub length: u32,
    pub xsdt_phys_addr: u64,
    ext_checksum: u8,
    _reserved: [u8; 3],
}

impl NewRsdp {
    pub fn is_valid(&self) -> bool {
        if !self.old_rsdp.is_valid() {
            return false;
        }
        if self.sum_fields() as u8 != 0 {
            return false;
        }
        true
    }

    fn sum_fields(&self) -> usize {
        self.old_rsdp.sum_fields()
            + ((self.length >> 0) & 0xFF) as usize
            + ((self.length >> 8) & 0xFF) as usize
            + ((self.length >> 16) & 0xFF) as usize
            + ((self.length >> 24) & 0xFF) as usize
            + ((self.xsdt_phys_addr >> 0) & 0xFF) as usize
            + ((self.xsdt_phys_addr >> 8) & 0xFF) as usize
            + ((self.xsdt_phys_addr >> 16) & 0xFF) as usize
            + ((self.xsdt_phys_addr >> 24) & 0xFF) as usize
            + ((self.xsdt_phys_addr >> 32) & 0xFF) as usize
            + ((self.xsdt_phys_addr >> 40) & 0xFF) as usize
            + ((self.xsdt_phys_addr >> 48) & 0xFF) as usize
            + ((self.xsdt_phys_addr >> 56) & 0xFF) as usize
            + self.ext_checksum as usize
            + self._reserved.iter().fold(0, |acc, x| acc + *x as usize)
    }
}

#[derive(Clone, Copy, Debug)]
#[repr(C, packed)]
pub struct Sdt {
    pub signature: [u8; 4],
    pub length: u32,
    revision: u8,
    checksum: u8,
    oemid: [u8; 6],
    oemtableid: [u8; 8],
    oemrevision: u32,
    creator_id: u32,
    creator_revision: u32,
}

impl Sdt {
    pub fn sum_fields(&self) -> usize {
        self.signature.iter().fold(0, |acc, x| acc + *x as usize)
            + ((self.length >> 0) & 0xFF) as usize
            + ((self.length >> 8) & 0xFF) as usize
            + ((self.length >> 16) & 0xFF) as usize
            + ((self.length >> 24) & 0xFF) as usize
            + self.revision as usize
            + self.checksum as usize
            + self.oemid.iter().fold(0, |acc, x| acc + *x as usize)
            + self.oemtableid.iter().fold(0, |acc, x| acc + *x as usize)
            + ((self.oemrevision >> 0) & 0xFF) as usize
            + ((self.oemrevision >> 8) & 0xFF) as usize
            + ((self.oemrevision >> 16) & 0xFF) as usize
            + ((self.oemrevision >> 24) & 0xFF) as usize
            + ((self.creator_id >> 0) & 0xFF) as usize
            + ((self.creator_id >> 8) & 0xFF) as usize
            + ((self.creator_id >> 16) & 0xFF) as usize
            + ((self.creator_id >> 24) & 0xFF) as usize
            + ((self.creator_revision >> 0) & 0xFF) as usize
            + ((self.creator_revision >> 8) & 0xFF) as usize
            + ((self.creator_revision >> 16) & 0xFF) as usize
            + ((self.creator_revision >> 24) & 0xFF) as usize
    }
}
