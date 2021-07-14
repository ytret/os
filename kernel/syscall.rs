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

use alloc::rc::Rc;

use crate::fs::VFS_ROOT;
use crate::task_manager::TASK_MANAGER;

use crate::fs;
use crate::task::OpenFileErr;

pub fn open(pathname: &str) -> Result<i32, OpenErr> {
    println!("[SYS OPEN] pathname = {:?}", pathname);
    let this_task = unsafe { TASK_MANAGER.this_task() };
    let maybe_node = VFS_ROOT.lock().as_mut().unwrap().path(pathname);
    if let Some(node) = maybe_node {
        match this_task.open_file_by_node(node) {
            Ok(fd) => {
                println!("[SYS OPEN] fd = {} for pid {}", fd, this_task.id);
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

pub fn write(fd: i32, buf: &[u8]) -> Result<usize, WriteErr> {
    let this_task = unsafe { TASK_MANAGER.this_task() };

    // println!("[SYS WRITE] fd = {} by pid {}", fd, this_task.id);
    // println!("[SYS WRITE] buf is at 0x{:08X}", &buf as *const _ as usize);
    // println!("[SYS WRITE] buf len = {}", buf.len());

    if !this_task.check_fd(fd) {
        println!(
            "[SYS WRITE] Invalid file descriptor {} for PID {}.",
            fd, this_task.id,
        );
        Err(WriteErr::BadFd)
    } else {
        let n = this_task.opened_file(fd).write(&buf);
        Ok(n)
    }
}

#[derive(Debug)]
pub enum WriteErr {
    BadFd,
}

pub fn read(fd: i32, buf: &mut [u8]) -> Result<usize, ReadErr> {
    let this_task = unsafe { TASK_MANAGER.this_task() };

    // println!("[SYS READ] fd = {} by task ID {}", fd, this_task.id);
    // println!("[SYS READ] buf is at 0x{:08X}", &buf as *const _ as usize);
    // println!("[SYS READ] buf len = {}", buf.len());

    loop {
        if !this_task.check_fd(fd) {
            println!(
                "[SYS READ] Invalid file descriptor {} for task ID {}.",
                fd, this_task.id,
            );
            return Err(ReadErr::BadFd);
        } else {
            match this_task.opened_file(fd).read(buf) {
                Ok(n) => return Ok(n),
                Err(err) => match err {
                    fs::ReadFileErr::Block => unsafe {
                        TASK_MANAGER.block_this_task();
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

pub fn seek(variant: Seek, fd: i32, offset: usize) -> Result<usize, SeekErr> {
    let this_task = unsafe { TASK_MANAGER.this_task() };
    if !this_task.check_fd(fd) {
        println!(
            "[SYS SEEK] Invalid file descriptor {} for PID {}.",
            fd, this_task.id,
        );
        Err(SeekErr::BadFd)
    } else {
        Ok(match variant {
            Seek::Abs => this_task.opened_file(fd).seek_abs(offset),
            Seek::Rel => this_task.opened_file(fd).seek_rel(offset),
        })
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

pub fn mem_map(
    addr: usize,
    len: usize,
    prot: MemMapProt,
    flags: MemMapFlags,
    fd: i32,
    offset: usize,
) -> Result<usize, MemMapErr> {
    println!(
        "[SYS MEM_MAP] addr = 0x{:08X}, len = 0x{:08X}, prot = {:?}, flags = {:?}, fd = {}, offset = 0x{:08X}",
        addr, len, prot, flags, fd, offset,
    );

    if addr != 0 {
        unimplemented!("syscall mem_map: addr is not 0");
    }
    if fd != -1 {
        unimplemented!("syscall mem_map: fd is not -1");
    }
    if offset != 0 {
        println!("[SYS MEM_MAP] non-zero offset (0x{:X}) is ignored", offset);
    }

    assert_eq!(prot, MemMapProt::READ | MemMapProt::WRITE);
    assert_eq!(flags, MemMapFlags::PRIVATE | MemMapFlags::ANONYMOUS);

    let mapping = unsafe { TASK_MANAGER.this_task().mem_map(len) };

    Ok(mapping.region.start as usize)
}

bitflags_new! {
    pub struct MemMapProt: u32 {
        const NONE = 0b0001;
        const READ = 0b0010;
        const WRITE = 0b0100;
        const EXEC = 0b1000;
    }
}

bitflags_new! {
    pub struct MemMapFlags: u32 {
        const PRIVATE = 0b0001;
        const ANONYMOUS = 0b0010;
        const SHARED = 0b0100;
        const FIXED = 0b1000;
    }
}

#[derive(Debug)]
pub enum MemMapErr {}

pub fn set_tls(ptr: usize) {
    unsafe {
        let this_task = TASK_MANAGER.this_task();
        this_task.set_tls(ptr);
        println!(
            "[SYS SET_TLS] tls_ptr = 0x{:08X} for task ID {}",
            ptr, this_task.id,
        );
    }
}

pub fn debug_print_num(num: u32) {
    println!("[SYS DEBUG_PRINT_NUM] 0x{:08X}", num);
}

pub fn debug_print_str(s: &str) {
    println!("[SYS DEBUG_PRINT_STR] {}", s);
}

pub fn exit(status: i32) -> ! {
    unsafe {
        TASK_MANAGER.terminate_this_task(status);
    }
}

pub fn is_tty(fd: i32) -> Result<bool, IsTtyErr> {
    let this_task = unsafe { TASK_MANAGER.this_task() };
    if !this_task.check_fd(fd) {
        return Err(IsTtyErr::BadFd);
    } else {
        // The char devices (and thus ttys) are currently located only in /dev.
        // Furthermore, they are named tty*.  So the check is fairly easy.
        let f = this_task.opened_file(fd);
        let devfs = VFS_ROOT
            .lock()
            .as_mut()
            .unwrap()
            .child_named("dev")
            .unwrap();
        if !Rc::ptr_eq(&devfs.0, &f.node.mount_point()) {
            Ok(false)
        } else {
            Ok(f.node.0.borrow().name.starts_with("tty"))
        }
    }
}

#[derive(Debug)]
pub enum IsTtyErr {
    BadFd,
}

pub fn get_pid() -> i32 {
    unsafe { TASK_MANAGER.this_task().id as i32 }
}
