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

use core::slice;
use core::str;

use crate::fs::VFS_ROOT;
use crate::scheduler::SCHEDULER;

use crate::arch::interrupts::InterruptStackFrame;
use crate::process::OpenFileErr;

#[derive(Debug)]
pub struct GpRegs {
    edi: u32,
    esi: u32,
    ebp: u32,
    esp: u32,
    ebx: u32,
    edx: u32,
    ecx: u32,
    eax: u32,
}

const OPEN_ENOENT: i32 = -1;
const OPEN_EMFILE: i32 = -2;
const OPEN_EINVAL: i32 = -3;

const WRITE_EBADF: i32 = -1;

const READ_EBADF: i32 = -1;

#[no_mangle]
pub extern "C" fn syscall_handler(
    _stack_frame: &InterruptStackFrame,
    gp_regs: &mut GpRegs,
) {
    // println!("[SYS] Syscall number: {}", gp_regs.eax);
    // println!("{:#010X?}", gp_regs);
    let current_process = unsafe { SCHEDULER.current_process() };
    let return_value: i32;

    // 0 open
    // ebx: pathname, *const u8
    // ecx: pathname len, u32
    // returns: fd or error number, i32
    if gp_regs.eax == 0 {
        let pathname = unsafe {
            let bytes = slice::from_raw_parts(
                gp_regs.ebx as *const u8,
                gp_regs.ecx as usize,
            );
            str::from_utf8(&bytes).unwrap()
        };
        println!("[SYS OPEN] pathname = {:?}", pathname);
        let maybe_node = VFS_ROOT.lock().as_mut().unwrap().path(pathname);
        if let Some(node) = maybe_node {
            match current_process.open_file_by_node(node) {
                Ok(fd) => {
                    println!("[SYS OPEN] fd = {}", fd);
                    return_value = fd;
                }
                Err(err) => {
                    println!("[SYS OPEN] Could not open the node: {:?}", err);
                    return_value = match err {
                        OpenFileErr::MaxOpenedFiles => OPEN_EMFILE,
                        OpenFileErr::UnsupportedFileType => OPEN_EINVAL,
                    };
                }
            }
        } else {
            println!("[SYS OPEN] Node not found.");
            return_value = OPEN_ENOENT;
        }
    }
    // 1 write
    // ebx: fd, i32
    // ecx: buffer pointer, *const u8
    // edx: buffer size in bytes, u32
    // returns 0 or error number, i32
    else if gp_regs.eax == 1 {
        let fd = gp_regs.ebx as i32;
        let buf = unsafe {
            slice::from_raw_parts(
                gp_regs.ecx as *const u8,
                gp_regs.edx as usize,
            )
        };

        // println!("[SYS WRITE] fd = {}", fd);
        // println!("[SYS WRITE] buf is at 0x{:08X}", &buf as *const _ as usize);
        // println!("[SYS WRITE] buf len = {}", buf.len());

        if !current_process.check_fd(fd) {
            println!("[SYS WRITE] Invalid file descriptor.");
            return_value = WRITE_EBADF;
        } else {
            current_process.opened_file(fd).write(&buf);
            return_value = 0;
        }
    }
    // 2 read
    // ebx: fd, i32
    // ecx: buffer pointer, *mut u8
    // edx: buffer size in bytes, u32
    // returns FIXME
    else if gp_regs.eax == 2 {
        let fd = gp_regs.ebx as i32;
        let mut buf = unsafe {
            slice::from_raw_parts_mut(
                gp_regs.ecx as *mut u8,
                gp_regs.edx as usize,
            )
        };

        // println!("[SYS READ] fd = {}", fd);
        // println!("[SYS READ] buf is at 0x{:08X}", &buf as *const _ as usize);
        // println!("[SYS READ] buf len = {}", buf.len());

        if !current_process.check_fd(fd) {
            println!("[SYS READ] Invalid file descriptor.");
            return_value = READ_EBADF;
        } else {
            current_process.opened_file(fd).read(&mut buf);
            return_value = 0;
        }
    } else {
        println!("[SYS] Ignoring an invalid syscall number.");
        return_value = 0;
    }

    gp_regs.eax = return_value as u32;
}
