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

use alloc::boxed::Box;
use alloc::vec::Vec;

use crate::arch::interrupts::{InterruptStackFrame, IDT};
use crate::arch::pic::PIC;
use crate::kernel_static::Mutex;
use crate::port::{Port, PortBuilder};

extern "C" {
    // See interrupts.s
    fn irq14_handler(stack_frame: &InterruptStackFrame);
    fn irq15_handler(stack_frame: &InterruptStackFrame);
}

const PORT_IO_BASE: u16 = 0x1F0;
const PORT_CONTROL_BASE: u16 = 0x3F6;

kernel_static! {
    static ref REGISTER_DATA: Port =
        PortBuilder::port(PORT_IO_BASE + 0).size(16).done();
    static ref REGISTER_ERROR: Port =
        PortBuilder::port(PORT_IO_BASE + 1).read_size(8).read_size(16).done();
    static ref REGISTER_FEATURES: Port =
        PortBuilder::port(PORT_IO_BASE + 1).write_size(8).write_size(16).done();
    static ref REGISTER_SECTOR_COUNT: Port =
        PortBuilder::port(PORT_IO_BASE + 2).size(8).size(16).done();
    static ref REGISTER_LBA_LOW: Port =
        PortBuilder::port(PORT_IO_BASE + 3).size(8).size(16).done();
    static ref REGISTER_LBA_MID: Port =
        PortBuilder::port(PORT_IO_BASE + 4).size(8).size(16).done();
    static ref REGISTER_LBA_HIGH: Port =
        PortBuilder::port(PORT_IO_BASE + 5).size(8).size(16).done();
    static ref REGISTER_DRIVE: Port =
        PortBuilder::port(PORT_IO_BASE + 6).size(8).done();
    static ref REGISTER_STATUS: Port =
        PortBuilder::port(PORT_IO_BASE + 7).read_size(8).done();
    static ref REGISTER_COMMAND: Port =
        PortBuilder::port(PORT_IO_BASE + 7).write_size(8).done();

    static ref REGISTER_ALT_STATUS: Port =
        PortBuilder::port(PORT_CONTROL_BASE + 0).read_size(8).done();
    static ref REGISTER_DEVICE_CONTROL: Port =
        PortBuilder::port(PORT_CONTROL_BASE + 0).write_size(8).done();
    static ref REGISTER_DRIVE_ADDRESS: Port =
        PortBuilder::port(PORT_CONTROL_BASE + 1).read_size(8).done();
}

struct Driver {}

impl Driver {
    fn identify(&mut self) -> Box<[u16]> {
        unsafe {
            (*REGISTER_DRIVE).write(0b1111_0000u8);
            (*REGISTER_DEVICE_CONTROL).write(1u8 << 1);
            (*REGISTER_SECTOR_COUNT).write(0u8);
            (*REGISTER_LBA_LOW).write(0u8);
            (*REGISTER_LBA_MID).write(0u8);
            (*REGISTER_LBA_HIGH).write(0u8);
            (*REGISTER_COMMAND).write(0xECu8);

            let status: u8 = (*REGISTER_ALT_STATUS).read();
            println!("status: {:08b}", status);
            assert_ne!(status, 0, "no drive");

            if status & 1 != 0 {
                println!("ERR of status is set");
                let error: u8 = (*REGISTER_ERROR).read();
                println!("Error register: {:08b}", error);

                println!(
                    "Register LBA MID: 0x{:08X}",
                    REGISTER_LBA_MID.read::<u8>(),
                );
                println!(
                    "Register LBA HIGH: 0x{:08X}",
                    REGISTER_LBA_HIGH.read::<u8>(),
                );

                panic!();
            }

            let mut buf: Vec<u16> = Vec::with_capacity(256);
            for _ in 0..256 {
                let word: u16 = (*REGISTER_DATA).read();
                buf.push(word);
            }

            buf.into_boxed_slice()
        }
    }

    fn read(&mut self, lba: u32, num_sectors: u8) -> Box<[u16]> {
        unsafe {
            (*REGISTER_SECTOR_COUNT).write(num_sectors);
            (*REGISTER_LBA_LOW).write(lba as u8);
            (*REGISTER_LBA_MID).write((lba >> 8) as u8);
            (*REGISTER_LBA_HIGH).write((lba >> 16) as u8);
            (*REGISTER_COMMAND).write(0x20u8);
        }

        self.wait_until_ready();

        let buf_len = 256 * num_sectors as usize;
        let mut buf: Vec<u16> = Vec::with_capacity(buf_len);

        for _ in 0..num_sectors {
            for _ in 0..256 {
                let word: u16 = unsafe { (*REGISTER_DATA).read() };
                buf.push(word);
            }
        }

        buf.into_boxed_slice()
    }

    fn wait_until_ready(&self) {
        unsafe {
            let mut status: u8 = (*REGISTER_STATUS).read();

            // BSY?
            while (status >> 7) & 1 != 0 {
                status = (*REGISTER_STATUS).read();
            }

            // DF?
            if (status >> 5) & 1 != 0 {
                panic!("Drive fault error.");
            }

            // ERR?
            if (status >> 0) & 1 != 0 {
                println!("ERR of status is set");
                let error: u8 = (*REGISTER_ERROR).read();
                println!("Error register: {:08b}", error);
                panic!();
            }

            // Wait for DRQ to be set.
            while (status >> 3) & 1 != 1 {
                status = (*REGISTER_STATUS).read();
            }
        }
    }
}

kernel_static! {
    static ref DRIVER: Mutex<Driver> = Mutex::new(Driver {});
}

pub fn init() {
    IDT.lock().interrupts[14].set_handler(irq14_handler);
    IDT.lock().interrupts[15].set_handler(irq15_handler);
    PIC.set_irq_mask(14, false);
    PIC.set_irq_mask(15, false);

    let ident_data = DRIVER.lock().identify();

    for i in 0..256 {
        print!("{:04X} ", ident_data[i]);
    }

    if ident_data[83] & (1 << 10) != 0 {
        println!("LBA48 is supported");
    } else {
        println!("LBA48 is not supported");
    }

    let num28: u32 = ((ident_data[61] as u32) << 16) | ident_data[60] as u32;
    println!("LBA28 addressable sectors: {}", num28);
    let num48: u64 = ((ident_data[103] as u64) << 48)
        | ((ident_data[102] as u64) << 32)
        | ((ident_data[101] as u64) << 16)
        | ident_data[100] as u64;
    println!("LBA48 addressable sectors: {}", num48);

    let sector = DRIVER.lock().read(1, 1);
    for i in sector.iter() {
        print!("{:04X} ", i);
    }
}

#[no_mangle]
pub extern "C" fn ata_irq14_handler() {
    println!("IRQ 14");
    PIC.send_eoi(14);
}

#[no_mangle]
pub extern "C" fn ata_irq15_handler() {
    println!("IRQ 15");
    PIC.send_eoi(15);
}
