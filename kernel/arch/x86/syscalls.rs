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

use core::slice;
use core::str;

use crate::arch::interrupts::InterruptStackFrame;
use crate::fs::VFS_ROOT;
use crate::scheduler::SCHEDULER;

#[derive(Debug)]
pub struct GpRegisters {
    edi: u32,
    esi: u32,
    ebp: u32,
    esp: u32,
    ebx: u32,
    edx: u32,
    ecx: u32,
    eax: u32,
}

#[no_mangle]
pub extern "C" fn syscall_handler(
    _stack_frame: &InterruptStackFrame,
    gp_regs: &GpRegisters,
) {
    println!("[SYS] Syscall number: {}", gp_regs.eax);
    println!("{:#010X?}", gp_regs);
    let current_process = unsafe { SCHEDULER.current_process() };

    // 0 open
    // ebx: pathname, *const u8
    // ecx: pathname len, u32
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
                Ok(fd) => println!("[SYS OPEN] fd = {}", fd),
                Err(err) => println!("[SYS OPEN] err: {:?}", err),
            }
        } else {
            // FIXME: return something.
            unimplemented!();
        }
    }

    // 1 write
    // ebx: fd, u32
    // ecx: buffer pointer, *const u8
    // edx: buffer size in bytes, u32
    if gp_regs.eax == 1 {
        let fd = gp_regs.ebx as usize;
        let buf = unsafe {
            slice::from_raw_parts(
                gp_regs.ecx as *const u8,
                gp_regs.edx as usize,
            )
        };
        println!("[SYS WRITE] fd = {}", fd);
        println!("[SYS WRITE] buf is at 0x{:08X}", &buf as *const _ as usize);
        println!("[SYS WRITE] buf len = {}", buf.len());

        current_process.opened_files[fd].write(&buf);
    }
}
