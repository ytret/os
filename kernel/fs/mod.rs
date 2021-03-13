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

pub mod ext2;

use alloc::boxed::Box;
use alloc::rc::{Rc, Weak};
use alloc::string::{FromUtf8Error, String};
use alloc::vec::Vec;
use core::cell::RefCell;
use core::fmt;

use crate::disk;

#[derive(Clone, Debug)]
pub struct Node(pub Rc<RefCell<NodeInternals>>);

#[derive(Clone, Debug)]
pub struct NodeInternals {
    pub _type: NodeType,
    name: String,
    pub id_in_fs: Option<usize>,
    parent: Option<Weak<RefCell<NodeInternals>>>,
    pub maybe_children: Option<Vec<Node>>,
}

impl NodeInternals {
    fn is_mount_point(&self) -> bool {
        if let NodeType::MountPoint(_) = self._type {
            true
        } else {
            false
        }
    }

    fn has_parent(&self) -> bool {
        if let Some(_) = self.parent {
            true
        } else {
            false
        }
    }
}

impl Node {
    /// Searches for the first [`MountPoint`](NodeType) among the parent nodes.
    ///
    /// # Notes
    /// If this node is a mount point, there will be no search and this node
    /// will be returned.
    ///
    /// # Panics
    /// This method panics if it could not find any mount point parent node or
    /// if any of the parent nodes has been deallocated.
    fn mount_point(&self) -> Rc<RefCell<NodeInternals>> {
        let mut current = Rc::clone(&self.0);
        loop {
            if current.borrow().is_mount_point() {
                return current;
            } else if current.borrow().has_parent() {
                let weak = current.borrow().parent.as_ref().unwrap().clone();
                current = weak.upgrade().unwrap();
            } else {
                panic!("could not find any mount point");
            }
        }
    }

    /// Returns a [`FileSystem`] which this node resides on.
    pub fn fs(&self) -> Rc<Box<dyn FileSystem>> {
        let mp_node = self.mount_point();
        let mp_node_internals = mp_node.borrow();
        if let NodeType::MountPoint(disk_id) = mp_node_internals._type {
            let disk = &disk::DISKS.lock()[disk_id];
            Rc::clone(disk.file_system.as_ref().unwrap())
        } else {
            unreachable!();
        }
    }

    /// Returns the children of the node.
    ///
    /// # Panics
    /// This method panics if:
    /// * the node is not a directory node,
    /// * it has `id_in_fs` unset, or
    /// * it is named `..`.
    pub fn children(&mut self) -> Vec<Node> {
        assert_eq!(self.0.borrow()._type, NodeType::Dir);
        assert_ne!(self.0.borrow().name, String::from(".."));
        let asdasd = self.0.borrow().maybe_children.clone();
        if let Some(children) = asdasd {
            children.clone()
        } else {
            let fs = self.fs();
            let id_in_fs = self.0.borrow().id_in_fs.unwrap();
            let node = fs.read_dir(id_in_fs).unwrap(); // FIXME: no panic
            let some_clone = node.0.borrow().maybe_children.clone();
            self.0.borrow_mut().maybe_children = some_clone;
            self.0.borrow().maybe_children.as_ref().unwrap().clone()
        }
    }

    /// Returns the `nth` child node of the node.
    ///
    /// # Panics
    /// See [`Node::children()`].
    pub fn child(&mut self, nth: usize) -> Node {
        self.children()[nth].clone()
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum NodeType {
    MountPoint(usize),
    RegularFile,
    Dir,
}

#[derive(Debug)]
pub enum ReadDirErr {
    NoRwInterface,
    DiskErr(disk::ReadErr),
    InvalidName(FromUtf8Error),
    InvalidDescriptor,
}

impl From<FromUtf8Error> for ReadDirErr {
    fn from(err: FromUtf8Error) -> Self {
        ReadDirErr::InvalidName(err)
    }
}

#[derive(Debug)]
pub enum ReadFileErr {
    NoRwInterface,
    DiskErr(disk::ReadErr),
    InvalidBlockNum, // FIXME: is this ext2-specific?
    InvalidOffsetOrLength,
}

pub trait FileSystem {
    fn root_dir(&self) -> Result<Node, ReadDirErr>;
    fn read_dir(&self, id: usize) -> Result<Node, ReadDirErr>;

    fn read_file(&self, id: usize) -> Result<Vec<u8>, ReadFileErr>;
    fn read_file_offset_len(
        &self,
        id: usize,
        offset: usize,
        len: usize,
    ) -> Result<Vec<u8>, ReadFileErr>;

    fn file_size_bytes(&self, id: usize) -> Result<usize, ReadFileErr>;
    fn file_size_blocks(&self, id: usize) -> Result<usize, ReadFileErr>;
}
