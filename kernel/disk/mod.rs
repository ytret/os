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

pub mod ata;

use alloc::boxed::Box;
use alloc::vec::Vec;

use crate::kernel_static::Mutex;

pub trait Disk {
    fn has_sector(&self, sector_idx: usize) -> bool;

    fn read_sector(&self, sector_idx: usize) -> Result<Box<[u8]>, ReadErr>;
    fn read_sectors(
        &self,
        first_sector_idx: usize,
        num_sectors: usize,
    ) -> Result<Box<[u8]>, ReadErr>;

    fn write_sector(
        &self,
        sector_idx: usize,
        data: [u8; 512],
    ) -> Result<(), WriteErr>;
    fn write_sectors(
        &self,
        first_sector_idx: usize,
        data: &[u8],
    ) -> Result<(), WriteErr>;
}

#[derive(Debug)]
pub enum ReadErr {
    DiskUnavailable,
    NoSuchSector,
    TooMuchSectors,
    ZeroNumSectors,
}

#[derive(Debug)]
pub enum WriteErr {
    DiskUnavailable,
    NoSuchSector,
    TooMuchSectors,
    EmptyDataPassed,
}

kernel_static! {
    pub static ref DISKS: Mutex<Vec<Box<dyn Disk>>> = Mutex::new(Vec::new());
}
