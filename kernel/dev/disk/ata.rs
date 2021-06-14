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
use alloc::vec::Vec;
use core::cell::RefCell;
use core::mem::align_of;
use core::slice;

use crate::arch::interrupts::{InterruptStackFrame, IDT, STAGE2_IRQ15_HANDLER};
use crate::arch::dev::pic::PIC;
use crate::dev::disk::{ReadErr, ReadWriteInterface, WriteErr};
use crate::port::{Port, PortBuilder};

extern "C" {
    // See interrupts.s
    fn irq14_handler();
    fn irq15_handler();
}

pub struct Bus {
    registers: Registers,
    selected_drive: DriveId,
}

impl Bus {
    fn new(port_io_base: u16, port_control_base: u16) -> Self {
        Bus {
            registers: Registers::new(port_io_base, port_control_base),
            selected_drive: DriveId::Master,
        }
    }

    fn init_and_get_drives(&mut self) -> [Option<Drive>; 2] {
        let mut drives = [None, None];
        self.enable_lba();
        self.disable_interrupts();

        // Master drive.
        match self.identify() {
            Some(data) => {
                let master = Drive::from_identify_data(DriveId::Master, &data);
                if master.num_sectors_lba28 != 0 {
                    drives[0] = Some(master);
                    println!("[ATA] Found a master drive.");
                } else {
                    println!(
                        "[ATA] Ignoring a master drive without LBA28 support."
                    );
                }
            }
            None => println!("[ATA] No master drive found."),
        }

        // Slave drive.
        self.select_drive(DriveId::Slave);
        match self.identify() {
            Some(data) => {
                let slave = Drive::from_identify_data(DriveId::Slave, &data);
                if slave.num_sectors_lba28 != 0 {
                    drives[1] = Some(slave);
                    println!("[ATA] Found a slave drive.");
                } else {
                    println!(
                        "[ATA] Ignoring a slave drive without LBA28 support."
                    );
                }
            }
            None => println!("[ATA] No slave drive found."),
        }

        drives
    }

    fn select_drive(&mut self, drive: DriveId) {
        if drive != self.selected_drive {
            unsafe {
                let mut val: u8 = self.registers.drive.read();
                val &= !(1 << 4); // DRV
                val |= (matches!(drive, DriveId::Slave) as u8) << 4;
                self.registers.drive.write(val);
                // FIXME: 400ns delay?
            }
            self.selected_drive = drive;
        }
    }

    fn identify(&mut self) -> Option<[u16; 256]> {
        unsafe {
            self.registers.sector_count.write(0u8);
            self.set_lba(0);
            self.registers.command.write(0xECu8);

            let status: u8 = self.registers.alt_status.read();
            if status == 0 {
                println!("[ATA] Drive does not exist.");
                return None;
            }

            // Wait for BSY to be unset.
            while self.registers.status.read::<u8>() & (1 << 7) != 0 {}

            // ERR?
            if status & 1 != 0 {
                let lba_8: u8 = self.registers.lba_8.read();
                let lba_16: u8 = self.registers.lba_16.read();
                if lba_8 == 0 && lba_16 == 0 {
                    let error: u8 = self.registers.error.read();
                    println!(
                        "[ATA] Identify command aborted. Error: {:08b}.",
                        error,
                    );
                    return None;
                } else {
                    println!("[ATA] Ignoring an ATAPI or SATA drive.");
                    return None;
                }
            }

            self.wait_until_ready();

            let mut buf = [0u16; 256];
            for i in 0..256 {
                buf[i] = self.registers.data.read();
            }

            Some(buf)
        }
    }

    fn check_for_errors(&self) {
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
        }
    }

    fn wait_until_ready(&self) {
        unsafe {
            let mut status: u8 = self.registers.status.read();
            // Check the status for errors.
            self.check_for_errors();
            // Wait for DRQ to be set.
            while (status >> 3) & 1 != 1 {
                status = self.registers.status.read();
            }
        }
    }

    fn enable_lba(&self) {
        // FIXME: this does not check if the bus supports the LBA addressing
        // mode.
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

    fn read(&self, lba: u32, buf: &mut [u8]) -> usize {
        assert_ne!(buf.len(), 0, "cannot read into an empty buffer");
        assert_eq!(
            buf.len() % 512,
            0,
            "buffer length must be a multiple of 512",
        );
        let num_sectors = (buf.len() / 512) as u8;
        assert_ne!(num_sectors, 0, "too many sectors to read");

        self.check_for_errors();

        unsafe {
            self.registers.sector_count.write(num_sectors);
            self.set_lba(lba);
            self.registers.command.write(0x20u8);
        }

        for i in 0..num_sectors {
            self.wait_until_ready();
            for j in 0..256 {
                let word: u16 = unsafe { self.registers.data.read() };
                let idx = (i as usize) * 512 + j * 2;
                buf[idx] = word as u8;
                buf[idx + 1] = (word >> 8) as u8;
            }
        }

        buf.len()
    }

    fn write(&self, lba: u32, num_sectors: u8, data: &[u16]) {
        assert_eq!(data.len(), num_sectors as usize * 256, "invalid data size");
        self.check_for_errors();
        unsafe {
            self.registers.sector_count.write(num_sectors);
            self.set_lba(lba);
            self.registers.command.write(0x30u8);
        }
        self.wait_until_ready();
        for (i, &word) in data.iter().enumerate() {
            if i % 256 == 0 {
                self.wait_until_ready();
            }
            unsafe {
                self.registers.data.write(word);
            }
        }
    }
}

#[inline(always)]
fn slice_u8_to_u16(from: &[u8]) -> &[u16] {
    assert_eq!(from.len() % 2, 0, "invalid size of slice `from`");
    unsafe {
        // FIXME: endianness?
        let raw_u8: *const u8 = from.as_ptr();
        assert_eq!(raw_u8 as usize, align_of::<&[u16]>(), "alignment error");
        slice::from_raw_parts(raw_u8 as *const u16, from.len() / 2)
    }
}

#[derive(Clone, Copy, PartialEq)]
enum DriveId {
    Master,
    Slave,
}

#[derive(Clone)]
pub struct Drive {
    // 1) First, an Option is used because Bus::init_etc. cannot set this field
    //    so that it points to itself and due do that it's None.  However init()
    //    immediately sets it properly, thus one can safely assume it's Some.
    // 2) Second, an Rc is used because an ATA bus has a master and a slave
    //    drives which are separate Disks for the kernel; both point to the same
    //    Bus, so a shared pointer is necessary.
    // 3) Third, a RefCell is used for interior mutability with runtime checks:
    //    interior mutability allows the Drive methods to mutate its Bus state
    //    without the Drive itself being mutable, otherwise the
    //    ReadWriteInterface methods would need to be mutable as well, and that
    //    would make things even harder; using compile-time checks seemed
    //    impossible to me for a similar reason.
    bus: Option<Rc<RefCell<Bus>>>,
    id: DriveId,
    supports_lba48: bool,
    num_sectors_lba28: u32,
    num_sectors_lba48: u64,
}

impl Drive {
    fn from_identify_data(id: DriveId, data: &[u16]) -> Self {
        assert_eq!(data.len(), 256, "invalid identify data");
        Drive {
            bus: None,
            id,
            supports_lba48: data[83] & (1 << 10) != 0,
            num_sectors_lba28: ((data[61] as u32) << 16) | data[60] as u32,
            num_sectors_lba48: ((data[103] as u64) << 48)
                | ((data[102] as u64) << 32)
                | ((data[101] as u64) << 16)
                | data[100] as u64,
        }
    }
}

impl ReadWriteInterface for Drive {
    fn block_size(&self) -> usize {
        // NOTE: this must correlate with the argument `data` of
        // ReadWriteInterface::write_block().
        512
    }

    fn has_block(&self, block_idx: usize) -> bool {
        !((block_idx != 0 && block_idx as u32 == 0)
            || block_idx as u32 >= self.num_sectors_lba28)
    }

    fn read_block(
        &self,
        block_idx: usize,
        buf: &mut [u8],
    ) -> Result<usize, ReadErr> {
        let mut bus = self.bus.as_ref().unwrap().borrow_mut();
        bus.select_drive(self.id);
        if self.has_block(block_idx) {
            Ok(bus.read(block_idx as u32, buf))
        } else {
            Err(ReadErr::NoSuchBlock)
        }
    }

    fn read_blocks(
        &self,
        first_block_idx: usize,
        buf: &mut [u8],
    ) -> Result<usize, ReadErr> {
        assert_eq!(buf.len() % self.block_size(), 0); // FIXME: Err(...)

        let num_blocks = buf.len() / self.block_size();
        if num_blocks == 0 {
            // FIXME: InvalidBuf?  InvalidBufLen?
            return Err(ReadErr::InvalidNumBlocks);
        }
        if num_blocks as u8 == 0 {
            return Err(ReadErr::TooMuchBlocks);
        }

        let mut bus = self.bus.as_ref().unwrap().borrow_mut();
        bus.select_drive(self.id);

        if self.has_block(first_block_idx) {
            Ok(bus.read(first_block_idx as u32, buf))
        } else {
            Err(ReadErr::NoSuchBlock)
        }
    }

    fn write_block(
        &self,
        block_idx: usize,
        data: [u8; 512],
    ) -> Result<(), WriteErr> {
        let mut bus = self.bus.as_ref().unwrap().borrow_mut();
        bus.select_drive(self.id);
        if !self.has_block(block_idx) {
            Err(WriteErr::NoSuchBlock)
        } else {
            let data: &[u16] = slice_u8_to_u16(&data);
            bus.write(block_idx as u32, 1, data);
            Ok(())
        }
    }

    fn write_blocks(
        &self,
        first_block_idx: usize,
        data: &[u8],
    ) -> Result<(), WriteErr> {
        if data.len() == 0 {
            return Err(WriteErr::EmptyDataPassed);
        }
        assert_eq!(data.len() % self.block_size(), 0, "invalid data size");
        let num_blocks = data.len() / self.block_size();

        let mut bus = self.bus.as_ref().unwrap().borrow_mut();
        bus.select_drive(self.id);

        let last_block_idx = first_block_idx + num_blocks - 1;
        if !self.has_block(first_block_idx) {
            Err(WriteErr::NoSuchBlock)
        } else if !self.has_block(last_block_idx)
            || (num_blocks != 0 && num_blocks as u8 == 0)
        {
            Err(WriteErr::TooMuchBlocks)
        } else {
            let data = slice_u8_to_u16(data);
            bus.write(first_block_idx as u32, num_blocks as u8, data);
            Ok(())
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

// Primary bus.
const ATA0_PORT_IO_BASE: u16 = 0x1F0;
const ATA0_PORT_CONTROL_BASE: u16 = 0x3F6;

// Secondary bus.
const ATA1_PORT_IO_BASE: u16 = 0x170;
const ATA1_PORT_CONTROL_BASE: u16 = 0x376;

pub unsafe fn init() -> Vec<Drive> {
    // SAFETY: This function does not check if there are any actual ATA ports at
    // the standard places.  If they are not there, it means either that they
    // are somewhere else or that there is no IDE controller.

    // 1. Handle the IRQs.
    IDT.lock().interrupts[14].set_handler(irq14_handler);

    // IRQ 15 can also be a spurious IRQ sent from the slave PIC, so it has a
    // two-stage handler.  Set the second stage handler now.
    STAGE2_IRQ15_HANDLER = Some(ata_irq15_handler);

    IDT.lock().interrupts[15].set_handler(irq15_handler);

    PIC.set_irq_mask(14, false);
    PIC.set_irq_mask(15, false);

    // 2. Prepare shared pointers to the buses.
    let primary = Bus::new(ATA0_PORT_IO_BASE, ATA0_PORT_CONTROL_BASE);
    let secondary = Bus::new(ATA1_PORT_IO_BASE, ATA1_PORT_CONTROL_BASE);
    let rc_buses = [
        Rc::new(RefCell::new(primary)),
        Rc::new(RefCell::new(secondary)),
    ];

    // 3. Check for the drives.
    let mut all_drives = Vec::new();
    for (i, rc_bus) in rc_buses.iter().enumerate() {
        println!("[ATA] Initializing bus {}.", i);
        if rc_bus.borrow().registers.status.read::<u8>() == 0xFF {
            println!("[ATA] Ignoring a floating bus.");
            continue;
        }

        // 4. Connect each Drive to its Bus.  This is not done in Bus::init_etc.
        //    because I've found that somewhat difficult.
        let mut drives = rc_bus.borrow_mut().init_and_get_drives();
        if let Some(master) = &mut drives[0] {
            master.bus = Some(Rc::clone(&rc_bus));
            all_drives.push(master.clone())
        }
        if let Some(slave) = &mut drives[1] {
            slave.bus = Some(Rc::clone(&rc_bus));
            all_drives.push(slave.clone())
        }
    }
    all_drives
}

#[no_mangle]
pub extern "C" fn ata_irq14_handler(_: &InterruptStackFrame) {
    println!("[ATA] IRQ 14");
    unsafe {
        PIC.send_eoi(14);
    }
}

pub fn ata_irq15_handler(_: &InterruptStackFrame) {
    println!("[ATA] IRQ 15");
    unsafe {
        PIC.send_eoi(15);
    }
}
