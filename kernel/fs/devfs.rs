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

use crate::disk;

use super::{
    FileSystem, FsWrapper, Mountable, Node, NodeInternals, NodeType,
    ReadDirErr, ReadFileErr,
};

pub struct DevFs {}

impl FileSystem for DevFs {
    fn root_dir(&self) -> Result<Node, ReadDirErr> {
        self.read_dir(0)
    }

    fn read_dir(&self, id: usize) -> Result<Node, ReadDirErr> {
        // There is only one directory currently.
        assert_eq!(id, 0, "invalid id");

        let node = Node(Rc::new(RefCell::new(NodeInternals {
            _type: NodeType::Dir,
            name: String::from("/"),
            id_in_fs: Some(0),

            parent: None,
            maybe_children: Some(Vec::new()),
        })));
        let node_weak = Rc::downgrade(&node.0);
        let mut node_mut = node.0.borrow_mut();

        for (i, disk) in disk::DISKS.lock().iter().enumerate() {
            println!("[DEVFS] Creating a block device disk{}.", i);
            node_mut.maybe_children.as_mut().unwrap().push(Node(Rc::new(
                RefCell::new(NodeInternals {
                    _type: NodeType::BlockDevice,
                    name: format!("disk{}", i),
                    id_in_fs: Some(i + 1),

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
    /// * [`disk::ReadWriteInterface::read_blocks()`] returns an error.
    fn read_file_offset_len(
        &self,
        id: usize,
        offset: usize,
        len: usize,
    ) -> Result<Vec<u8>, ReadFileErr> {
        assert!(id > 0 && id - 1 < disk::DISKS.lock().len(), "invalid id");
        let disk_id = id - 1;
        let disk = &disk::DISKS.lock()[disk_id];
        let rwif = &disk.rw_interface;

        let mut res_buf = Vec::with_capacity(len);
        let start_block = offset / rwif.block_size();
        let end_block = (offset + len - 1) / rwif.block_size() + 1;
        let num_blocks = end_block - start_block;

        for block in rwif.read_blocks(start_block, num_blocks) {
            res_buf.extend_from_slice(&block);
        }

        res_buf.drain(0..offset % rwif.block_size());
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
