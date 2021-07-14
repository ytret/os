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

use crate::arch::port_io;

#[allow(dead_code)]
#[repr(u16)]
enum Port {
    MasterCommand = 0x20,
    MasterData = 0x21,
    SlaveCommand = 0xA0,
    SlaveData = 0xA1,
}

bitflags_new! {
    struct InitCommandWord1: u8 {
        const ICW4_NEEDED = 1 << 0;             // not set: no ICW4 needed
        const SINGLE_MODE = 1 << 1;             // not set: cascade mode
        const CALL_ADDR_INTERVAL_OF_4 = 1 << 2; // not set: call address interval of 8
        const LEVEL_TRIGGERED_MODE = 1 << 3;    // not set: edge triggered mode
        const ICW1 = 1 << 4;                    // must be set
    }
}

bitflags_new! {
    struct InitCommandWord4: u8 {
        const MODE_8086 = 1 << 0;                 // not set: MCS-80/85 mode
        const AUTO_EOI = 1 << 1;                  // not set: normal EOI
        const BUFFERED_MODE_MASTER = 0b11 << 2;   // not set: non-buffered mode
        const BUFFERED_MODE_SLAVE = 0b10 << 2;    // not set: non-buffered mode
        const SPECIAL_FULLY_NESTED_MODE = 1 << 4; // not set: not special f. n. mode
    }
}

bitflags_new! {
    struct OpControlWord3: u8 {
        // Bit 7 must be zero.
        const RESET_SPECIAL_MASK = 0b10 << 5; // not set: no action
        const SET_SPECIAL_MASK = 0b11 << 5;   // not set: no action
        // Bit 4 must be zero.
        const MUST_BE_SET = 1 << 3;
        const POLL_COMMAND = 1 << 2;          // not set: no poll command
        const READ_IRR = 0b10;                // not set: no action
        const READ_ISR = 0b11;                // not set: no action
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
        let icw1 = InitCommandWord1::ICW1 | InitCommandWord1::ICW4_NEEDED;
        self.send_master_command(icw1.bits());
        self.send_slave_command(icw1.bits());

        // ICW2: Tell PICs their vector offsets.
        self.send_master_data(self.master_vector_offset);
        self.send_slave_data(self.slave_vector_offset);

        // ICW3: Set up cascade mode.
        self.send_master_data(self.master_has_slave_at_ir);
        self.send_slave_data(self.slave_id);

        // ICW4: Set 8086/8088 mode.
        let icw4 = InitCommandWord4::MODE_8086;
        self.send_master_data(icw4.bits());
        self.send_slave_data(icw4.bits());

        // Mask IRQs.
        self.mask_irqs();
        self.set_irq_mask(self.slave_id, false);
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

    pub fn get_isr(&self) -> u16 {
        let ocw3 = OpControlWord3::MUST_BE_SET | OpControlWord3::READ_ISR;
        self.send_master_command(ocw3.bits());
        self.send_slave_command(ocw3.bits());
        unsafe {
            let master_isr = port_io::inb(Port::MasterCommand as u16) as u16;
            let slave_isr = port_io::inb(Port::SlaveCommand as u16) as u16;
            (slave_isr << 8) | master_isr
        }
    }
}

pub static mut PIC: Pic = Pic {
    master_vector_offset: 32,
    slave_vector_offset: 40,
    master_has_slave_at_ir: 0b0000_0100,
    slave_id: 2,
};

pub fn init() {
    unsafe {
        PIC.init();
    }
}
