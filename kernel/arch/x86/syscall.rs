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

use core::mem::size_of;
use core::slice;
use core::str;

use crate::scheduler::SCHEDULER;

use crate::arch::gdt;
use crate::arch::interrupts::InterruptStackFrame;
use crate::arch::process::jump_into_usermode;
use crate::bitflags::BitFlags;
use crate::syscall;
use crate::thread::Thread;

#[derive(Clone, Copy, Debug)]
#[repr(C, packed)]
pub struct GpRegs {
    // NOTE: the field order is hard-coded in scheduler.s.
    pub edi: u32,
    pub esi: u32,
    pub ebp: u32,
    pub esp: u32,
    pub ebx: u32,
    pub edx: u32,
    pub ecx: u32,
    pub eax: u32,
}

const EBADF: i32 = -1;
const EINVAL: i32 = -2;
const EMFILE: i32 = -3;
const ENOENT: i32 = -4;
const ENOTTY: i32 = -5;

#[no_mangle]
pub extern "C" fn syscall_handler(
    stack_frame: &InterruptStackFrame,
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
                syscall::OpenErr::NotFound => ENOENT,
                syscall::OpenErr::MaxOpenedFiles => EMFILE,
                syscall::OpenErr::UnsupportedFileType => EINVAL,
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
            Ok(n) => n as i32,
            Err(err) => match err {
                syscall::WriteErr::BadFd => EBADF,
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
            Ok(n) => n as i32,
            Err(err) => match err {
                syscall::ReadErr::BadFd => EBADF,
                syscall::ReadErr::NotReadable => EINVAL,
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
                syscall::SeekErr::BadFd => EBADF,
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
                    syscall::SeekErr::BadFd => EBADF,
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
    }
    // 11 is_tty
    // ebx: fd, i32
    // returns 1 or error number
    else if syscall_num == 11 {
        let fd = gp_regs.ebx as i32;
        return_value = match syscall::is_tty(fd) {
            Ok(res) => {
                if res {
                    1
                } else {
                    ENOTTY
                }
            }
            Err(err) => match err {
                syscall::IsTtyErr::BadFd => EBADF,
            },
        }
    }
    // 12 get_pid
    // returns process ID
    else if syscall_num == 12 {
        return_value = unsafe { SCHEDULER.running_process().id } as i32;
    }
    // 13 fork
    else if syscall_num == 13 {
        unsafe {
            println!(
                "[SYS FORK] Origin pid {} tid {}",
                SCHEDULER.running_process().id,
                SCHEDULER.running_thread().id,
            );

            let copy_id = SCHEDULER.allocate_process_id();
            let mut copy = SCHEDULER.running_process().clone(copy_id);

            // Define these before moving `copy' into SCHEDULER.
            let thread_id = copy.allocate_thread_id();
            let copy_vas_cr3 = copy.vas.pgdir_phys;

            SCHEDULER.add_process(copy);

            let mut thread = Thread::new_with_stack(
                copy_id,
                thread_id,
                jump_into_usermode as u32,
                5 + size_of::<GpRegs>(),
                // pass five arguments to jump_into_usermode and store the GP
                // registers on the stack.  FIXME: memory leak
            );
            // FIXME: bad API
            thread.tcb.cr3 = copy_vas_cr3;

            let kernel_stack_top = thread.tcb.esp as *mut u32;

            let mut new_gp_regs = gp_regs.clone();
            new_gp_regs.eax = 0; // syscall return value for the child process
            new_gp_regs.ebp = stack_frame.ebp;
            new_gp_regs.esp = stack_frame.esp;

            let new_gp_regs_ptr: *mut GpRegs =
                kernel_stack_top.wrapping_add(14).cast();
            new_gp_regs_ptr.write(new_gp_regs);

            println!("[SYS FORK] new_gp_regs = {:#X?}", new_gp_regs);

            *kernel_stack_top.wrapping_add(9) = gdt::USERMODE_CODE_SEG as u32;
            *kernel_stack_top.wrapping_add(10) = gdt::USERMODE_DATA_SEG as u32;
            *kernel_stack_top.wrapping_add(11) = gdt::TLS_SEG as u32;
            *kernel_stack_top.wrapping_add(12) = stack_frame.eip;
            *kernel_stack_top.wrapping_add(13) = new_gp_regs_ptr as u32;

            println!("[fork] esp = 0x{:08X}", stack_frame.esp);
            println!("[fork] eip = 0x{:08X}", stack_frame.eip);

            SCHEDULER.add_runnable_thread(thread);

            println!("[SYS FORK] Copy pid {} tid {}", copy_id, thread_id);

            return_value = copy_id as i32;
        }
    } else {
        println!("[SYS] Ignoring an invalid syscall number {}.", syscall_num);
        return_value = 0;
    }

    gp_regs.eax = return_value as u32;
}
