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

use alloc::boxed::Box;
use alloc::rc::Weak;
use alloc::string::String;
use alloc::vec::Vec;
use core::convert::TryFrom;
use core::fmt;
use core::mem::{align_of, size_of};
use core::ops::Range;
use core::slice;

use super::{DirEntryContent, Directory, FileSystem, ReadDirErr, ReadFileErr};
use crate::bitflags::BitFlags;
use crate::disk;

#[allow(dead_code)]
#[repr(C, packed)]
pub struct Superblock {
    total_num_inodes: u32,
    total_num_blocks: u32,
    num_reserved_blocks: u32,
    total_num_unallocated_blocks: u32,
    total_num_unallocated_inodes: u32,
    block_num_of_superblock: u32,
    pub log_block_size_minus_10: u32,
    log_fragment_size_minus_10: u32,
    block_group_num_blocks: u32,
    block_group_num_fragments: u32,
    block_group_num_inodes: u32,
    last_mount_time: u32,
    last_written_time: u32,
    num_mounts_since_consistency_check: u16,
    allowed_num_mounts_since_consistency_check: u16,
    pub ext2_signature: u16,
    fs_state: FsState,
    error_handling_method: ErrorHandlingMethod,
    version_minor: u16,
    time_of_consistency_check: u32,
    interval_force_consistency_check: u32,
    os_id: OsId,
    version_major: u32,
    user_id_can_use_reserved_blocks: u16,
    group_id_can_use_reserved_blocks: u16,
}

pub const EXT2_SIGNATURE: u16 = 0xEF53;

#[allow(dead_code)]
#[repr(u16)]
enum FsState {
    IsClean = 1,
    HasErrors = 2,
}

#[allow(dead_code)]
#[repr(u16)]
enum ErrorHandlingMethod {
    Ignore = 1,
    MountAsReadOnly = 2,
    KernelPanic = 3,
}

#[allow(dead_code)]
#[repr(u32)]
enum OsId {
    Linux = 0,
    GnuHurd = 1,
    Masix = 2,
    FreeBsd = 3,
    OtherBsdDescendants = 4,
}

#[allow(dead_code)]
#[repr(C, packed)]
struct ExtendedSuperblock {
    first_nonreserved_inode: u32,
    inode_size: u16,
    superblock_backup_block_group: u16,
    optional_features: u32,
    required_features: u32,
    read_only_features: u32,
    fs_id: u128,
    volume_name: [u8; 16],     // C-style string
    last_mount_path: [u8; 64], // C-style string
    compression_algorithms_used: u32,
    num_prealloc_blocks_for_file: u8,
    num_prealloc_blocks_for_dir: u8,
    _unused: u16,
    journal_id: [u8; 16], // C-style string
    journal_inode: u32,
    journal_device: u32,
    orphan_inode_list_head: u32,
}

bitflags! {
    #[repr(u32)]
    enum OptionalFeature {
        PreallocForDir = 0x01,
        AfsServerInodesExist = 0x02,
        FsHasJournal = 0x04,
        InodesWithExtAttr = 0x08,
        FsCanResize = 0x10,
        DirsUseHashIdx = 0x20,
    }
}

bitflags! {
    #[repr(u32)]
    pub enum RequiredFeature {
        Compression = 0x01,
        DirsWithType = 0x02,
        FsNeedsToReplayJournal = 0x04,
        FsUsesJournalDevice = 0x08,
    }
}

bitflags! {
    #[repr(u32)]
    enum ReadOnlyFeature {
        SparseSuperblocksAndBgdTables = 0x01,
        FileSize64Bit = 0x02,
        DirContentsInBinaryTree = 0x04,
    }
}

#[allow(dead_code)]
#[derive(Clone, Copy, Debug)]
#[repr(C, packed)]
pub struct BlockGroupDescriptor {
    block_usage_bitmap_block_addr: u32,
    inode_usage_bitmap_block_addr: u32,
    inode_table_start_block_addr: u32,
    num_unalloc_blocks: u16,
    num_unalloc_inodes: u16,
    num_dirs: u16,
    // unused_18: u32,
    // unused_22: u32,
    // unused_26: u32,
    // unused_30: u16,
}

#[allow(dead_code)]
#[derive(Clone, Copy, Debug)]
#[repr(C, packed)]
pub struct Inode {
    type_and_permissions: u16,
    user_id: u16,
    size: u32, // if ReadOnlyFeature::FileSize64Bit, these are the bits 0..31
    last_access_time: u32,
    creation_time: u32,
    last_modification_time: u32,
    deletion_time: u32,
    group_id: u16,
    count_hard_links: u16, // if 0, the data blocks are marked as unallocated
    count_disk_sectors: u32,
    flags: u32,
    os_specific_1: u32,
    direct_block_ptr_0: u32,
    direct_block_ptr_1: u32,
    direct_block_ptr_2: u32,
    direct_block_ptr_3: u32,
    direct_block_ptr_4: u32,
    direct_block_ptr_5: u32,
    direct_block_ptr_6: u32,
    direct_block_ptr_7: u32,
    direct_block_ptr_8: u32,
    direct_block_ptr_9: u32,
    direct_block_ptr_10: u32,
    direct_block_ptr_11: u32,
    singly_indirect_block_ptr: u32,
    doubly_indirect_block_ptr: u32,
    triply_indirect_block_ptr: u32,
    generation_number: u32,
    extended_attr_block: u32,  // if major version >= 1
    file_size_bits_32_63: u32, // if ReadOnlyFeature::FileSize64Bit
    fragment_block_addr: u32,
    os_specific_2: [u8; 12],
}

impl Inode {
    fn _type(&self) -> InodeType {
        let raw = (self.type_and_permissions >> 12) & 0b1111;
        InodeType::try_from(raw).unwrap()
    }

    fn direct_block_ptrs(&self) -> [u32; 12] {
        [
            self.direct_block_ptr_0,
            self.direct_block_ptr_1,
            self.direct_block_ptr_2,
            self.direct_block_ptr_3,
            self.direct_block_ptr_4,
            self.direct_block_ptr_5,
            self.direct_block_ptr_6,
            self.direct_block_ptr_7,
            self.direct_block_ptr_8,
            self.direct_block_ptr_9,
            self.direct_block_ptr_10,
            self.direct_block_ptr_11,
        ]
    }
}

// See also DirEntryType below.
#[derive(Clone, Copy, Debug)]
#[repr(u16)]
enum InodeType {
    Fifo = 0x1000 >> 12,
    CharDevice = 0x2000 >> 12,
    Dir = 0x4000 >> 12,
    BlockDevice = 0x6000 >> 12,
    RegularFile = 0x8000 >> 12,
    SymbolicLink = 0xA000 >> 12,
    UnixSocket = 0xC000 >> 12,
}

impl TryFrom<u16> for InodeType {
    type Error = ();
    fn try_from(raw: u16) -> Result<Self, ()> {
        match raw {
            x if x == InodeType::Fifo as u16
                || x == InodeType::CharDevice as u16
                || x == InodeType::Dir as u16
                || x == InodeType::BlockDevice as u16
                || x == InodeType::RegularFile as u16
                || x == InodeType::SymbolicLink as u16
                || x == InodeType::UnixSocket as u16 =>
            {
                let ptr = &raw as *const _ as *const InodeType;
                unsafe {
                    // SAFETY: `raw` is valid for reads and is properly
                    // initialized.
                    Ok(ptr.read_unaligned().clone())
                }
            }
            _ => Err(()),
        }
    }
}

// const INODE_PERMIT_OTHER_EXEC: u16 = 0x001;
// const INODE_PERMIT_OTHER_WRITE: u16 = 0x002;
// const INODE_PERMIT_OTHER_READ: u16 = 0x004;
// const INODE_PERMIT_GROUP_EXEC: u16 = 0x008;
// const INODE_PERMIT_GROUP_WRITE: u16 = 0x010;
// const INODE_PERMIT_GROUP_READ: u16 = 0x020;
// const INODE_PERMIT_USER_EXEC: u16 = 0x040;
// const INODE_PERMIT_USER_WRITE: u16 = 0x080;
// const INODE_PERMIT_USER_READ: u16 = 0x100;
// const INODE_PERMIT_STICKY_BIT: u16 = 0x200;
// const INODE_PERMIT_SET_GROUP_ID: u16 = 0x400;
// const INODE_PERMIT_SET_USER_ID: u16 = 0x800;

// const INODE_FLAG_SYNC_UPDATES: u32 = 1 << 3;
// const INODE_FLAG_IMMUT_FILE: u32 = 1 << 4;
// const INODE_FLAG_APPEND_ONLY: u32 = 1 << 5;
// const INODE_FLAG_NOT_IN_DUNP: u32 = 1 << 6;
// const INODE_FLAG_KEEP_LAST_ACCESS_TIME: u32 = 1 << 7;
// const INODE_FLAG_HASH_INDEXED_DIR: u32 = 1 << 16;
// const INODE_FLAG_AFS_DIR: u32 = 1 << 17;
// const INODE_FLAG_JOURNAL_FILE_DATA: u32 = 1 << 18;

#[allow(dead_code)]
#[repr(C, packed(4))]
struct DirEntry {
    inode: u32,
    total_size: u16, // including the subfields
    name_len_0_7: u8,
    type_or_name_len_8_16: u8, // type if RequiredFeature::DirsWithType
    name: [u8; 0],
}

// See also InodeType above.
#[derive(Clone, Copy, Debug)]
#[repr(u8)]
enum DirEntryType {
    Unknown = 0,
    RegularFile = 1,
    Dir = 2,
    CharDevice = 3,
    BlockDevice = 4,
    Fifo = 5,
    Socket = 6,
    SymbolicLink = 7,
}

impl TryFrom<u8> for DirEntryType {
    type Error = ();
    fn try_from(raw: u8) -> Result<Self, ()> {
        match raw {
            x if x == DirEntryType::Unknown as u8
                || x == DirEntryType::RegularFile as u8
                || x == DirEntryType::Dir as u8
                || x == DirEntryType::CharDevice as u8
                || x == DirEntryType::BlockDevice as u8
                || x == DirEntryType::Fifo as u8
                || x == DirEntryType::Socket as u8
                || x == DirEntryType::SymbolicLink as u8 =>
            {
                let ptr = &raw as *const _ as *const DirEntryType;
                unsafe {
                    // SAFETY: `raw` is valid for reads and is properly
                    // initialized.
                    Ok(ptr.read_unaligned().clone())
                }
            }
            _ => Err(()),
        }
    }
}

#[allow(dead_code)]
pub struct Ext2 {
    rw_interface: Weak<Box<dyn disk::ReadWriteInterface>>,

    version: (u32, u16), // major, minor
    optional_features: BitFlags<u32, OptionalFeature>,
    required_features: BitFlags<u32, RequiredFeature>,
    read_only_features: BitFlags<u32, ReadOnlyFeature>,

    total_num_blocks: u32,
    block_size: usize,
    inode_size: u16,
    block_group_num_blocks: u32,
    block_group_num_inodes: u32,
    bgd_table: Vec<BlockGroupDescriptor>,

    read_only: bool,
}

#[inline(always)]
fn f64_ceil(num: f64) -> usize {
    let int_part = num as usize;
    assert_ne!(int_part, usize::MAX, "too big f64");
    int_part + 1
}

impl Ext2 {
    pub unsafe fn from_raw(
        raw_superblock: &[u8],
        raw_block_group_descriptor: &[u8],
        rw_interface: Weak<Box<dyn disk::ReadWriteInterface>>,
    ) -> Result<Self, FromRawErr> {
        // SAFETY: argument alignment is not checked.
        // FIXME: add else.

        assert_eq!(raw_superblock.len(), 1024, "invalid raw superblock size");
        assert!(
            raw_block_group_descriptor.len() != 0,
            "invalid raw block group descriptor table size",
        );

        let superblock = &*(raw_superblock.as_ptr() as *const Superblock);
        assert_eq!(
            superblock.ext2_signature, EXT2_SIGNATURE,
            "not ext2: invalid signature",
        );

        let extended_superblock = {
            if superblock.version_major >= 1 {
                let mut ptr = raw_superblock.as_ptr() as usize;
                ptr += size_of::<Superblock>();
                Some(&*(ptr as *const ExtendedSuperblock))
            } else {
                None
            }
        };
        let raw_bgd_tbl =
            raw_block_group_descriptor.as_ptr() as *const BlockGroupDescriptor;
        let mut read_only = false;

        Ok(Ext2 {
            rw_interface,

            version: (superblock.version_major, superblock.version_minor),
            optional_features: {
                if superblock.version_major >= 1 {
                    let of = BitFlags::new(
                        extended_superblock.unwrap().optional_features,
                    );
                    let absent = of;

                    let mut names = Vec::new();
                    if absent.has_set(OptionalFeature::PreallocForDir) {
                        names.push("PreallocForDir");
                    }
                    if absent.has_set(OptionalFeature::AfsServerInodesExist) {
                        names.push("AfsServerInodesExist");
                    }
                    if absent.has_set(OptionalFeature::FsHasJournal) {
                        names.push("FsHasJournal");
                    }
                    if absent.has_set(OptionalFeature::InodesWithExtAttr) {
                        names.push("InodesWithExtAttr");
                    }
                    if absent.has_set(OptionalFeature::FsCanResize) {
                        names.push("FsCanResize");
                    }
                    if absent.has_set(OptionalFeature::DirsUseHashIdx) {
                        names.push("DirsUseHashIdx");
                    }
                    println!(
                        "[EXT2] Unsupported optional features: {}.",
                        names.join(", "),
                    );

                    of
                } else {
                    BitFlags::new(0)
                }
            },
            required_features: {
                if superblock.version_major >= 1 {
                    let rf = BitFlags::new(
                        extended_superblock.unwrap().required_features,
                    );
                    let mut absent = rf;

                    // Supported features.
                    if absent.has_set(RequiredFeature::DirsWithType) {
                        absent.unset_flag(RequiredFeature::DirsWithType);
                    }

                    // Any unsupported features?
                    if absent.value != 0 {
                        return Err(FromRawErr::NoRequiredFeatures(absent));
                    }

                    rf
                } else {
                    BitFlags::new(0)
                }
            },
            read_only_features: {
                if superblock.version_major >= 1 {
                    let rof = BitFlags::new(
                        extended_superblock.unwrap().read_only_features,
                    );
                    let mut absent = rof;

                    // Supported features.
                    if absent.has_set(ReadOnlyFeature::FileSize64Bit) {
                        absent.unset_flag(ReadOnlyFeature::FileSize64Bit);
                    }

                    // Any unsupported features?
                    if absent.value != 0 {
                        println!(
                            "[EXT2] Unsupported read-only features 0x{:02X}. \
                             File system is read-only.",
                            absent.value,
                        );
                        read_only = true;
                    }

                    rof
                } else {
                    BitFlags::new(0)
                }
            },

            total_num_blocks: superblock.total_num_blocks,
            block_size: {
                let bs = 1024 * 2usize.pow(superblock.log_block_size_minus_10);
                println!("[EXT2] Block size: {} bytes.", bs);
                bs
            },
            inode_size: {
                if superblock.version_major >= 1 {
                    let extended = &*((superblock as *const Superblock).add(1)
                        as *const ExtendedSuperblock);
                    extended.inode_size
                } else {
                    128
                }
            },
            block_group_num_blocks: superblock.block_group_num_blocks,
            block_group_num_inodes: superblock.block_group_num_inodes,
            bgd_table: {
                let mut bgd_table = Vec::new();
                let num_block_groups = f64_ceil(
                    superblock.total_num_blocks as f64
                        / superblock.block_group_num_blocks as f64,
                );
                for i in 0..num_block_groups {
                    let raw_bgd = (raw_bgd_tbl as usize + i * 32)
                        as *const BlockGroupDescriptor;
                    bgd_table.push((*raw_bgd).clone());
                }
                bgd_table
            },

            read_only,
        })
    }

    fn inode_addr(&self, inode_idx: u32) -> usize {
        assert!(inode_idx > 0, "invalid inode index");
        if self.block_size as u32 == 0 {
            unimplemented!("too big block size");
        }
        let block_size = self.block_size as u32;
        let inode_size = self.inode_size as u32;

        let block_group = (inode_idx - 1) / self.block_group_num_inodes;
        let idx_in_group = (inode_idx - 1) % self.block_group_num_inodes;
        let rel_block_with_inode =
            (idx_in_group * inode_size) / self.block_size as u32;
        let abs_block_with_inode = self.bgd_table[block_group as usize]
            .inode_table_start_block_addr
            + rel_block_with_inode;

        let inode_addr = abs_block_with_inode * block_size
            + (idx_in_group * inode_size) % block_size;
        // FIXME: inode_addr should be u64?
        inode_addr as usize
    }

    fn read_inode(&self, inode_idx: u32) -> Result<Box<Inode>, ReadInodeErr> {
        let rw_interface = self
            .rw_interface
            .upgrade()
            .ok_or(ReadInodeErr::NoRwInterface)
            .unwrap();
        let inode_addr = self.inode_addr(inode_idx);
        let first_block_idx = inode_addr / rw_interface.block_size();
        let num_blocks = size_of::<Inode>() / rw_interface.block_size() + 1;
        let offset_in_blocks = inode_addr % rw_interface.block_size();
        match rw_interface.read_blocks(first_block_idx, num_blocks) {
            Ok(blocks) => {
                let base = blocks.as_ptr();
                unsafe {
                    let raw = base.add(offset_in_blocks) as *const Inode;
                    Ok(Box::new((*raw).clone()))
                }
            }
            Err(err) => Err(ReadInodeErr::DiskErr(err)),
        }
    }

    fn inode_size(&self, inode: &Inode) -> usize {
        // TODO: read-only feature 64-bit file size
        inode.size as usize
    }

    fn read_inode_block(
        &self,
        inode: &Inode,
        index: usize,
    ) -> Result<Box<[u8]>, ReadInodeBlockErr> {
        // Divide all the possible blocks into SIBs, DIBs and TIBs.  The SIBs
        // are those blocks which are accessed using the singly indirect block
        // pointer, the DIBs are accessed using the doubly indirect block
        // pointer, etc.
        let sibs_range = Range {
            start: 12,
            end: 12 + self.block_size / 4,
        };
        let dibs_range = Range {
            start: sibs_range.end,
            end: sibs_range.end + sibs_range.len() * self.block_size / 4,
        };
        let tibs_range = Range {
            start: dibs_range.end,
            end: dibs_range.end + dibs_range.len() * self.block_size / 4,
        };

        let block_num = if index < 12 {
            inode.direct_block_ptrs()[index] as usize
        } else if sibs_range.contains(&index) {
            // FIXME: block numbers are always 32-bit.
            if { inode.singly_indirect_block_ptr } == 0 {
                return Err(ReadInodeBlockErr::BlockNotFound);
            }
            let sib_ptr_idx = index - sibs_range.start;
            self.read_block_entry(
                inode.singly_indirect_block_ptr as usize,
                sib_ptr_idx,
            )?
        } else if dibs_range.contains(&index) {
            if { inode.doubly_indirect_block_ptr } == 0 {
                return Err(ReadInodeBlockErr::BlockNotFound);
            }
            let dib_ptr_idx = (index - dibs_range.start) / sibs_range.len();
            let sib_ptr_idx = (index - dibs_range.start) % sibs_range.len();
            println!(
                "  DIB ptr idx {} SIB ptr idx {}",
                dib_ptr_idx, sib_ptr_idx,
            );
            let sib_ptr = self.read_block_entry(
                inode.doubly_indirect_block_ptr as usize,
                dib_ptr_idx,
            )?;
            if sib_ptr == 0 {
                return Err(ReadInodeBlockErr::BlockNotFound);
            }
            self.read_block_entry(sib_ptr, sib_ptr_idx)?
        } else if tibs_range.contains(&index) {
            if { inode.triply_indirect_block_ptr } == 0 {
                return Err(ReadInodeBlockErr::BlockNotFound);
            }
            let tib_ptr_idx = (index - tibs_range.start) / dibs_range.len();
            let dib_ptr_idx = ((index - tibs_range.start) % dibs_range.len())
                / sibs_range.len();
            let sib_ptr_idx = ((index - tibs_range.start) % dibs_range.len())
                % sibs_range.len();
            println!(
                "  TIB ptr idx {} DIB ptr idx {} SIB ptr idx {}",
                tib_ptr_idx, dib_ptr_idx, sib_ptr_idx,
            );
            let dib_ptr = self.read_block_entry(
                inode.triply_indirect_block_ptr as usize,
                tib_ptr_idx,
            )?;
            if dib_ptr == 0 {
                return Err(ReadInodeBlockErr::BlockNotFound);
            }
            let sib_ptr = self.read_block_entry(dib_ptr, dib_ptr_idx)?;
            if sib_ptr == 0 {
                return Err(ReadInodeBlockErr::BlockNotFound);
            }
            self.read_block_entry(sib_ptr, sib_ptr_idx)?
        } else {
            return Err(ReadInodeBlockErr::TooBigBlockIndex);
        };
        if block_num != 0 {
            Ok(self.read_block(block_num)?)
        } else {
            Err(ReadInodeBlockErr::BlockNotFound)
        }
    }

    fn num_block_entries(
        &self,
        block_num: usize,
    ) -> Result<usize, ReadBlockErr> {
        let block = self.read_block(block_num)?;
        let mut i = 0;
        while i < self.block_size / 4 {
            let first = i * 4;
            let entry = block[first] as usize
                | ((block[first + 1] as usize) << 8)
                | ((block[first + 2] as usize) << 16)
                | ((block[first + 3] as usize) << 24);
            if entry == 0 {
                break;
            }
            i += 1;
        }
        Ok(i)
    }

    fn read_block_entry(
        &self,
        block_num: usize,
        entry_idx: usize,
    ) -> Result<usize, ReadBlockErr> {
        let block = self.read_block(block_num)?;
        assert!(entry_idx * 4 <= block.len() - 4);
        let first = entry_idx * 4;
        Ok(block[first] as usize
            | ((block[first + 1] as usize) << 8)
            | ((block[first + 2] as usize) << 16)
            | ((block[first + 3] as usize) << 24))
    }

    fn read_block(&self, block_num: usize) -> Result<Box<[u8]>, ReadBlockErr> {
        let rw_interface = self
            .rw_interface
            .upgrade()
            .ok_or(ReadBlockErr::NoRwInterface)
            .unwrap();
        if block_num >= self.total_num_blocks as usize {
            return Err(ReadBlockErr::InvalidBlockNum);
        }
        let addr = block_num * self.block_size;
        assert_eq!(
            addr % rw_interface.block_size(),
            0,
            "cannot convert the block address to a block index",
        );
        let block_idx = addr / rw_interface.block_size();
        assert_eq!(
            self.block_size % rw_interface.block_size(),
            0,
            "cannot convert the ext2 block size to a number of system blocks",
        );
        let num_blocks = self.block_size / rw_interface.block_size();
        Ok(rw_interface.read_blocks(block_idx, num_blocks)?)
    }

    fn iter_dir(
        &self,
        first_entry: *const DirEntry,
        total_size: usize,
    ) -> DirEntryIter {
        DirEntryIter {
            current: first_entry,
            start: first_entry,
            total_size,
        }
    }
}

pub enum FromRawErr {
    NoRequiredFeatures(BitFlags<u32, RequiredFeature>),
}

impl fmt::Debug for FromRawErr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FromRawErr::NoRequiredFeatures(rf) => f.write_fmt(format_args!(
                "NoRequiredFeatures(0x{:0X})",
                rf.value,
            )),
        }
    }
}

#[derive(Debug)]
enum ReadInodeErr {
    NoRwInterface,
    DiskErr(disk::ReadErr),
}

impl From<disk::ReadErr> for ReadInodeErr {
    fn from(err: disk::ReadErr) -> Self {
        ReadInodeErr::DiskErr(err)
    }
}

impl From<ReadInodeErr> for super::ReadDirErr {
    fn from(err: ReadInodeErr) -> Self {
        match err {
            ReadInodeErr::NoRwInterface => Self::NoRwInterface,
            ReadInodeErr::DiskErr(e) => Self::DiskErr(e),
        }
    }
}

impl From<ReadInodeErr> for super::ReadFileErr {
    fn from(err: ReadInodeErr) -> Self {
        match err {
            ReadInodeErr::NoRwInterface => Self::NoRwInterface,
            ReadInodeErr::DiskErr(e) => Self::DiskErr(e),
        }
    }
}

#[derive(Debug)]
enum ReadInodeBlockErr {
    BlockNotFound,
    TooBigBlockIndex,
    ReadBlockErr(ReadBlockErr),
}

impl From<ReadBlockErr> for ReadInodeBlockErr {
    fn from(err: ReadBlockErr) -> Self {
        ReadInodeBlockErr::ReadBlockErr(err)
    }
}

impl From<ReadInodeBlockErr> for ReadDirErr {
    fn from(err: ReadInodeBlockErr) -> Self {
        match err {
            ReadInodeBlockErr::BlockNotFound
            | ReadInodeBlockErr::TooBigBlockIndex => {
                ReadDirErr::InvalidDescriptor
            }
            ReadInodeBlockErr::ReadBlockErr(e) => match e {
                ReadBlockErr::NoRwInterface => ReadDirErr::NoRwInterface,
                ReadBlockErr::DiskErr(e) => ReadDirErr::DiskErr(e),
                ReadBlockErr::InvalidBlockNum => ReadDirErr::InvalidDescriptor,
            },
        }
    }
}

#[derive(Debug)]
enum ReadBlockErr {
    NoRwInterface,
    DiskErr(disk::ReadErr),
    InvalidBlockNum,
}

impl From<disk::ReadErr> for ReadBlockErr {
    fn from(err: disk::ReadErr) -> Self {
        ReadBlockErr::DiskErr(err)
    }
}

impl From<ReadBlockErr> for super::ReadFileErr {
    fn from(err: ReadBlockErr) -> Self {
        match err {
            ReadBlockErr::NoRwInterface => Self::NoRwInterface,
            ReadBlockErr::DiskErr(e) => Self::DiskErr(e),
            ReadBlockErr::InvalidBlockNum => Self::InvalidBlockNum,
        }
    }
}

impl FileSystem for Ext2 {
    fn root_dir(&self) -> Result<Directory, ReadDirErr> {
        self.read_dir(2)
    }

    fn read_dir(&self, id: usize) -> Result<Directory, ReadDirErr> {
        assert_ne!(id as u32, 0, "invalid id");
        let dir_inode = self.read_inode(id as u32)?;
        let mut dir = Directory {
            id,
            name: String::new(),
            entries: Vec::new(),
        };

        // Traverse the directory.
        let total_size = self.inode_size(&dir_inode);
        let num_blocks = (total_size + self.block_size - 1) / self.block_size;
        let blocks = unsafe {
            let mut res = Vec::new();
            for i in 0..num_blocks {
                let block = self.read_inode_block(&dir_inode, i)?;
                res.push(block.to_vec());
            }
            res.concat()
        };
        let first_entry = blocks.as_ptr() as *const DirEntry;

        for raw_entry in self.iter_dir(first_entry, total_size) {
            // TODO: read all inodes together in a hope that they are
            // stored close to each other?
            let entry = unsafe { &*raw_entry };
            let inode_idx = entry.inode;
            let mut name_len = entry.name_len_0_7 as usize;

            let _type = {
                if self
                    .required_features
                    .has_set(RequiredFeature::DirsWithType)
                {
                    DirEntryContent::from(
                        DirEntryType::try_from(entry.type_or_name_len_8_16)
                            .unwrap(),
                    )
                } else {
                    name_len |= (entry.type_or_name_len_8_16 as usize) << 8;
                    let inode = self.read_inode(inode_idx)?;
                    DirEntryContent::from(inode._type())
                }
            };

            dir.entries.push(super::DirEntry {
                id: inode_idx as usize,
                name: {
                    let s = unsafe {
                        slice::from_raw_parts(
                            &entry.name as *const u8,
                            name_len,
                        )
                    };
                    String::from_utf8(s.to_vec())?
                },
                content: _type,
            });
        }

        // Obtain the directory name.
        // FIXME: is ".." always the first dir entry?
        if dir.entries[0].name != ".." {
            unimplemented!();
        } else if id == 2 {
            dir.name = String::from("/");
        } else {
            let parent_dir_id = dir.entries[0].id;
            let parent_dir = self.read_dir(parent_dir_id)?;
            match parent_dir.entries.iter().find(|&e| e.id == id) {
                Some(itself) => dir.name = itself.name.clone(),
                None => {
                    // unreachable? see fixme above
                    unimplemented!();
                }
            }
        }

        Ok(dir)
    }

    fn read_file(&self, id: usize) -> Result<Vec<Box<[u8]>>, ReadFileErr> {
        assert_ne!(id as u32, 0, "invalid id");
        let inode = self.read_inode(id as u32)?;
        let mut i: usize = 0;
        let mut all_bufs = Vec::new();
        loop {
            match self.read_inode_block(&inode, i) {
                Ok(buf) => {
                    all_bufs.push(buf);
                    i += 1;
                }
                Err(err) => match err {
                    ReadInodeBlockErr::BlockNotFound
                    | ReadInodeBlockErr::TooBigBlockIndex => break,
                    ReadInodeBlockErr::ReadBlockErr(e) => {
                        return Err(From::from(e))
                    }
                },
            }
        }
        Ok(all_bufs)
    }

    fn file_size_bytes(&self, id: usize) -> Result<u64, ReadFileErr> {
        assert_ne!(id as u32, 0, "invalid id");
        let inode = self.read_inode(id as u32)?;
        let mut size = inode.size as u64;
        if self
            .read_only_features
            .has_set(ReadOnlyFeature::FileSize64Bit)
        {
            size |= (inode.file_size_bits_32_63 as u64) << 32;
        }
        Ok(size)
    }

    fn file_size_blocks(&self, id: usize) -> Result<usize, ReadFileErr> {
        // FIXME: compare with inode.count_disk_sectors
        assert_ne!(id as u32, 0, "invalid id");
        let rw_interface = self
            .rw_interface
            .upgrade()
            .ok_or(ReadFileErr::NoRwInterface)
            .unwrap();
        let inode = self.read_inode(id as u32)?;
        let mut size = 0;
        size += inode.direct_block_ptrs().iter().fold(0, |acc, x| match x {
            0 => acc,
            _ => acc + 1,
        });
        let mut fs_blocks = 0; // blocks used by the file system for the inode
        if inode.singly_indirect_block_ptr != 0 {
            //let was = size;
            size += self
                .num_block_entries(inode.singly_indirect_block_ptr as usize)?;
            fs_blocks += 1;
            //let is = size;
            //println!("sibs: {}", is - was);
        }
        if inode.doubly_indirect_block_ptr != 0 {
            let num_dibs = self
                .num_block_entries(inode.doubly_indirect_block_ptr as usize)?;
            //println!("num_dibs = {}", num_dibs);
            let last_dib = self.read_block_entry(
                inode.doubly_indirect_block_ptr as usize,
                num_dibs - 1,
            )?;
            let num_sibs_in_last_dib = self.num_block_entries(last_dib)?;
            //println!("num_sibs_in_last_dib = {}", num_sibs_in_last_dib);
            size += (num_dibs - 1) * self.block_size / 4 + num_sibs_in_last_dib;
            fs_blocks += 1 + num_dibs;
        }
        if inode.triply_indirect_block_ptr != 0 {
            let num_tibs = self
                .num_block_entries(inode.triply_indirect_block_ptr as usize)?;
            let last_tib = self.read_block_entry(
                inode.triply_indirect_block_ptr as usize,
                num_tibs - 1,
            )?;
            let num_dibs_in_last_tib = self.num_block_entries(last_tib)?;
            let last_dib =
                self.read_block_entry(last_tib, num_dibs_in_last_tib - 1)?;
            let num_sibs_in_last_dib = self.num_block_entries(last_dib)?;
            size += (num_tibs - 1) * (self.block_size / 4).pow(2)
                + num_dibs_in_last_tib * self.block_size / 4
                + num_sibs_in_last_dib;
            fs_blocks +=
                1 + (num_tibs - 1) * self.block_size / 4 + num_dibs_in_last_tib;
        }
        assert!(self.block_size >= rw_interface.block_size());
        assert_eq!(
            inode.count_disk_sectors as usize,
            (size + fs_blocks) * (self.block_size / rw_interface.block_size()),
        );
        Ok(size)
    }
}

impl From<InodeType> for DirEntryContent {
    fn from(inode_type: InodeType) -> Self {
        match inode_type {
            InodeType::RegularFile => DirEntryContent::RegularFile,
            InodeType::Dir => DirEntryContent::Directory,
            _ => DirEntryContent::Unknown,
        }
    }
}

impl From<DirEntryType> for DirEntryContent {
    fn from(entry_type: DirEntryType) -> Self {
        match entry_type {
            DirEntryType::RegularFile => DirEntryContent::RegularFile,
            DirEntryType::Dir => DirEntryContent::Directory,
            _ => DirEntryContent::Unknown,
        }
    }
}

struct DirEntryIter {
    current: *const DirEntry,
    start: *const DirEntry,
    total_size: usize,
}

impl Iterator for DirEntryIter {
    type Item = *const DirEntry;

    fn next(&mut self) -> Option<Self::Item> {
        if self.current.is_null() || self.start.is_null() {
            unreachable!();
        }
        unsafe {
            let entry_size = (&*self.current).total_size as usize;
            let align = align_of::<DirEntry>() - 1;
            self.current = ((self.current as usize + entry_size + align)
                & !align) as *const DirEntry;
            if (self.current as usize) < self.start as usize + self.total_size {
                Some(self.current)
            } else {
                None
            }
        }
    }
}
