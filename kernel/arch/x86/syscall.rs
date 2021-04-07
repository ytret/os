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

use crate::arch::interrupts::InterruptStackFrame;
use crate::bitflags::BitFlags;
use crate::syscall;

#[derive(Clone, Copy, Debug)]
#[repr(C, packed)]
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
const READ_EINVAL: i32 = -2;

const SEEK_EBADF: i32 = -1;

#[no_mangle]
pub extern "C" fn syscall_handler(
    _stack_frame: &InterruptStackFrame,
    gp_regs: &mut GpRegs,
) {
    // println!(
    //     "[SYS] Syscall number {} by PID {}",
    //     gp_regs.eax,
    //     unsafe { SCHEDULER.running_process().id },
    // );
    // println!("{:#010X?}", gp_regs);
    let syscall_num: u32 = { gp_regs.eax };
    let return_value: i32;

    // 0 open
    // ebx: pathname, *const u8
    // ecx: pathname len, u32
    // returns fd or error number, i32
    if syscall_num == 0 {
        let pathname = unsafe {
            let bytes = slice::from_raw_parts(
                gp_regs.ebx as *const u8,
                gp_regs.ecx as usize,
            );
            str::from_utf8(&bytes).unwrap()
        };
        return_value = match syscall::open(pathname) {
            Ok(fd) => fd,
            Err(err) => match err {
                syscall::OpenErr::NotFound => OPEN_ENOENT,
                syscall::OpenErr::MaxOpenedFiles => OPEN_EMFILE,
                syscall::OpenErr::UnsupportedFileType => OPEN_EINVAL,
            },
        };
    }
    // 1 write
    // ebx: fd, i32
    // ecx: buffer pointer, *const u8
    // edx: buffer size in bytes, u32
    // returns 0 or error number, i32
    else if syscall_num == 1 {
        let fd = gp_regs.ebx as i32;
        let buf = unsafe {
            slice::from_raw_parts(
                gp_regs.ecx as *const u8,
                gp_regs.edx as usize,
            )
        };
        return_value = match syscall::write(fd, buf) {
            Ok(_) => 0,
            Err(err) => match err {
                syscall::WriteErr::BadFd => WRITE_EBADF,
            },
        };
    }
    // 2 read
    // ebx: fd, i32
    // ecx: buffer pointer, *mut u8
    // edx: buffer size in bytes, u32
    // returns 0 or error number, i32
    else if syscall_num == 2 {
        let fd = gp_regs.ebx as i32;
        let buf = unsafe {
            slice::from_raw_parts_mut(
                gp_regs.ecx as *mut u8,
                gp_regs.edx as usize,
            )
        };
        return_value = match syscall::read(fd, buf) {
            Ok(_) => 0,
            Err(err) => match err {
                syscall::ReadErr::BadFd => READ_EBADF,
                syscall::ReadErr::NotReadable => READ_EINVAL,
            },
        };
    }
    // 3 seek_abs
    // ebx: fd, i32
    // ecx: new offset, u32
    // returns 0 or error number, i32
    else if syscall_num == 3 {
        let fd = gp_regs.ebx as i32;
        let new_offset = gp_regs.ecx as usize;
        return_value = match syscall::seek(syscall::Seek::Abs, fd, new_offset) {
            Ok(new_offset) => new_offset as i32,
            Err(err) => match err {
                syscall::SeekErr::BadFd => SEEK_EBADF,
            },
        };
    }
    // 4 seek_rel
    // ebx: fd, i32
    // ecx: add to offset, u32
    // returns 0 or error number, i32
    else if syscall_num == 4 {
        let fd = gp_regs.ebx as i32;
        let add_to_offset = gp_regs.ecx as usize;
        return_value =
            match syscall::seek(syscall::Seek::Rel, fd, add_to_offset) {
                Ok(new_offset) => new_offset as i32,
                Err(err) => match err {
                    syscall::SeekErr::BadFd => SEEK_EBADF,
                },
            };
    }
    // 5 mem_map
    // ebx: args, *const struct, where struct is:
    //     addr, u32
    //     len, u32
    //     prot, u32
    //     flags, u32
    //     fd, i32
    //     offset, u32
    // return value: FIXME:
    else if syscall_num == 5 {
        let args =
            unsafe { slice::from_raw_parts(gp_regs.ebx as *const u32, 6) };

        let addr = args[0] as usize;
        let len = args[1] as usize;
        let prot = BitFlags::<u32, syscall::MemMapProt>::new(args[2]);
        let flags = BitFlags::<u32, syscall::MemMapFlags>::new(args[3]);
        let fd = args[4] as i32;
        let offset = args[5] as usize;

        return_value =
            match syscall::mem_map(addr, len, prot, flags, fd, offset) {
                Ok(ptr) => ptr as i32,
                Err(_) => unimplemented!(),
            };
    }
    // 6 set_tls
    // ebx: a pointer to the TLS, u32
    // returns 0
    else if syscall_num == 6 {
        let ptr = gp_regs.ebx as usize;
        syscall::set_tls(ptr);
        return_value = 0;
    }
    // 7 get_tls
    // returns the pointer to the TLS, u32
    else if syscall_num == 7 {
        return_value = syscall::get_tls() as i32;
    }
    // 8 debug_print_num
    // ebx: num, u32
    // returns 0
    else if syscall_num == 8 {
        let num = gp_regs.ebx;
        syscall::debug_print_num(num);
        return_value = 0;
    }
    // 9 debug_print_str
    // ebx: string, *const u8
    // ecx: string len, u32
    // returns 0
    else if syscall_num == 9 {
        let string = unsafe {
            let bytes = slice::from_raw_parts(
                gp_regs.ebx as *const u8,
                gp_regs.ecx as usize,
            );
            str::from_utf8(&bytes).unwrap()
        };
        syscall::debug_print_str(string);
        return_value = 0;
    }
    // 10 exit
    // ebx: exit status, i32
    // does not return
    else if syscall_num == 10 {
        let status = gp_regs.ebx as i32;
        syscall::exit(status);
    } else {
        println!("[SYS] Ignoring an invalid syscall number {}.", syscall_num);
        return_value = 0;
    }

    gp_regs.eax = return_value as u32;
}
