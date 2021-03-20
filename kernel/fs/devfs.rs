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
use alloc::format;
use alloc::rc::{Rc, Weak};
use alloc::string::String;
use alloc::vec::Vec;
use core::cell::RefCell;

use crate::block_device;

use super::{
    FileSystem, Node, NodeInternals, NodeType, ReadDirErr, ReadFileErr,
};

const ROOT_ID: usize = 200;
const MAX_BLOCK_DEVICES: usize = 100; // block device IDs: 0..100

pub struct DevFs {
    block_devices: Vec<Rc<RefCell<dyn block_device::BlockDevice>>>,
}

impl DevFs {
    pub fn init() -> Self {
        let mut res = DevFs {
            block_devices: Vec::new(),
        };

        // Register all block devices.
        for blkdev in block_device::BLOCK_DEVICES.lock().iter() {
            res.register_block_device(blkdev);
        }

        // Register char devices.
        // res.register_char_device();

        res
    }

    /// Allocates an inode ID.
    ///
    /// # Panics
    /// This method panics if there are [`MAX_BLOCK_DEVICES`] or more registered
    /// block devices.
    fn allocate_id(&self, is_block_device: bool) -> usize {
        if is_block_device {
            assert!(self.block_devices.len() < MAX_BLOCK_DEVICES);
            self.block_devices.len()
        } else {
            unimplemented!();
        }
    }

    fn resolve_id(&self, id_in_fs: usize) -> ResolveId {
        if id_in_fs < MAX_BLOCK_DEVICES {
            let blkdev_id = id_in_fs;
            let rc_blkdev = unsafe {
                Rc::clone(&block_device::BLOCK_DEVICES.lock()[blkdev_id])
            };
            ResolveId::BlockDevice(rc_blkdev)
        } else {
            unimplemented!();
        }
    }

    fn register_block_device(
        &mut self,
        blkdev: &Rc<RefCell<dyn block_device::BlockDevice>>,
    ) -> usize {
        let id_in_fs = self.allocate_id(true);
        println!("[DEVFS] Registering a block device blk{}.", id_in_fs);
        self.block_devices.push(Rc::clone(blkdev));
        id_in_fs
    }
}

impl FileSystem for DevFs {
    fn root_dir(&self) -> Result<Node, ReadDirErr> {
        self.read_dir(ROOT_ID)
    }

    fn read_dir(&self, id: usize) -> Result<Node, ReadDirErr> {
        // There is only one directory currently.
        assert_eq!(id, ROOT_ID, "invalid id");

        let node = Node(Rc::new(RefCell::new(NodeInternals {
            _type: NodeType::Dir,
            name: String::from("/"),
            id_in_fs: Some(ROOT_ID),

            parent: None,
            maybe_children: Some(Vec::new()),
        })));
        let node_weak = Rc::downgrade(&node.0);
        let mut node_mut = node.0.borrow_mut();

        for (i, disk) in self.block_devices.iter().enumerate() {
            node_mut.maybe_children.as_mut().unwrap().push(Node(Rc::new(
                RefCell::new(NodeInternals {
                    _type: NodeType::BlockDevice,
                    name: format!("blk{}", i),
                    id_in_fs: Some(i),

                    parent: Some(Weak::clone(&node_weak)),
                    maybe_children: None,
                }),
            )));
        }

        drop(node_mut);
        Ok(node)
    }

    fn read_file(&self, id: usize) -> Result<Vec<u8>, ReadFileErr> {
        unimplemented!();
    }

    /// Reads `len` bytes from the specified block device starting at byte
    /// `offset`.
    ///
    /// # Panics
    /// This method panics if:
    /// * there is no such device,
    /// * one or more bytes from the range `offset..offset+len` lie outside the
    ///   block device,
    /// * [`block_device::BlockDevice::read_blocks()`] returns an error.
    fn read_file_offset_len(
        &self,
        id: usize,
        offset: usize,
        len: usize,
    ) -> Result<Vec<u8>, ReadFileErr> {
        let refcell_blkdev = match self.resolve_id(id) {
            ResolveId::BlockDevice(blkdev) => blkdev,
        };
        let blkdev = refcell_blkdev.borrow();

        let mut res_buf = Vec::with_capacity(len);
        let start_block = offset / blkdev.block_size();
        let end_block = (offset + len - 1) / blkdev.block_size() + 1;
        let num_blocks = end_block - start_block;

        for block in blkdev.read_blocks(start_block, num_blocks) {
            res_buf.extend_from_slice(&block);
        }

        res_buf.drain(0..offset % blkdev.block_size());
        res_buf.truncate(len);

        Ok(res_buf)
    }

    fn file_size_bytes(&self, id: usize) -> Result<usize, ReadFileErr> {
        unimplemented!();
    }

    fn file_size_blocks(&self, id: usize) -> Result<usize, ReadFileErr> {
        unimplemented!();
    }
}

enum ResolveId {
    BlockDevice(Rc<RefCell<dyn block_device::BlockDevice>>),
}
