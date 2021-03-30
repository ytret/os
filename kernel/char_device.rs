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

use alloc::boxed::Box;
use alloc::rc::Rc;
use alloc::vec::Vec;
use core::cell::RefCell;

use crate::fs::{ReadFileErr, WriteFileErr};
use crate::kernel_static::Mutex;

pub trait CharDevice {
    fn read(&mut self) -> Result<u8, ReadErr>;
    fn read_many(&mut self, len: usize) -> Result<Box<[u8]>, ReadErr>;

    fn write(&mut self, byte: u8) -> Result<(), WriteErr>;
    fn write_many(&mut self, bytes: &[u8]) -> Result<(), WriteErr>;
}

#[derive(Debug)]
pub enum ReadErr {
    NotReadable,
    InvalidLen,
    Block,
}

impl From<ReadErr> for ReadFileErr {
    fn from(err: ReadErr) -> Self {
        match err {
            ReadErr::NotReadable => ReadFileErr::NotReadable,
            ReadErr::InvalidLen => ReadFileErr::InvalidOffsetOrLen,
            ReadErr::Block => ReadFileErr::Block,
        }
    }
}

#[derive(Debug)]
pub enum WriteErr {
    NotWritable,
}

impl From<WriteErr> for WriteFileErr {
    fn from(err: WriteErr) -> Self {
        match err {
            WriteErr::NotWritable => WriteFileErr::NotWritable,
        }
    }
}

kernel_static! {
    pub static ref CHAR_DEVICES: Mutex<Vec<Rc<RefCell<dyn CharDevice>>>>
        = Mutex::new(Vec::new());
}
