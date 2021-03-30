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

use crate::fs::VFS_ROOT;
use crate::scheduler::SCHEDULER;

use crate::process::OpenFileErr;

macro_rules! current_process {
    () => {
        unsafe { SCHEDULER.current_process() }
    };
}

pub fn open(pathname: &str) -> Result<i32, OpenErr> {
    println!("[SYS OPEN] pathname = {:?}", pathname);
    let maybe_node = VFS_ROOT.lock().as_mut().unwrap().path(pathname);
    if let Some(node) = maybe_node {
        match current_process!().open_file_by_node(node) {
            Ok(fd) => {
                println!("[SYS OPEN] fd = {}", fd);
                Ok(fd)
            }
            Err(err) => {
                println!("[SYS OPEN] Could not open the node: {:?}", err);
                Err(err.into())
            }
        }
    } else {
        println!("[SYS OPEN] Node not found.");
        Err(OpenErr::NotFound)
    }
}

pub enum OpenErr {
    NotFound,
    MaxOpenedFiles,
    UnsupportedFileType,
}

impl From<OpenFileErr> for OpenErr {
    fn from(err: OpenFileErr) -> Self {
        match err {
            OpenFileErr::MaxOpenedFiles => OpenErr::MaxOpenedFiles,
            OpenFileErr::UnsupportedFileType => OpenErr::UnsupportedFileType,
        }
    }
}

pub fn write(fd: i32, buf: &[u8]) -> Result<(), WriteErr> {
    // println!("[SYS WRITE] fd = {}", fd);
    // println!("[SYS WRITE] buf is at 0x{:08X}", &buf as *const _ as usize);
    // println!("[SYS WRITE] buf len = {}", buf.len());

    if !current_process!().check_fd(fd) {
        println!("[SYS WRITE] Invalid file descriptor.");
        Err(WriteErr::BadFd)
    } else {
        current_process!().opened_file(fd).write(&buf);
        Ok(())
    }
}

pub enum WriteErr {
    BadFd,
}

pub fn read(fd: i32, buf: &mut [u8]) -> Result<(), ReadErr> {
    // println!("[SYS READ] fd = {}", fd);
    // println!("[SYS READ] buf is at 0x{:08X}", &buf as *const _ as usize);
    // println!("[SYS READ] buf len = {}", buf.len());

    if !current_process!().check_fd(fd) {
        println!("[SYS READ] Invalid file descriptor.");
        Err(ReadErr::BadFd)
    } else {
        current_process!().opened_file(fd).read(buf);
        Ok(())
    }
}

pub enum ReadErr {
    BadFd,
}
