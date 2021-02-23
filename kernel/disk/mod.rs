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
use core::mem::size_of;
use core::ops::Range;

use crate::fs::{ext2, FileSystem};
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

impl Disk {
    pub fn probe_fs(&self) -> Result<KnownFs, ProbeErr> {
        let rwif = self.rw_interface.as_ref();
        let block_size = rwif.block_size();

        // Ext2?  Read the superblock and check the signature.
        let sb_start = 1024;
        let sb_size = size_of::<ext2::Superblock>();
        let blocks_to_read = Range {
            start: sb_start / block_size,
            end: (sb_start + sb_size) / block_size + 1,
        };
        let raw_sb =
            rwif.read_blocks(blocks_to_read.start, blocks_to_read.len())?;
        let offset_in_raw = sb_start % block_size;
        assert!(offset_in_raw + sb_size <= raw_sb.len());
        let sb = unsafe {
            // SAFETY?
            (raw_sb.as_ptr().add(offset_in_raw) as *const ext2::Superblock)
                .read_unaligned()
        };
        if sb.ext2_signature == ext2::EXT2_SIGNATURE {
            println!("[DISK] Found an ext2 signature.");
            return Ok(KnownFs::Ext2);
        }

        println!("[DISK] Unknown file system.");
        Err(ProbeErr::UnknownFs)
    }
}

#[derive(Debug)]
pub enum KnownFs {
    Ext2,
}

#[derive(Debug)]
pub enum ProbeErr {
    UnknownFs,
    ReadErr(ReadErr),
}

impl From<ReadErr> for ProbeErr {
    fn from(err: ReadErr) -> Self {
        ProbeErr::ReadErr(err)
    }
}

kernel_static! {
    pub static ref DISKS: Mutex<Vec<Disk>> = Mutex::new(Vec::new());
}
