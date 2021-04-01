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

pub mod devfs;
pub mod ext2;

use alloc::rc::{Rc, Weak};
use alloc::string::{FromUtf8Error, String};
use alloc::vec::Vec;
use core::cell::RefCell;
use core::cmp;
use core::fmt;

use crate::disk;
use crate::kernel_static::Mutex;

#[derive(Clone, Debug)]
pub struct Node(pub Rc<RefCell<NodeInternals>>);

/// Internals of a node.
///
/// # `..` node
/// For directories, there must be exactly one child named `..`.  For mount
/// points, there must be no such child.
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
    pub fn fs(&self) -> Rc<dyn FileSystem> {
        let mp_node = self.mount_point();
        let mp_node_internals = mp_node.borrow();
        if let NodeType::MountPoint(mountable) = mp_node_internals._type.clone()
        {
            mountable.borrow().fs()
        } else {
            unreachable!();
        }
    }

    /// Returns all children of the node.
    ///
    /// # Panics
    /// This method panics if the node:
    /// * is not a directory node or a mount point node,
    /// * is named `..`, or
    /// * has `id_in_fs` unset.
    pub fn children(&mut self) -> Vec<Node> {
        assert!(
            self.0.borrow()._type == NodeType::Dir
                || self.0.borrow().is_mount_point(),
        );
        assert_ne!(self.0.borrow().name, String::from(".."));
        if self.0.borrow().maybe_children.is_some() {
            self.0.borrow().maybe_children.as_ref().unwrap().clone()
        } else {
            let fs = self.fs();
            let id_in_fs = self.0.borrow().id_in_fs.unwrap();
            let node = fs.read_dir(id_in_fs).unwrap(); // FIXME: no panic

            // Set the parent of the node.
            node.0.borrow_mut().parent = self.0.borrow().parent.clone();

            // We don't clone the maybe_children Vec of node, but rather make
            // self an Rc to node's RefCell.  That's because in the first case
            // one would not only add a clone overhead, but also have to update
            // the parent field of each node's child.  In the second case,
            // however, one simply updates the Rc pointing to self in the self's
            // parent, as it is done below.
            let old_rc = self.0.clone();
            self.0 = node.0;

            if let Some(parent_weak) = &self.0.borrow().parent {
                let parent = parent_weak.upgrade().unwrap();
                let mut parent_internals = parent.borrow_mut();

                // Find the index of self among the parent's children.
                let parent_children =
                    parent_internals.maybe_children.as_mut().unwrap();
                let self_idx = parent_children
                    .iter()
                    .position(|x| Rc::ptr_eq(&x.0, &old_rc))
                    .unwrap();
                parent_children[self_idx] = self.clone();
            }

            self.0.borrow().maybe_children.as_ref().unwrap().clone()
        }
    }

    /// Returns the `nth` child of the node.
    ///
    /// # Panics
    /// See [`Node::children()`].
    pub fn child(&mut self, nth: usize) -> Node {
        self.children()[nth].clone()
    }

    /// Returns the child named `name`.
    ///
    /// # Panics
    /// See [`Node::children()`].
    pub fn child_named(&mut self, name: &str) -> Option<Node> {
        for child in self.children() {
            if child.0.borrow().name == name {
                return Some(child);
            }
        }
        None
    }

    /// Returns `true` if the node has children nodes named other than `..`.
    ///
    /// # Panics
    /// See [`Node::children()`].
    pub fn has_children(&mut self) -> bool {
        if self.children().len() == 1 {
            self.child(0).0.borrow().name != ".."
        } else if self.children().len() > 1 {
            true
        } else {
            false
        }
    }

    /// Replaces the specified child node internals with the root node internals
    /// of a [`Mountable`], adjusting the latter to imitate a child directory.
    ///
    /// The children nodes of the mount point are also modified so that they
    /// consider the adjusted node internals their parent.
    ///
    /// # Panics
    /// This method panics if:
    /// * there is no child with the specified name,
    /// * the child is not an empty directory,
    /// * see also [`Node::children()`] and [`FileSystem::root_dir()`].
    pub fn mount_on_child(
        &mut self,
        child_name: &str,
        mountable: Rc<RefCell<dyn Mountable>>,
    ) {
        let maybe_child = self.child_named(child_name);
        let mut child = maybe_child.unwrap();
        assert_eq!(child.0.borrow()._type, NodeType::Dir);
        assert!(!child.has_children());

        let mut mount_node = mountable.borrow().fs().root_dir().unwrap();
        mount_node.0.borrow_mut()._type =
            NodeType::MountPoint(Rc::clone(&mountable));
        mount_node.0.borrow_mut().name = String::from(child_name);
        mount_node.0.borrow_mut().parent = Some(Rc::downgrade(&child.0));
        child.0.replace(mount_node.0.borrow().clone());
        let child_weak = Rc::downgrade(&child.0);

        // Adjust the mount point children.
        for mp_child in mount_node.children() {
            mp_child.0.borrow_mut().parent = Some(Weak::clone(&child_weak));
        }
    }

    pub fn path(&mut self, path: &str) -> Option<Node> {
        let mut current = self.clone();
        let last_is_dir = path.ends_with("/");
        for elem in path.split("/") {
            if !elem.is_empty() {
                if let Some(child) = current.child_named(elem) {
                    current = child;
                } else {
                    return None;
                }
            }
        }
        if last_is_dir && current.0.borrow()._type != NodeType::Dir {
            return None;
        }
        Some(current)
    }
}

#[derive(Clone)]
pub enum NodeType {
    MountPoint(Rc<RefCell<dyn Mountable>>),
    Dir,
    RegularFile,
    BlockDevice,
    CharDevice,
}

impl NodeType {
    pub fn is_seekable(&self) -> bool {
        self.clone() == NodeType::RegularFile
            || self.clone() == NodeType::BlockDevice
    }
}

impl cmp::PartialEq for NodeType {
    fn eq(&self, other: &Self) -> bool {
        if let NodeType::MountPoint(rc1) = self {
            if let NodeType::MountPoint(rc2) = other {
                Rc::as_ptr(&rc1) == Rc::as_ptr(&rc2)
            } else {
                false
            }
        } else if let NodeType::Dir = self {
            if let NodeType::Dir = other {
                true
            } else {
                false
            }
        } else if let NodeType::RegularFile = self {
            if let NodeType::RegularFile = other {
                true
            } else {
                false
            }
        } else if let NodeType::BlockDevice = self {
            if let NodeType::BlockDevice = other {
                true
            } else {
                false
            }
        } else if let NodeType::CharDevice = self {
            if let NodeType::CharDevice = other {
                true
            } else {
                false
            }
        } else {
            unreachable!();
        }
    }
}

impl fmt::Debug for NodeType {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NodeType::MountPoint(_) => fmt.write_str("MountPoint(_)"),
            NodeType::Dir => fmt.write_str("Dir"),
            NodeType::RegularFile => fmt.write_str("RegularFile"),
            NodeType::BlockDevice => fmt.write_str("BlockDevice"),
            NodeType::CharDevice => fmt.write_str("CharDevice"),
        }
    }
}

pub trait Mountable {
    fn fs(&self) -> Rc<dyn FileSystem>;
}

pub trait FileSystem {
    fn root_dir(&self) -> Result<Node, ReadDirErr>;
    fn read_dir(&self, id: usize) -> Result<Node, ReadDirErr>;

    fn read_file(
        &self,
        id: usize,
        offset: usize,
        len: usize,
    ) -> Result<Vec<u8>, ReadFileErr>;

    fn write_file(
        &self,
        id: usize,
        offset: usize,
        buf: &[u8],
    ) -> Result<(), WriteFileErr>;

    fn file_size_bytes(&self, id: usize) -> Result<usize, ReadFileErr>;
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
    InvalidOffsetOrLen,
    NotReadable,
    Block,
}

#[derive(Debug)]
pub enum WriteFileErr {
    NotWritable,
}

pub struct FsWrapper(Rc<dyn FileSystem>);

impl Mountable for FsWrapper {
    fn fs(&self) -> Rc<dyn FileSystem> {
        Rc::clone(&self.0)
    }
}

kernel_static! {
    pub static ref VFS_ROOT: Mutex<Option<Node>> = Mutex::new(None);
    pub static ref DEV_FS: Mutex<Option<Rc<RefCell<FsWrapper>>>> = Mutex::new(None);
}

/// Initializes the VFS root on the specified disk.
///
/// # Locks
/// This function accesses the mutexes:
/// * [`static@disk::DISKS`] and
/// * [`static@VFS_ROOT`].
///
/// # Panics
/// This function panics if:
/// * there is no disk with the specified ID (see [`static@disk::DISKS`]) or
/// * a file system on the specified disk is already initialized and thus the
///   root node cannot be acquired by [`disk::Disk::try_init_fs`].
pub fn init_vfs_root_on_disk(disk_id: usize) {
    assert!(disk_id < disk::DISKS.lock().len(), "invalid disk id");

    // Make up the VFS root node.
    let mut root_node = {
        let disks = disk::DISKS.lock();
        let mut disk = disks[disk_id].borrow_mut();
        disk.try_init_fs().unwrap()
    };
    let mountable = Rc::clone(&disk::DISKS.lock()[disk_id]);
    root_node.0.borrow_mut()._type = NodeType::MountPoint(mountable);

    // Initialize devfs on /dev.
    println!("[VFS] Initializing devfs on /dev.");
    *DEV_FS.lock() = Some(Rc::new(RefCell::new(FsWrapper(Rc::new(
        devfs::DevFs::init(),
    )))));
    let mountable = Rc::clone(DEV_FS.lock().as_ref().unwrap());
    root_node.mount_on_child("dev", mountable);

    *VFS_ROOT.lock() = Some(root_node);
}
