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
use crate::scheduler::SCHEDULER;

use crate::bitflags::BitFlags;
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

pub fn write(fd: i32, buf: &[u8]) -> Result<usize, WriteErr> {
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
        let n = running_process!().opened_file(fd).write(&buf);
        Ok(n)
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

pub fn seek(variant: Seek, fd: i32, offset: usize) -> Result<usize, SeekErr> {
    if !running_process!().check_fd(fd) {
        println!(
            "[SYS SEEK] Invalid file descriptor {} for PID {}.",
            fd,
            running_process!().id,
        );
        Err(SeekErr::BadFd)
    } else {
        Ok(match variant {
            Seek::Abs => running_process!().opened_file(fd).seek_abs(offset),
            Seek::Rel => running_process!().opened_file(fd).seek_rel(offset),
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
    prot: BitFlags<u32, MemMapProt>,
    flags: BitFlags<u32, MemMapFlags>,
    fd: i32,
    offset: usize,
) -> Result<usize, MemMapErr> {
    println!(
        "[SYS MEM_MAP] addr = 0x{:08X}, len = 0x{:08X}, prot = {}, flags = {}, fd = {}, offset = 0x{:08X}",
        addr, len, prot.value, flags.value, fd, offset,
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

    let mut readable = false;
    let mut writable = false;

    let mut prot_left = prot;
    if prot_left.has_set(MemMapProt::None) {
        prot_left.unset_flag(MemMapProt::None);
    }
    if prot_left.has_set(MemMapProt::Read) {
        readable = true;
        prot_left.unset_flag(MemMapProt::Read);
    }
    if prot_left.has_set(MemMapProt::Write) {
        writable = true;
        prot_left.unset_flag(MemMapProt::Write);
    }
    if prot_left.has_set(MemMapProt::Exec) {
        unimplemented!("syscall mem_map: MemMapProt::Exec");
    }
    assert_eq!(prot_left.value, 0, "unknown prot: {}", prot_left.value);

    let mut private = false;
    let mut anonymous = false;

    let mut flags_left = flags;
    if flags_left.has_set(MemMapFlags::Private) {
        private = true;
        flags_left.unset_flag(MemMapFlags::Private);
    }
    if flags_left.has_set(MemMapFlags::Anonymous) {
        anonymous = true;
        flags_left.unset_flag(MemMapFlags::Anonymous);
    }
    if flags_left.has_set(MemMapFlags::Shared) {
        unimplemented!("syscall mem_map: MemMapFlags::Shared");
    }
    if flags_left.has_set(MemMapFlags::Fixed) {
        unimplemented!("syscall mem_map: MemMapFlags::Fixed");
    }
    assert_eq!(flags_left.value, 0, "unknown flags: {}", flags_left.value);

    assert!(readable && writable);
    assert!(private && anonymous);

    let mapping = unsafe { SCHEDULER.running_process().mem_map(len) };

    Ok(mapping.region.start as usize)
}

bitflags! {
    #[repr(u32)]
    pub enum MemMapProt {
        None = 1 << 0,
        Read = 1 << 1,
        Write = 1 << 2,
        Exec = 1 << 3,
    }
}

bitflags! {
    #[repr(u32)]
    pub enum MemMapFlags {
        Private = 1 << 0,
        Anonymous = 1 << 1,
        Shared = 1 << 2,
        Fixed = 1 << 3,
    }
}

#[derive(Debug)]
pub enum MemMapErr {}

pub fn set_tls(ptr: usize) {
    let this_thread = unsafe { SCHEDULER.running_thread() };
    this_thread.tls_ptr = Some(ptr as usize);
    println!(
        "[SYS SET_TLS] tls_ptr = 0x{:08X} for pid {} tid {}",
        this_thread.tls_ptr.unwrap(),
        this_thread.process_id,
        this_thread.id,
    );
}

pub fn get_tls() -> usize {
    let this_thread = unsafe { SCHEDULER.running_thread() };
    println!(
        "[SYS GET_TLS] tls_ptr = 0x{:08X} for pid {} tid {}",
        this_thread.tls_ptr.unwrap(),
        this_thread.process_id,
        this_thread.id,
    );
    this_thread.tls_ptr.unwrap()
}

pub fn debug_print_num(num: u32) {
    println!("[SYS DEBUG_PRINT_NUM] 0x{:08X}", num);
}

pub fn debug_print_str(s: &str) {
    println!("[SYS DEBUG_PRINT_STR] {}", s);
}

pub fn exit(status: i32) -> ! {
    unsafe {
        SCHEDULER.terminate_running_thread(status);
    }
}

pub fn is_tty(fd: i32) -> Result<bool, IsTtyErr> {
    if !running_process!().check_fd(fd) {
        return Err(IsTtyErr::BadFd);
    } else {
        // The char devices (and thus ttys) are currently located only in /dev.
        // Furthermore, they are named tty*.  So the check is fairly easy.
        let f = running_process!().opened_file(fd);
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
