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

pub mod ata;

use alloc::rc::Rc;
use alloc::vec;
use alloc::vec::Vec;
use core::cell::RefCell;
use core::mem::size_of;

use crate::block_device;
use crate::fs::{ext2, FileSystem, Mountable, Node, ReadDirErr};
use crate::kernel_static::Mutex;

pub struct Disk {
    pub id: usize,
    pub rw_interface: Rc<dyn ReadWriteInterface>,
    pub file_system: Option<Rc<dyn FileSystem>>,
}

impl Disk {
    pub fn probe_fs(&self) -> Result<KnownFs, ProbeFsErr> {
        // Ext2?  Read the superblock and check the signature.
        let mut raw_sb = vec![0u8; size_of::<ext2::Superblock>()];
        assert_eq!(self.rw_interface.read(1024, &mut raw_sb)?, raw_sb.len());
        let sb = unsafe {
            // SAFETY?
            raw_sb.as_ptr().cast::<ext2::Superblock>().read_unaligned()
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
                let mut raw_sb = [0u8; 1024];
                assert_eq!(rwif.read(sb_offset, &mut raw_sb)?, 1024);
                let sb = unsafe {
                    raw_sb.as_ptr().cast::<ext2::Superblock>().read_unaligned()
                };

                let bs = 1024 * 2usize.pow(sb.log_block_size_minus_10);
                let bgd_offset = bs * (sb_offset / bs + 1);
                let num_bgds = sb.total_num_blocks as usize
                    / sb.block_group_num_blocks as usize;
                let mut raw_bgd =
                    vec![
                        0u8;
                        num_bgds * size_of::<ext2::BlockGroupDescriptor>()
                    ];
                assert_eq!(rwif.read(bgd_offset, &mut raw_bgd)?, raw_bgd.len());
                let ext2 = unsafe {
                    // SAFETY?
                    ext2::Ext2::from_raw(
                        &raw_sb,
                        &raw_bgd,
                        Rc::downgrade(&rwif),
                    )?
                };
                self.file_system = Some(Rc::new(ext2));
                Ok(self.file_system.as_ref().unwrap().root_dir()?)
            }
        }
    }
}

impl block_device::BlockDevice for Disk {
    fn block_size(&self) -> usize {
        self.rw_interface.block_size()
    }

    fn has_block(&self, block_idx: usize) -> bool {
        self.rw_interface.has_block(block_idx)
    }

    fn read_block(
        &self,
        block_idx: usize,
        buf: &mut [u8],
    ) -> Result<usize, block_device::ReadErr> {
        Ok(self.rw_interface.read_block(block_idx, buf)?)
    }

    fn read_blocks(
        &self,
        first_block_idx: usize,
        buf: &mut [u8],
    ) -> Result<usize, block_device::ReadErr> {
        Ok(self.rw_interface.read_blocks(first_block_idx, buf)?)
    }

    fn write_block(
        &self,
        block_idx: usize,
        data: [u8; 512],
    ) -> Result<(), block_device::WriteErr> {
        Ok(self.rw_interface.write_block(block_idx, data)?)
    }

    fn write_blocks(
        &self,
        first_block_idx: usize,
        data: &[u8],
    ) -> Result<(), block_device::WriteErr> {
        Ok(self.rw_interface.write_blocks(first_block_idx, data)?)
    }
}

impl Mountable for Disk {
    fn fs(&self) -> Rc<dyn FileSystem> {
        self.file_system.as_ref().unwrap().clone()
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

pub trait ReadWriteInterface {
    fn block_size(&self) -> usize;
    fn has_block(&self, block_idx: usize) -> bool;

    fn read_block(
        &self,
        block_idx: usize,
        buf: &mut [u8],
    ) -> Result<usize, ReadErr>;
    fn read_blocks(
        &self,
        first_block_idx: usize,
        buf: &mut [u8],
    ) -> Result<usize, ReadErr>;

    fn read(&self, from_byte: usize, buf: &mut [u8]) -> Result<usize, ReadErr> {
        assert_ne!(buf.len(), 0, "cannot read into an empty buffer");
        let to_byte = from_byte + buf.len();

        let from_block =
            (from_byte + self.block_size()) / self.block_size() - 1;
        let to_block = (to_byte + self.block_size()) / self.block_size();
        let num_blocks = to_block - from_block;

        if from_byte % self.block_size() != 0
            || to_byte % self.block_size() != 0
        {
            let mut tmp_buf = vec![0u8; num_blocks * self.block_size()];
            assert_eq!(
                self.read_blocks(from_block, &mut tmp_buf)?,
                tmp_buf.len(),
            );
            tmp_buf.drain(..from_byte % self.block_size());
            tmp_buf.truncate(buf.len());
            buf.copy_from_slice(&tmp_buf);
            Ok(buf.len())
        } else {
            Ok(self.read_blocks(from_block, buf)?)
        }
    }

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
    NoSuchBlock,
    TooMuchBlocks,
    InvalidNumBlocks,
}

#[derive(Debug)]
pub enum WriteErr {
    NoSuchBlock,
    TooMuchBlocks,
    EmptyDataPassed,
}

kernel_static! {
    pub static ref DISKS: Mutex<Vec<Rc<RefCell<Disk>>>> = Mutex::new(Vec::new());
}
