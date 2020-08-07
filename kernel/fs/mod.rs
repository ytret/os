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
use alloc::string::{FromUtf8Error, String};
use alloc::vec::Vec;

use crate::disk::{ReadErr, ReadWriteInterface};

#[derive(Debug)]
pub struct Directory {
    id: usize,
    name: String,
    entries: Vec<DirEntry>,
}

#[derive(Debug)]
struct DirEntry {
    id: usize,
    name: String,
    content: DirEntryContent,
}

#[derive(Debug)]
enum DirEntryContent {
    Unknown,
    RegularFile,
    Directory,
}

#[derive(Debug)]
pub enum ReadDirErr {
    DiskErr(ReadErr),
    InvalidName(FromUtf8Error),
}

impl From<ReadErr> for ReadDirErr {
    fn from(err: ReadErr) -> Self {
        ReadDirErr::DiskErr(err)
    }
}

impl From<FromUtf8Error> for ReadDirErr {
    fn from(err: FromUtf8Error) -> Self {
        ReadDirErr::InvalidName(err)
    }
}

#[derive(Debug)]
pub enum ReadFileErr {
    DiskErr(ReadErr),
}

impl From<ReadErr> for ReadFileErr {
    fn from(err: ReadErr) -> Self {
        ReadFileErr::DiskErr(err)
    }
}

#[derive(Debug)]
pub enum FileSizeErr {
    DiskErr(ReadErr),
}

impl From<ReadErr> for FileSizeErr {
    fn from(err: ReadErr) -> Self {
        FileSizeErr::DiskErr(err)
    }
}

pub trait FileSystem {
    fn root_dir(
        &self,
        rw_interface: &Box<dyn ReadWriteInterface>,
    ) -> Result<Directory, ReadDirErr>;

    fn read_dir(
        &self,
        id: usize,
        rw_interface: &Box<dyn ReadWriteInterface>,
    ) -> Result<Directory, ReadDirErr>;

    fn read_file(
        &self,
        id: usize,
        rw_interface: &Box<dyn ReadWriteInterface>,
    ) -> Result<Vec<Box<[u8]>>, ReadFileErr>;

    fn file_size_bytes(
        &self,
        id: usize,
        rw_interface: &Box<dyn ReadWriteInterface>,
    ) -> Result<u64, FileSizeErr>;

    fn file_size_blocks(
        &self,
        id: usize,
        rw_interface: &Box<dyn ReadWriteInterface>,
    ) -> Result<usize, FileSizeErr>;
}
