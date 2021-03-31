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

use crate::fs;
use crate::process::OpenFileErr;

macro_rules! running_process {
    () => {
        unsafe { SCHEDULER.running_process() }
    };
}

pub fn open(pathname: &str) -> Result<i32, OpenErr> {
    println!("[SYS OPEN] pathname = {:?}", pathname);
    let maybe_node = VFS_ROOT.lock().as_mut().unwrap().path(pathname);
    if let Some(node) = maybe_node {
        match running_process!().open_file_by_node(node) {
            Ok(fd) => {
                println!(
                    "[SYS OPEN] fd = {} for pid {}",
                    fd,
                    running_process!().id,
                );
                Ok(fd)
            }
            Err(err) => {
                println!("[SYS OPEN] Could not open the node: {:?}.", err);
                Err(err.into())
            }
        }
    } else {
        println!("[SYS OPEN] Node not found.");
        Err(OpenErr::NotFound)
    }
}

#[derive(Debug)]
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
    // println!("[SYS WRITE] fd = {} by pid {}", fd, running_process!().id);
    // println!("[SYS WRITE] buf is at 0x{:08X}", &buf as *const _ as usize);
    // println!("[SYS WRITE] buf len = {}", buf.len());

    if !running_process!().check_fd(fd) {
        println!(
            "[SYS WRITE] Invalid file descriptor {} for PID {}.",
            fd,
            running_process!().id,
        );
        Err(WriteErr::BadFd)
    } else {
        running_process!().opened_file(fd).write(&buf);
        Ok(())
    }
}

#[derive(Debug)]
pub enum WriteErr {
    BadFd,
}

pub fn read(fd: i32, buf: &mut [u8]) -> Result<(), ReadErr> {
    // println!("[SYS READ] fd = {} by pid {}", fd, running_process!().id);
    // println!("[SYS READ] buf is at 0x{:08X}", &buf as *const _ as usize);
    // println!("[SYS READ] buf len = {}", buf.len());

    loop {
        if !running_process!().check_fd(fd) {
            println!(
                "[SYS READ] Invalid file descriptor {} for PID {}.",
                fd,
                running_process!().id,
            );
            return Err(ReadErr::BadFd);
        } else {
            match running_process!().opened_file(fd).read(buf) {
                Ok(_) => return Ok(()),
                Err(err) => match err {
                    fs::ReadFileErr::Block => unsafe {
                        SCHEDULER.block_running_thread();
                    },
                    fs::ReadFileErr::NotReadable => {
                        return Err(ReadErr::NotReadable);
                    }
                    other => unimplemented!("FIXME: handle {:?}", other),
                },
            }
        }
    }
}

#[derive(Debug)]
pub enum ReadErr {
    BadFd,
    NotReadable,
}

pub fn seek(variant: Seek, fd: i32, offset: usize) -> Result<(), SeekErr> {
    if !running_process!().check_fd(fd) {
        println!(
            "[SYS SEEK] Invalid file descriptor {} for PID {}.",
            fd,
            running_process!().id,
        );
        Err(SeekErr::BadFd)
    } else {
        match variant {
            Seek::Abs => running_process!().opened_file(fd).seek_abs(offset),
            Seek::Rel => running_process!().opened_file(fd).seek_rel(offset),
        }
        Ok(())
    }
}

#[derive(Debug)]
pub enum Seek {
    Abs,
    Rel,
}

#[derive(Debug)]
pub enum SeekErr {
    BadFd,
}
