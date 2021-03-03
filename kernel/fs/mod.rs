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
use alloc::rc::Rc;
use alloc::string::{FromUtf8Error, String};
use alloc::vec::Vec;
use core::fmt;

use crate::disk;

#[derive(Debug)]
pub struct Node {
    _type: NodeType,
    name: String,
    id_in_fs: Option<usize>,
    maybe_children: Option<Vec<Node>>,
}

enum NodeType {
    MountPoint(Rc<disk::Disk>),
    RegularFile,
    Dir,
}

impl fmt::Debug for NodeType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            NodeType::MountPoint(_) => "MountPoint",
            NodeType::RegularFile => "RegularFile",
            NodeType::Dir => "Dir",
        })
    }
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
}

pub trait FileSystem {
    fn root_dir(&self) -> Result<Node, ReadDirErr>;
    fn read_dir(&self, id: usize) -> Result<Node, ReadDirErr>;
    fn read_file(&self, id: usize) -> Result<Vec<Box<[u8]>>, ReadFileErr>;
    fn file_size_bytes(&self, id: usize) -> Result<u64, ReadFileErr>;
    fn file_size_blocks(&self, id: usize) -> Result<usize, ReadFileErr>;
}
