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
use alloc::string::String;
use alloc::vec::Vec;
use core::mem::{align_of, size_of};
use core::ops::Range;
use core::slice;

use super::{
    DirEntryContent, Directory, FileSizeErr, FileSystem, ReadDirErr,
    ReadFileErr,
};
use crate::bitflags::BitFlags;
use crate::disk::{ReadErr, ReadWriteInterface};

#[allow(dead_code)]
#[repr(C, packed)]
struct Superblock {
    total_num_inodes: u32,
    total_num_blocks: u32,
    num_reserved_blocks: u32,
    total_num_unallocated_blocks: u32,
    total_num_unallocated_inodes: u32,
    block_num_of_superblock: u32,
    log_block_size_minus_10: u32,
    log_fragment_size_minus_10: u32,
    block_group_num_blocks: u32,
    block_group_num_fragments: u32,
    block_group_num_inodes: u32,
    last_mount_time: u32,
    last_written_time: u32,
    num_mounts_since_consistency_check: u16,
    allowed_num_mounts_since_consistency_check: u16,
    ext2_signature: u16,
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

const EXT2_SIGNATURE: u16 = 0xEF53;

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
    unused: u16,
    journal_id: [u8; 16], // C-style string
    journal_inode: u32,
    journal_device: u32,
    orphan_inode_list_head: u32,
}

// const OPTIONAL_FEATURE_PREALLOC_FOR_DIR: u32 = 0x01;
// const OPTIONAL_FEATURE_AFS_SERVER_INODES_EXIST: u32 = 0x02;
// const OPTIONAL_FEATURE_FS_HAS_JOURNAL: u32 = 0x04;
// const OPTIONAL_FEATURE_INODES_WITH_EXTENDED_ATTR: u32 = 0x08;
// const OPTIONAL_FEATURE_FS_CAN_RESIZE: u32 = 0x10;
// const OPTIONAL_FEATURE_DIRS_USE_HASH_INDEX: u32 = 0x20;

// const REQUIRED_FEATURE_COMPRESSION: u32 = 0x01;
// const REQUIRED_FEATURE_DIRS_WITH_TYPE: u32 = 0x02;
// const REQUIRED_FEATURE_FS_NEEDS_TO_REPLAY_JOURNAL: u32 = 0x04;
// const REQUIRED_FEATURE_FS_USES_JOURNAL_DEVICE: u32 = 0x08;

bitflags! {
    #[repr(u32)]
    enum RequiredFeature {
        Compression = 0x01,
        DirsWithType = 0x02,
        FsNeedsToReplayJournal = 0x04,
        FsUsesJournalDevice = 0x08,
    }
}

// const FEATURE_OR_READ_ONLY_SPARSE: u32 = 0x01;
// const FEATURE_OR_READ_ONLY_64BIT_FILE_SIZE: u32 = 0x02;
// const FEATURE_OR_READ_ONLY_DIR_CONTENTS_BIN_TREE: u32 = 0x04;

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
struct BlockGroupDescriptor {
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
    size: u32, // if FEATURE_OR_READ_ONLY_64BIT_FILE_SIZE, these are bits 0..31
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
    file_size_bits_32_63: u32, // if FEATURE_OR_READ_ONLY_64BIT_FILE_SIZE
    fragment_block_addr: u32,
    os_specific_2: [u8; 12],
}

impl Inode {
    fn _type(&self) -> InodeType {
        let _type = (self.type_and_permissions >> 12) & 0b1111;
        let raw_enum = &_type as *const _ as *const InodeType;
        // FIXME: make sure that there is a right value written?
        assert_eq!(
            raw_enum as usize % align_of::<InodeType>(),
            0,
            "invalid align",
        );
        unsafe { *raw_enum }
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

// const INODE_TYPE_FIFO: u16 = 0x1000;
// const INODE_TYPE_CHAR_DEVICE: u16 = 0x2000;
// const INODE_TYPE_DIR: u16 = 0x4000;
// const INODE_TYPE_BLOCK_DEVICE: u16 = 0x6000;
// const INODE_TYPE_REGULAR_FILE: u16 = 0x8000;
// const INODE_TYPE_SYMBOLIC_LINK: u16 = 0xA000;
// const INODE_TYPE_UNIX_SOCKET: u16 = 0xC000;

#[allow(dead_code)]
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
    total_size: u16, // including subfields
    name_len_0_7: u8,
    type_or_name_len_8_16: u8, // type if REQUIRED_FEATURE_DIRS_WITH_TYPE
    name: [u8; 0],
}

// const DIR_ENTRY_TYPE_UNKNOWN: u8 = 0;
const DIR_ENTRY_TYPE_REGULAR_FILE: u8 = 1;
const DIR_ENTRY_TYPE_DIR: u8 = 2;
// const DIR_ENTRY_TYPE_CHAR_DEVICE: u8 = 3;
// const DIR_ENTRY_TYPE_BLOCK_DEVICE: u8 = 4;
// const DIR_ENTRY_TYPE_FIFO: u8 = 5;
// const DIR_ENTRY_TYPE_SOCKET: u8 = 6;
// const DIR_ENTRY_TYPE_SYMBOLIC_LINK: u8 = 7;

pub struct Ext2 {
    version: (u32, u16), // major, minor
    //optional_features: BitFlags<u32, OptionalFeature>,
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
    ) -> Self {
        assert_eq!(raw_superblock.len(), 1024, "invalid raw superblock size");
        assert!(
            (raw_block_group_descriptor.len()
                % align_of::<BlockGroupDescriptor>()
                == 0)
                && raw_block_group_descriptor.len() != 0,
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
        Ext2 {
            version: (superblock.version_major, superblock.version_minor),
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

                    // Unsupported features.
                    // FIXME: return error instead of panic
                    if absent.has_set(RequiredFeature::Compression) {
                        panic!(
                            "Required feature Compression is not \
                             supported.",
                        );
                    }
                    if absent.has_set(RequiredFeature::FsNeedsToReplayJournal) {
                        panic!(
                            "Required feature FsNeedsToReplayJournal is not \
                             supported.",
                        );
                    }
                    if absent.has_set(RequiredFeature::FsUsesJournalDevice) {
                        panic!(
                            "Required feature FsUsesJournalDevice is not \
                             supported.",
                        );
                    }

                    // Any unsupported features?
                    if absent.value != 0 {
                        panic!(
                            "Required features 0x{:X} are not supported",
                            rf.value,
                        );
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
                        // FIXME: mount read only
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
            block_size: 1024 * 2usize.pow(superblock.log_block_size_minus_10),
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
        }
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

    fn read_inode(
        &self,
        inode_idx: u32,
        rw_interface: &Box<dyn ReadWriteInterface>,
    ) -> Result<Box<Inode>, ReadErr> {
        let inode_addr = self.inode_addr(inode_idx);
        let first_sector_idx = inode_addr / rw_interface.sector_size();
        let num_sectors = size_of::<Inode>() / rw_interface.sector_size() + 1;
        let offset_in_sectors = inode_addr % rw_interface.sector_size();
        match rw_interface.read_sectors(first_sector_idx, num_sectors) {
            Ok(sectors) => {
                let base = sectors.as_ptr();
                unsafe {
                    let raw = base.add(offset_in_sectors) as *const Inode;
                    Ok(Box::new((*raw).clone()))
                }
            }
            Err(err) => Err(err),
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
        rw_interface: &Box<dyn ReadWriteInterface>,
    ) -> Result<Box<[u8]>, ReadInodeBlockErr> {
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
            // FIXME block numbers are always 32-bit
            assert_ne!(inode.singly_indirect_block_ptr, 0);
            let idx_in_sibs = index - sibs_range.start;
            self.read_block_entry(
                inode.singly_indirect_block_ptr as usize,
                idx_in_sibs,
                rw_interface,
            )?
        } else if dibs_range.contains(&index) {
            unimplemented!();
        } else if tibs_range.contains(&index) {
            unimplemented!();
        } else {
            panic!("Too big index.");
        };
        if block_num != 0 {
            Ok(self.read_block(block_num, rw_interface)?)
        } else {
            Err(ReadInodeBlockErr::BlockNotFound)
        }
    }

    fn num_block_entries(
        &self,
        block_num: usize,
        rw_interface: &Box<dyn ReadWriteInterface>,
    ) -> Result<usize, ReadErr> {
        let block = self.read_block(block_num, rw_interface)?;
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
        rw_interface: &Box<dyn ReadWriteInterface>,
    ) -> Result<usize, ReadErr> {
        let block = self.read_block(block_num, rw_interface)?;
        assert!(entry_idx * 4 <= block.len() - 4);
        let first = entry_idx * 4;
        Ok(block[first] as usize
            | ((block[first + 1] as usize) << 8)
            | ((block[first + 2] as usize) << 16)
            | ((block[first + 3] as usize) << 24))
    }

    fn read_block(
        &self,
        block_num: usize,
        rw_interface: &Box<dyn ReadWriteInterface>,
    ) -> Result<Box<[u8]>, ReadErr> {
        if block_num >= self.total_num_blocks as usize {
            return Err(ReadErr::Other("invalid block num"));
        }
        let addr = block_num * self.block_size;
        assert_eq!(
            addr % rw_interface.sector_size(),
            0,
            "cannot convert block address to sector idx",
        );
        let sector_idx = addr / rw_interface.sector_size();
        assert_eq!(
            self.block_size % rw_interface.sector_size(),
            0,
            "cannot convert block size to num of sectors",
        );
        let num_sectors = self.block_size / rw_interface.sector_size();
        rw_interface.read_sectors(sector_idx, num_sectors)
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

impl FileSystem for Ext2 {
    fn root_dir(
        &self,
        rw_interface: &Box<dyn ReadWriteInterface>,
    ) -> Result<Directory, ReadDirErr> {
        self.read_dir(2, rw_interface)
    }

    fn read_dir(
        &self,
        id: usize,
        rw_interface: &Box<dyn ReadWriteInterface>,
    ) -> Result<Directory, ReadDirErr> {
        assert_ne!(id as u32, 0, "invalid id");
        let dir_inode = self.read_inode(id as u32, rw_interface)?;
        let mut dir = Directory {
            id,
            name: String::new(),
            entries: Vec::new(),
        };

        // Traverse the directory.
        let dbp0 = self.read_inode_block(&dir_inode, 0, rw_interface).unwrap();
        let first_entry = dbp0.as_ptr() as *const DirEntry;
        let total_size = self.inode_size(&dir_inode);
        if total_size > self.block_size {
            unimplemented!();
        }

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
                    match entry.type_or_name_len_8_16 {
                        DIR_ENTRY_TYPE_REGULAR_FILE => {
                            DirEntryContent::RegularFile
                        }
                        DIR_ENTRY_TYPE_DIR => DirEntryContent::Directory,
                        _ => DirEntryContent::Unknown,
                    }
                } else {
                    name_len |= (entry.type_or_name_len_8_16 as usize) << 8;
                    let inode = self.read_inode(inode_idx, rw_interface)?;
                    match inode._type() {
                        InodeType::RegularFile => DirEntryContent::RegularFile,
                        InodeType::Dir => DirEntryContent::Directory,
                        _ => DirEntryContent::Unknown,
                    }
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
            let parent_dir = self.read_dir(parent_dir_id, rw_interface)?;
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

    fn read_file(
        &self,
        id: usize,
        rw_interface: &Box<dyn ReadWriteInterface>,
    ) -> Result<Vec<Box<[u8]>>, ReadFileErr> {
        assert_ne!(id as u32, 0, "invalid id");
        let inode = self.read_inode(id as u32, rw_interface)?;
        let mut i: usize = 0;
        let mut all_bufs = Vec::new();
        loop {
            match self.read_inode_block(&inode, i, rw_interface) {
                Ok(buf) => {
                    all_bufs.push(buf);
                    i += 1;
                }
                Err(err) => match err {
                    ReadInodeBlockErr::BlockNotFound => break,
                    ReadInodeBlockErr::DiskErr(disk_err) => {
                        return Err(ReadFileErr::DiskErr(disk_err))
                    }
                },
            }
        }
        Ok(all_bufs)
    }

    fn file_size_bytes(
        &self,
        id: usize,
        rw_interface: &Box<dyn ReadWriteInterface>,
    ) -> Result<u64, FileSizeErr> {
        assert_ne!(id as u32, 0, "invalid id");
        let inode = self.read_inode(id as u32, rw_interface)?;
        let mut size = inode.size as u64;
        if self
            .read_only_features
            .has_set(ReadOnlyFeature::FileSize64Bit)
        {
            size |= (inode.file_size_bits_32_63 as u64) << 32;
        }
        Ok(size)
    }

    fn file_size_blocks(
        &self,
        id: usize,
        rw_interface: &Box<dyn ReadWriteInterface>,
    ) -> Result<usize, FileSizeErr> {
        // FIXME: compare with inode.count_disk_sectors
        assert_ne!(id as u32, 0, "invalid id");
        let inode = self.read_inode(id as u32, rw_interface)?;
        let mut size = 0;
        size += inode.direct_block_ptrs().iter().fold(0, |acc, x| match x {
            0 => acc,
            _ => acc + 1,
        });
        if inode.singly_indirect_block_ptr != 0 {
            let was = size;
            size += self.num_block_entries(
                inode.singly_indirect_block_ptr as usize,
                rw_interface,
            )?;
            let is = size;
            println!("sibs: {}", is - was);
        }
        if inode.doubly_indirect_block_ptr != 0 {
            let num_dibs = self.num_block_entries(
                inode.doubly_indirect_block_ptr as usize,
                rw_interface,
            )?;
            println!("num_dibs = {}", num_dibs);
            let last_dib = self.read_block_entry(
                inode.doubly_indirect_block_ptr as usize,
                num_dibs - 1,
                rw_interface,
            )?;
            let num_sibs_in_last_dib =
                self.num_block_entries(last_dib, rw_interface)?;
            println!("num_sibs_in_last_dib = {}", num_sibs_in_last_dib);
            size += (num_dibs - 1) * self.block_size / 4 + num_sibs_in_last_dib;
        }
        if inode.triply_indirect_block_ptr != 0 {
            let num_tibs = self.num_block_entries(
                inode.triply_indirect_block_ptr as usize,
                rw_interface,
            )?;
            let last_tib = self.read_block_entry(
                inode.triply_indirect_block_ptr as usize,
                num_tibs - 1,
                rw_interface,
            )?;
            let num_dibs_in_last_tib =
                self.num_block_entries(last_tib, rw_interface)?;
            let last_dib = self.read_block_entry(
                last_tib,
                num_dibs_in_last_tib - 1,
                rw_interface,
            )?;
            let num_sibs_in_last_dib =
                self.num_block_entries(last_dib, rw_interface)?;
            size +=
                num_tibs * (self.block_size / 4).pow(2) + num_sibs_in_last_dib;
        }
        Ok(size)
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

#[derive(Debug)]
enum ReadInodeBlockErr {
    BlockNotFound,
    DiskErr(ReadErr),
}

impl From<ReadErr> for ReadInodeBlockErr {
    fn from(err: ReadErr) -> Self {
        ReadInodeBlockErr::DiskErr(err)
    }
}
