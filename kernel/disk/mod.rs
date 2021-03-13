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

use crate::fs::{ext2, FileSystem, Node, ReadDirErr};
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
    fn read(&self, from_byte: usize, len: usize) -> Result<Box<[u8]>, ReadErr>;

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
    pub id: usize,
    pub rw_interface: Rc<Box<dyn ReadWriteInterface>>,
    pub file_system: Option<Rc<Box<dyn FileSystem>>>,
}

impl Disk {
    pub fn probe_fs(&self) -> Result<KnownFs, ProbeFsErr> {
        // Ext2?  Read the superblock and check the signature.
        let raw_sb = self
            .rw_interface
            .read(1024, size_of::<ext2::Superblock>())?;
        let sb = unsafe {
            // SAFETY?
            (raw_sb.as_ptr() as *const ext2::Superblock).read_unaligned()
        };
        if sb.ext2_signature == ext2::EXT2_SIGNATURE {
            println!("[DISK] Found an ext2 signature.");
            return Ok(KnownFs::Ext2);
        }

        println!("[DISK] Unknown file system.");
        Err(ProbeFsErr::UnknownFs)
    }

    pub fn try_init_fs(&mut self) -> Result<Node, TryInitFsErr> {
        if self.file_system.is_some() {
            return Err(TryInitFsErr::AlreadyHasFs);
        }

        match self.probe_fs()? {
            KnownFs::Ext2 => {
                let rwif = &self.rw_interface;
                let sb_offset = 1024;
                let raw_sb = rwif.read(sb_offset, 1024)?;
                let raw_bgd = unsafe {
                    // SAFETY?
                    let sb = (raw_sb.as_ptr() as *const ext2::Superblock)
                        .read_unaligned();
                    let bs = 1024 * 2usize.pow(sb.log_block_size_minus_10);
                    let bgd_offset = bs * (sb_offset / bs + 1);
                    let num_bgds = sb.total_num_blocks as usize
                        / sb.block_group_num_blocks as usize;
                    rwif.read(
                        bgd_offset,
                        num_bgds * size_of::<ext2::BlockGroupDescriptor>(),
                    )?
                };
                let ext2 = unsafe {
                    // SAFETY?
                    ext2::Ext2::from_raw(
                        &raw_sb,
                        &raw_bgd,
                        Rc::downgrade(&rwif),
                    )?
                };
                self.file_system = Some(Rc::new(Box::new(ext2)));
                Ok(self.file_system.as_ref().unwrap().root_dir()?)
            }
        }
    }
}

#[derive(Debug)]
pub enum KnownFs {
    Ext2,
}

#[derive(Debug)]
pub enum ProbeFsErr {
    UnknownFs,
    ReadErr(ReadErr),
}

impl From<ReadErr> for ProbeFsErr {
    fn from(err: ReadErr) -> Self {
        ProbeFsErr::ReadErr(err)
    }
}

#[derive(Debug)]
pub enum TryInitFsErr {
    AlreadyHasFs,
    ProbeFsErr(ProbeFsErr),
    InitExt2Err(ext2::FromRawErr),
    ReadErr(ReadErr),
    ReadRootDirErr(ReadDirErr),
}

impl From<ProbeFsErr> for TryInitFsErr {
    fn from(err: ProbeFsErr) -> Self {
        TryInitFsErr::ProbeFsErr(err)
    }
}

impl From<ext2::FromRawErr> for TryInitFsErr {
    fn from(err: ext2::FromRawErr) -> Self {
        TryInitFsErr::InitExt2Err(err)
    }
}

impl From<ReadErr> for TryInitFsErr {
    fn from(err: ReadErr) -> Self {
        TryInitFsErr::ReadErr(err)
    }
}

impl From<ReadDirErr> for TryInitFsErr {
    fn from(err: ReadDirErr) -> Self {
        TryInitFsErr::ReadRootDirErr(err)
    }
}

kernel_static! {
    pub static ref DISKS: Mutex<Vec<Rc<Disk>>> = Mutex::new(Vec::new());
}
