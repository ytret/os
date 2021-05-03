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

use crate::arch::interrupts::{IDT, IRQ0_RUST_HANDLER};
use crate::arch::pic::PIC;
use crate::dev::timer::TIMER;

use crate::arch::port_io;
use crate::dev::timer::{Timer, TimerCallback};

extern "C" {
    fn irq0_handler(); // interrupts.s
}

#[allow(dead_code)]
#[repr(u16)]
enum Port {
    Channel0Data = 0x40,
    Channel1Data = 0x41,
    Channel2Data = 0x42,
    ModeCommandRegister = 0x43, // write-only, a read is ignored
}

#[allow(dead_code)]
#[repr(u8)]
enum Channel {
    // Only channel 0 is used and the code implies that.
    Ch0 = 0b00 << 6,
    Ch1 = 0b01 << 6,
    Ch2 = 0b10 << 6,
    ReadBackCommand = 0b11 << 6, // on non-obsolete hardware
}

#[allow(dead_code)]
#[derive(Clone, Copy)]
#[repr(u8)]
enum AccessMode {
    // (0b00 << 4) - latch count value command
    LowByteOnly = 0b01,
    HighByteOnly = 0b10,
    BothBytes = 0b11,
}

#[allow(dead_code)]
#[derive(Clone, Copy)]
#[repr(u8)]
enum OperatingMode {
    InterruptOnTerminalCount = 0b000,
    HardwareRetriggableOneShot = 0b001,
    RateGenerator = 0b010,
    SquareWaveGenerator = 0b011,
    SoftwareTriggeredStrobe = 0b100,
    HardwareTriggeredStrobe = 0b101,
    // 0b110 <=> 0b010,
    // 0b111 <=> 0b011,
}

pub const IRQ: u8 = 0;
const BASE_FREQUENCY: f64 = 1.193182e+6; // Hz

pub struct Pit {
    reload_value: u16,
    operating_mode: OperatingMode,
    access_mode: AccessMode,

    callback: Option<TimerCallback>,
}

impl Pit {
    pub fn init(&self) {
        self.send_register();
        self.send_reload_value();
    }

    /// Sets the period in seconds.
    ///
    /// The biggest possible value is 54 ms due to hardware limitations.
    /// Trying to set the period to 55 ms or greater will make
    /// [`Pit::set_frequency()`] panic.
    pub fn set_period(&mut self, period: f64) {
        self.set_frequency(1.0 / period);
    }

    pub fn set_frequency(&mut self, freq: f64) {
        let reload_value = (BASE_FREQUENCY / freq) as u32;
        assert!(reload_value <= 65535, "frequency is too low");
        self.reload_value = reload_value as u16;
    }

    pub fn period(&self) -> f64 {
        1.0 / self.frequency()
    }

    pub fn frequency(&self) -> f64 {
        BASE_FREQUENCY / self.reload_value as f64
    }

    fn send_register(&self) {
        let mut value: u8 = 0;
        value |= 0 << 0; // binary mode (not BCD)
        value |= (self.operating_mode as u8) << 1;
        value |= (self.access_mode as u8) << 4;
        value |= (Channel::Ch0 as u8) << 6;

        unsafe {
            println!("[PIT] Register: 0x{:X}", value);
            port_io::outb(Port::ModeCommandRegister as u16, value);
        }
    }

    fn send_reload_value(&self) {
        match self.access_mode {
            AccessMode::LowByteOnly => {
                assert_eq!(
                    self.reload_value >> 8,
                    0,
                    "non-zero high byte of reload value is ignored"
                );
                let data = (self.reload_value >> 0) as u8;
                unsafe {
                    port_io::outb(Port::Channel0Data as u16, data);
                }
            }
            AccessMode::HighByteOnly => {
                assert_eq!(
                    self.reload_value & 0xFF,
                    0,
                    "non-zero low byte of reload value is ignored"
                );
                let data = (self.reload_value >> 8) as u8;
                unsafe {
                    port_io::outb(Port::Channel0Data as u16, data);
                }
            }
            AccessMode::BothBytes => {
                let low = (self.reload_value >> 0) as u8;
                let high = (self.reload_value >> 8) as u8;
                unsafe {
                    port_io::outb(Port::Channel0Data as u16, low);
                    port_io::outb(Port::Channel0Data as u16, high);
                }
            }
        }
    }
}

impl Timer for Pit {
    fn init_with_period_ms(period_ms: usize) -> Self {
        let mut pit = Pit {
            reload_value: 0,
            operating_mode: OperatingMode::SquareWaveGenerator,
            access_mode: AccessMode::BothBytes,

            callback: None,
        };

        pit.set_period(period_ms as f64 * 1e-3);
        println!(
            "[PIT] Reload value: {}, frequency: {:.1} Hz, period: {:.2e} s",
            pit.reload_value,
            pit.frequency(),
            pit.period(),
        );
        pit.init();

        IDT.lock().interrupts[IRQ as usize].set_handler(irq0_handler);
        unsafe {
            IRQ0_RUST_HANDLER = pit_irq_handler;
            PIC.set_irq_mask(IRQ, false);
        }

        pit
    }

    fn period_ms(&self) -> usize {
        let res = (self.period() * 1e9) as usize;
        assert_ne!(res, 0);
        res
    }

    fn set_callback(&mut self, callback: TimerCallback) {
        self.callback = Some(callback);
    }

    fn callback(&self) -> Option<TimerCallback> {
        self.callback
    }
}

#[no_mangle]
pub extern "C" fn pit_irq_handler() {
    unsafe {
        PIC.send_eoi(IRQ);

        if let Some(timer) = TIMER.as_ref() {
            if let Some(callback) = timer.callback() {
                callback();
            }
        }
    }
}
