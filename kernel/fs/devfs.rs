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

use alloc::format;
use alloc::rc::{Rc, Weak};
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;
use core::cell::RefCell;

use crate::block_device;
use crate::char_device;

use super::{
    FileSystem, Node, NodeInternals, NodeType, ReadDirErr, ReadFileErr,
    WriteFileErr,
};

const ROOT_ID: usize = 200;
const MAX_BLOCK_DEVICES: usize = 100; // block device IDs: 0..100
const MAX_CHAR_DEVICES: usize = 100; // char device IDs: 100..200

pub struct DevFs {
    block_devices: Vec<Rc<RefCell<dyn block_device::BlockDevice>>>,
    char_devices: Vec<Rc<RefCell<dyn char_device::CharDevice>>>,
}

impl DevFs {
    pub fn init() -> Self {
        let mut res = DevFs {
            block_devices: Vec::new(),
            char_devices: Vec::new(),
        };

        // Register all block devices.
        for blkdev in block_device::BLOCK_DEVICES.lock().iter() {
            res.register_block_device(blkdev);
        }

        // Register char devices.
        for chrdev in char_device::CHAR_DEVICES.lock().iter() {
            res.register_char_device(chrdev);
        }

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
            assert!(self.char_devices.len() < MAX_CHAR_DEVICES);
            MAX_BLOCK_DEVICES + self.char_devices.len()
        }
    }

    fn resolve_id(&self, id_in_fs: usize) -> ResolveId {
        if id_in_fs < MAX_BLOCK_DEVICES {
            let blkdev_id = id_in_fs;
            let rc_blkdev =
                Rc::clone(&block_device::BLOCK_DEVICES.lock()[blkdev_id]);
            ResolveId::BlockDevice(rc_blkdev)
        } else if id_in_fs < MAX_BLOCK_DEVICES + MAX_CHAR_DEVICES {
            let chrdev_id = id_in_fs - MAX_BLOCK_DEVICES;
            let rc_chrdev =
                Rc::clone(&char_device::CHAR_DEVICES.lock()[chrdev_id]);
            ResolveId::CharDevice(rc_chrdev)
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

    fn register_char_device(
        &mut self,
        chrdev: &Rc<RefCell<dyn char_device::CharDevice>>,
    ) -> usize {
        let id_in_fs = self.allocate_id(false);
        println!(
            "[DEVFS] Registering a char device chr{}.",
            id_in_fs - MAX_BLOCK_DEVICES,
        );
        self.char_devices.push(Rc::clone(chrdev));
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

        for (i, _) in self.block_devices.iter().enumerate() {
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

        for (i, _) in self.char_devices.iter().enumerate() {
            node_mut.maybe_children.as_mut().unwrap().push(Node(Rc::new(
                RefCell::new(NodeInternals {
                    _type: NodeType::CharDevice,
                    name: format!("chr{}", i),
                    id_in_fs: Some(i + MAX_BLOCK_DEVICES),

                    parent: Some(Weak::clone(&node_weak)),
                    maybe_children: None,
                }),
            )));
        }

        drop(node_mut);
        Ok(node)
    }

    fn read_file(
        &self,
        id: usize,
        offset: usize,
        buf: &mut [u8],
    ) -> Result<usize, ReadFileErr> {
        match self.resolve_id(id) {
            ResolveId::BlockDevice(rc_refcell_blkdev) => {
                let blkdev = rc_refcell_blkdev.borrow();

                let start_block = offset / blkdev.block_size();
                let end_block =
                    (offset + buf.len() - 1) / blkdev.block_size() + 1;
                let num_blocks = end_block - start_block;

                let mut tmp_buf = vec![0u8; num_blocks * blkdev.block_size()];

                // FIXME: don't unwrap.
                assert_eq!(
                    blkdev.read_blocks(start_block, &mut tmp_buf).unwrap(),
                    tmp_buf.len(),
                );

                tmp_buf.drain(..offset % blkdev.block_size());
                tmp_buf.truncate(buf.len());
                buf.clone_from_slice(&tmp_buf);
                Ok(buf.len())
            }
            ResolveId::CharDevice(rc_refcell_chrdev) => {
                let mut chrdev = rc_refcell_chrdev.borrow_mut();
                Ok(chrdev.read_many(buf)?)
            }
        }
    }

    fn write_file(
        &self,
        id: usize,
        _offset: usize,
        buf: &[u8],
    ) -> Result<(), WriteFileErr> {
        match self.resolve_id(id) {
            ResolveId::BlockDevice(_) => unimplemented!(),
            ResolveId::CharDevice(rc_refcell_chrdev) => {
                let mut chrdev = rc_refcell_chrdev.borrow_mut();
                chrdev.write_many(buf)?;
            }
        }
        Ok(())
    }

    fn file_size_bytes(&self, _id: usize) -> Result<usize, ReadFileErr> {
        Ok(0)
    }
}

enum ResolveId {
    BlockDevice(Rc<RefCell<dyn block_device::BlockDevice>>),
    CharDevice(Rc<RefCell<dyn char_device::CharDevice>>),
}
