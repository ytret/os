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

use alloc::vec::Vec;

pub use crate::arch::process::default_entry_point;
use crate::fs;

pub const MAX_OPENED_FILES: i32 = 32;

pub struct Process {
    pub id: usize,
    pub opened_files: Vec<OpenedFile>,
    new_thread_id: usize,
}

impl Process {
    pub fn new(id: usize) -> Self {
        Process {
            id,
            opened_files: Vec::new(),
            new_thread_id: 0,
        }
    }

    pub fn allocate_thread_id(&mut self) -> usize {
        let id = self.new_thread_id;
        self.new_thread_id += 1;
        id
    }

    pub fn open_file_by_node(
        &mut self,
        node: fs::Node,
    ) -> Result<i32, OpenFileErr> {
        let file_type = node.0.borrow()._type.clone();
        if file_type == fs::NodeType::RegularFile
            || file_type == fs::NodeType::BlockDevice
            || file_type == fs::NodeType::CharDevice
        {
            if self.opened_files.len() == MAX_OPENED_FILES as usize {
                return Err(OpenFileErr::MaxOpenedFiles);
            }
            let fd = self.opened_files.len() as i32;
            self.opened_files
                .push(OpenedFile::new(node.clone(), file_type.is_seekable()));
            Ok(fd)
        } else {
            Err(OpenFileErr::UnsupportedFileType)
        }
    }

    pub fn opened_file(&mut self, fd: i32) -> &mut OpenedFile {
        &mut self.opened_files[fd as usize]
    }

    pub fn check_fd(&self, fd: i32) -> bool {
        return 0 <= fd && fd < self.opened_files.len() as i32;
    }
}

#[derive(Debug)]
pub enum OpenFileErr {
    MaxOpenedFiles,
    UnsupportedFileType,
}

pub struct OpenedFile {
    node: fs::Node,
    offset: Option<usize>,
}

impl OpenedFile {
    fn new(node: fs::Node, seekable: bool) -> Self {
        OpenedFile {
            node,
            offset: if seekable { Some(0) } else { None },
        }
    }

    fn seek(&mut self, add_offset: usize) {
        if let Some(offset) = self.offset.as_mut() {
            *offset += add_offset;
        }
    }

    pub fn read(&mut self, buf: &mut [u8]) {
        let fs = self.node.fs();
        let id_in_fs = self.node.0.borrow().id_in_fs.unwrap();
        let res = fs
            .read_file(id_in_fs, self.offset.unwrap_or(0), buf.len())
            .unwrap();
        self.seek(buf.len());
        buf.clone_from_slice(&res);
    }

    pub fn write(&mut self, buf: &[u8]) {
        let fs = self.node.fs();
        let id_in_fs = self.node.0.borrow().id_in_fs.unwrap();
        fs.write_file(id_in_fs, self.offset.unwrap_or(0), buf)
            .unwrap();
        self.seek(buf.len());
    }
}
