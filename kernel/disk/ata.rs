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
use core::slice;

use crate::arch::interrupts::{InterruptStackFrame, IDT};
use crate::arch::pic::PIC;
use crate::disk::{Disk, ReadErr, WriteErr};
use crate::port::{Port, PortBuilder};

extern "C" {
    // See interrupts.s
    fn irq14_handler(stack_frame: &InterruptStackFrame);
    fn irq15_handler(stack_frame: &InterruptStackFrame);
}

pub struct Bus {
    registers: Registers,
    selected_drive: BusDrive,
    drives: [Option<Drive>; 2],
}

impl Bus {
    fn new(port_io_base: u16, port_control_base: u16) -> Self {
        let mut bus = Bus {
            registers: Registers::new(port_io_base, port_control_base),
            selected_drive: BusDrive::Master,
            drives: [None, None],
        };

        bus.enable_lba();
        bus.disable_interrupts();

        // Master drive.
        match bus.identify() {
            Some(data) => {
                let master = Drive::from_identify_data(&data);
                if master.num_sectors_lba28 != 0 {
                    bus.drives[0] = Some(master);
                } else {
                    println!(
                        "[ATA] Ignoring master drive without LBA28 support."
                    );
                }
            }
            None => println!("[ATA] No master drive found."),
        }

        // Slave drive.
        bus.select_drive(BusDrive::Slave);
        match bus.identify() {
            Some(data) => {
                let slave = Drive::from_identify_data(&data);
                if slave.num_sectors_lba28 != 0 {
                    bus.drives[1] = Some(slave);
                } else {
                    println!(
                        "[ATA] Ignoring slave drive without LBA28 support."
                    );
                }
            }
            None => println!("[ATA] No slave drive found."),
        }

        bus
    }

    fn selected_drive(&self) -> Option<Drive> {
        match self.selected_drive {
            BusDrive::Master => self.drives[0],
            BusDrive::Slave => self.drives[1],
        }
    }

    fn select_drive(&mut self, drive: BusDrive) {
        if self.selected_drive != drive {
            unsafe {
                let mut val: u8 = self.registers.drive.read();
                val &= !(1 << 4); // DRV
                val |= (matches!(drive, BusDrive::Slave) as u8) << 4;
                self.registers.drive.write(val);
            }
            self.selected_drive = drive;
        }
    }

    fn identify(&mut self) -> Option<Box<[u16]>> {
        unsafe {
            self.registers.sector_count.write(0u8);
            self.set_lba(0);
            self.registers.command.write(0xECu8);

            let status: u8 = self.registers.alt_status.read();
            if status == 0 {
                println!("[ATA] Drive does not exist.");
                return None;
            }

            if status & 1 != 0 {
                let lba_8: u8 = self.registers.lba_8.read();
                let lba_16: u8 = self.registers.lba_16.read();
                if lba_8 == 0 && lba_16 == 0 {
                    let error: u8 = self.registers.error.read();
                    println!(
                        "[ATA] Identify command aborted. Error: {:08b}",
                        error,
                    );
                    panic!();
                } else {
                    println!("[ATA] Ignoring ATAPI or SATA drive.");
                    return None;
                }
            }

            let mut buf: Vec<u16> = Vec::with_capacity(256);
            for _ in 0..256 {
                let word: u16 = self.registers.data.read();
                buf.push(word);
            }

            Some(buf.into_boxed_slice())
        }
    }

    fn wait_until_ready(&self) {
        unsafe {
            let mut status: u8 = self.registers.status.read();

            // BSY?
            while (status >> 7) & 1 != 0 {
                status = self.registers.status.read();
            }

            // DF?
            if (status >> 5) & 1 != 0 {
                panic!("Drive fault error.");
            }

            // ERR?
            if (status >> 0) & 1 != 0 {
                println!("[ATA] ERR of status is set");
                let error: u8 = self.registers.error.read();
                println!("[ATA] Error register: {:08b}", error);
                panic!();
            }

            // Wait for DRQ to be set.
            while (status >> 3) & 1 != 1 {
                status = self.registers.status.read();
            }
        }
    }

    fn enable_lba(&self) {
        // NOTE: This does not check if the bus supports LBA addressing mode.
        unsafe {
            let mut drive: u8 = self.registers.drive.read();
            drive |= 1 << 6; // LBA
            self.registers.drive.write(drive);
        }
    }

    fn disable_interrupts(&self) {
        let nien: u8 = 1 << 1; // nIEN
        unsafe {
            self.registers.device_control.write(nien);
        }
    }

    fn set_lba(&self, lba: u32) {
        assert_eq!(lba & (0xF << 27), 0, "bits 28-31 of LBA must be clear");
        unsafe {
            self.registers.lba_0.write(lba as u8);
            self.registers.lba_8.write((lba >> 8) as u8);
            self.registers.lba_16.write((lba >> 16) as u8);
            let lba_24 = (lba >> 24) as u8;
            let mut was: u8 = self.registers.drive.read();
            was &= !(0xF);
            was |= lba_24 & 0xF;
            self.registers.drive.write(was);
        }
    }

    fn read(&self, lba: u32, num_sectors: u8) -> Box<[u16]> {
        unsafe {
            self.registers.sector_count.write(num_sectors);
            self.set_lba(lba);
            self.registers.command.write(0x20u8);
        }

        self.wait_until_ready();

        let buf_len = 256 * num_sectors as usize;
        let mut buf: Vec<u16> = Vec::with_capacity(buf_len);

        for _ in 0..num_sectors {
            for _ in 0..256 {
                let word: u16 = unsafe { self.registers.data.read() };
                buf.push(word);
            }
        }

        buf.into_boxed_slice()
    }

    fn write(&self, lba: u32, num_sectors: u8, data: &[u16]) {
        assert_eq!(num_sectors as usize * 256, data.len(), "invalid data size");
        unsafe {
            self.registers.sector_count.write(num_sectors);
            self.set_lba(lba);
            self.registers.command.write(0x30u8);
        }
        self.wait_until_ready();
        for &word in data {
            unsafe {
                self.registers.data.write(word);
            }
        }
    }
}

#[inline(always)]
fn boxed_slice_u16_to_u8(from: Box<[u16]>) -> Box<[u8]> {
    unsafe {
        // FIXME: endiannes?
        let slice_u16: &[u16] = &*Box::into_raw(from);
        let slice_u8: &[u8] = slice::from_raw_parts(
            slice_u16.as_ptr() as *const u8,
            2 * slice_u16.len(),
        );
        Box::from_raw(slice_u8 as *const _ as *mut [u8]) // same box
    }
}

#[inline(always)]
fn slice_u8_to_u16(from: &[u8]) -> &[u16] {
    assert_eq!(from.len() % 2, 0, "invalid size of slice `from`");
    unsafe {
        // FIXME: endiannes?
        let raw_u8: *const u8 = from.as_ptr();
        slice::from_raw_parts(raw_u8 as *const u16, from.len() / 2)
    }
}

impl Disk for Bus {
    fn has_sector(&self, sector_idx: usize) -> bool {
        let maybe_drive = self.selected_drive();
        match maybe_drive {
            Some(drive) => {
                !((sector_idx != 0 && sector_idx as u32 == 0)
                    || sector_idx as u32 >= drive.num_sectors_lba28)
            }
            None => false,
        }
    }

    fn read_sector(&self, sector_idx: usize) -> Result<Box<[u8]>, ReadErr> {
        let maybe_drive = self.selected_drive();
        match maybe_drive {
            Some(_) => {
                if !self.has_sector(sector_idx) {
                    Err(ReadErr::NoSuchSector)
                } else {
                    let data = self.read(sector_idx as u32, 1);
                    Ok(boxed_slice_u16_to_u8(data))
                }
            }
            None => Err(ReadErr::DiskUnavailable),
        }
    }

    fn read_sectors(
        &self,
        first_sector_idx: usize,
        num_sectors: usize,
    ) -> Result<Box<[u8]>, ReadErr> {
        if num_sectors == 0 {
            return Err(ReadErr::ZeroNumSectors);
        }

        let maybe_drive = self.selected_drive();
        match maybe_drive {
            Some(_) => {
                let last_sector_idx = first_sector_idx + num_sectors - 1;
                if !self.has_sector(first_sector_idx) {
                    Err(ReadErr::NoSuchSector)
                } else if !self.has_sector(last_sector_idx)
                    || (num_sectors != 0 && num_sectors as u8 == 0)
                {
                    Err(ReadErr::TooMuchSectors)
                } else {
                    let data =
                        self.read(first_sector_idx as u32, num_sectors as u8);
                    Ok(boxed_slice_u16_to_u8(data))
                }
            }
            None => Err(ReadErr::DiskUnavailable),
        }
    }

    fn write_sector(
        &self,
        sector_idx: usize,
        data: [u8; 512],
    ) -> Result<(), WriteErr> {
        // FIXME: OS-specific sector size is 512?
        let maybe_drive = self.selected_drive();
        match maybe_drive {
            Some(_) => {
                if !self.has_sector(sector_idx) {
                    Err(WriteErr::NoSuchSector)
                } else {
                    let data: &[u16] = slice_u8_to_u16(&data);
                    self.write(sector_idx as u32, 1, data);
                    Ok(())
                }
            }
            None => Err(WriteErr::DiskUnavailable),
        }
    }

    fn write_sectors(
        &self,
        first_sector_idx: usize,
        data: &[u8],
    ) -> Result<(), WriteErr> {
        if data.len() == 0 {
            return Err(WriteErr::EmptyDataPassed);
        }

        // FIXME: OS-specific sector size is 512?
        assert_eq!(data.len() % 512, 0, "invalid data size");
        let num_sectors = data.len() / 512;

        let maybe_drive = self.selected_drive();
        match maybe_drive {
            Some(_) => {
                let last_sector_idx = first_sector_idx + num_sectors - 1;
                if !self.has_sector(first_sector_idx) {
                    Err(WriteErr::NoSuchSector)
                } else if !self.has_sector(last_sector_idx)
                    || (num_sectors != 0 && num_sectors as u8 == 0)
                {
                    Err(WriteErr::TooMuchSectors)
                } else {
                    let data = slice_u8_to_u16(data);
                    self.write(
                        first_sector_idx as u32,
                        num_sectors as u8,
                        data,
                    );
                    Ok(())
                }
            }
            None => Err(WriteErr::DiskUnavailable),
        }
    }
}

#[allow(dead_code)]
struct Registers {
    data: Port,
    error: Port,
    features: Port,
    sector_count: Port,
    lba_0: Port,
    lba_8: Port,
    lba_16: Port,
    drive: Port,
    status: Port,
    command: Port,

    alt_status: Port,
    device_control: Port,
    drive_address: Port,
}

impl Registers {
    fn new(port_io_base: u16, port_control_base: u16) -> Self {
        Registers {
            data: PortBuilder::port(port_io_base + 0).size(16).done(),
            error: PortBuilder::port(port_io_base + 1)
                .read_size(8)
                .read_size(16)
                .done(),
            features: PortBuilder::port(port_io_base + 1)
                .write_size(8)
                .write_size(16)
                .done(),
            sector_count: PortBuilder::port(port_io_base + 2)
                .size(8)
                .size(16)
                .done(),
            lba_0: PortBuilder::port(port_io_base + 3).size(8).size(16).done(),
            lba_8: PortBuilder::port(port_io_base + 4).size(8).size(16).done(),
            lba_16: PortBuilder::port(port_io_base + 5).size(8).size(16).done(),
            drive: PortBuilder::port(port_io_base + 6).size(8).done(),
            status: PortBuilder::port(port_io_base + 7).read_size(8).done(),
            command: PortBuilder::port(port_io_base + 7).write_size(8).done(),

            alt_status: PortBuilder::port(port_control_base + 0)
                .read_size(8)
                .done(),
            device_control: PortBuilder::port(port_control_base + 0)
                .write_size(8)
                .done(),
            drive_address: PortBuilder::port(port_control_base + 1)
                .read_size(8)
                .done(),
        }
    }
}

#[derive(PartialEq)]
enum BusDrive {
    Master,
    Slave,
}

#[derive(Clone, Copy)]
struct Drive {
    supports_lba48: bool,
    num_sectors_lba28: u32,
    num_sectors_lba48: u64,
}

impl Drive {
    fn from_identify_data(data: &[u16]) -> Self {
        assert_eq!(data.len(), 256, "invalid identify data");
        Drive {
            supports_lba48: data[83] & (1 << 10) != 0,
            num_sectors_lba28: ((data[61] as u32) << 16) | data[60] as u32,
            num_sectors_lba48: ((data[103] as u64) << 48)
                | ((data[102] as u64) << 32)
                | ((data[101] as u64) << 16)
                | data[100] as u64,
        }
    }
}

const ATA0_PORT_IO_BASE: u16 = 0x1F0;
const ATA0_PORT_CONTROL_BASE: u16 = 0x3F6;

pub unsafe fn init() -> Vec<Bus> {
    // SAFETY: This does not check if there are actually ATA ports at the
    // standard places.  If they are not there, it means either that they are
    // somewhere else or that there is no IDE controller.

    IDT.lock().interrupts[14].set_handler(irq14_handler);
    IDT.lock().interrupts[15].set_handler(irq15_handler);
    PIC.set_irq_mask(14, false);
    PIC.set_irq_mask(15, false);

    let mut bus = Bus::new(ATA0_PORT_IO_BASE, ATA0_PORT_CONTROL_BASE);
    bus.select_drive(BusDrive::Slave);

    vec![bus]
}

#[no_mangle]
pub extern "C" fn ata_irq14_handler() {
    println!("[ATA] IRQ 14");
    PIC.send_eoi(14);
}

#[no_mangle]
pub extern "C" fn ata_irq15_handler() {
    println!("[ATA] IRQ 15");
    PIC.send_eoi(15);
}
