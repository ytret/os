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
use alloc::rc::Rc;
use alloc::vec::Vec;

use crate::fs::FileSystem;
use crate::kernel_static::Mutex;

pub trait ReadWriteInterface {
    fn block_size(&self) -> usize;
    fn has_block(&self, block_idx: usize) -> bool;

    fn read_block(&self, block_idx: usize) -> Result<Box<[u8]>, ReadErr>;
    fn read_blocks(
        &self,
        first_block_idx: usize,
        num_blocks: usize,
    ) -> Result<Box<[u8]>, ReadErr>;

    fn write_block(
        &self,
        block_idx: usize,
        data: [u8; 512],
    ) -> Result<(), WriteErr>;
    fn write_blocks(
        &self,
        first_block_idx: usize,
        data: &[u8],
    ) -> Result<(), WriteErr>;
}

#[derive(Debug)]
pub enum ReadErr {
    BusUnavailable,
    NoSuchBlock,
    TooMuchBlocks,
    ZeroNumBlocks,
    Other(&'static str),
}

#[derive(Debug)]
pub enum WriteErr {
    BusUnavailable,
    NoSuchBlock,
    TooMuchBlocks,
    EmptyDataPassed,
    Other(&'static str),
}

pub struct Disk {
    pub rw_interface: Rc<Box<dyn ReadWriteInterface>>,
    pub file_system: Option<Box<dyn FileSystem>>,
}

kernel_static! {
    pub static ref DISKS: Mutex<Vec<Disk>> = Mutex::new(Vec::new());
}
