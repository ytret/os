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

use crate::arch::port_io;
use crate::bitflags::BitFlags;

#[allow(dead_code)]
#[repr(u16)]
enum Port {
    MasterCommand = 0x20,
    MasterData = 0x21,
    SlaveCommand = 0xA0,
    SlaveData = 0xA1,
}

bitflags! {
    #[repr(u8)]
    enum InitCommandWord1 {
        Icw4Needed = 1 << 0,          // not set: no ICW4 needed
        SingleMode = 1 << 1,          // not set: cascade mode
        CallAddrIntervalOf4 = 1 << 2, // not set: call address interval of 8
        LevelTriggeredMode = 1 << 3,  // not set: edge triggered mode
        Icw1 = 1 << 4,                // must be set
    }
}

bitflags! {
    #[repr(u8)]
    enum InitCommandWord4 {
        Mode8086 = 1 << 0,               // not set: MCS-80/85 mode
        AutoEoi = 1 << 1,                // not set: normal EOI
        BufferedModeMaster = 0b11 << 2,  // not set: non-buffered mode
        BufferedModeSlave = 0b10 << 2,   // not set: non-buffered mode
        SpecialFullyNestedMode = 1 << 4, // not set: not special f. n. mode
    }
}

const EOI: u8 = 1 << 5;

pub struct Pic {
    master_vector_offset: u8,
    slave_vector_offset: u8,
    master_has_slave_at_ir: u8,
    slave_id: u8,
}

impl Pic {
    fn init(&self) {
        // ICW1: Start the init sequence.
        let icw1: BitFlags<u8, InitCommandWord1> = BitFlags::new(0)
            | InitCommandWord1::Icw1
            | InitCommandWord1::Icw4Needed;
        self.send_command(icw1.value);

        // ICW2: Tell PICs their vector offsets.
        self.send_master_data(self.master_vector_offset);
        self.send_slave_data(self.slave_vector_offset);

        // ICW3: Set up cascade mode.
        self.send_master_data(self.master_has_slave_at_ir);
        self.send_slave_data(self.slave_id);

        // ICW4: Set 8086/8088 mode.
        let icw4: BitFlags<u8, InitCommandWord4> =
            BitFlags::new(0) | InitCommandWord4::Mode8086;
        self.send_data(icw4.value);

        // Mask IRQs.
        self.mask_irqs();
    }

    fn mask_irqs(&self) {
        for i in 0..16 {
            self.set_irq_mask(i, true);
        }
    }

    pub fn set_irq_mask(&self, mut irq_num: u8, mask: bool) {
        let port;
        if irq_num < 8 {
            port = Port::MasterData as u16;
        } else {
            port = Port::SlaveData as u16;
            irq_num -= 8;
        }
        unsafe {
            let mut value = port_io::inb(port);
            if mask {
                value |= 1 << irq_num;
            } else {
                value &= !(1 << irq_num);
            }
            port_io::outb(port, value);
        }
    }

    pub fn send_eoi(&self, irq_num: u8) {
        if irq_num >= 8 {
            self.send_slave_command(EOI);
        }
        self.send_master_command(EOI);
    }

    fn send_command(&self, cmd: u8) {
        self.send_master_command(cmd);
        self.send_slave_command(cmd);
    }

    fn send_data(&self, data: u8) {
        self.send_master_data(data);
        self.send_slave_data(data);
    }

    fn send_master_command(&self, cmd: u8) {
        unsafe {
            port_io::outb(Port::MasterCommand as u16, cmd);
        }
    }

    fn send_slave_command(&self, cmd: u8) {
        unsafe {
            port_io::outb(Port::SlaveCommand as u16, cmd);
        }
    }

    fn send_master_data(&self, data: u8) {
        unsafe {
            port_io::outb(Port::MasterData as u16, data);
        }
    }
    fn send_slave_data(&self, data: u8) {
        unsafe {
            port_io::outb(Port::SlaveData as u16, data);
        }
    }
}

kernel_static! {
    pub static ref PIC: Pic = Pic {
        master_vector_offset: 32,
        slave_vector_offset: 40,
        master_has_slave_at_ir: 0b0000_0100,
        slave_id: 2,
    };
}

pub fn init() {
    PIC.init();
}
