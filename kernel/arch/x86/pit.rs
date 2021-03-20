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

use core::sync::atomic::{AtomicU32, Ordering};

use crate::arch::interrupts::{InterruptStackFrame, IDT};
use crate::arch::pic::PIC;
use crate::arch::port_io;

use crate::scheduler::SCHEDULER;

extern "C" {
    fn irq0_handler(stack_frame: &InterruptStackFrame); // interrupts.s
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

struct Pit {
    reload_value: u16,
    operating_mode: OperatingMode,
    access_mode: AccessMode,
}

impl Pit {
    fn init(&self) {
        self.send_register();
        self.send_reload_value();
    }

    fn set_period(&mut self, period: f64) {
        self.set_frequency(1.0 / period);
    }

    fn set_frequency(&mut self, freq: f64) {
        let mut reload_value = (BASE_FREQUENCY / freq) as u32;
        if reload_value > 65535 {
            println!(
                "[PIT] Reload value = {} > 65535, setting to 65535.",
                reload_value,
            );
            reload_value = 65535;
        }
        self.reload_value = reload_value as u16;
    }

    fn period(&self) -> f64 {
        1.0 / self.frequency()
    }

    fn frequency(&self) -> f64 {
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

static mut PIT: Pit = Pit {
    reload_value: 0,
    operating_mode: OperatingMode::SquareWaveGenerator,
    access_mode: AccessMode::BothBytes,
};

pub fn init() {
    unsafe {
        PIT.set_period(2.0);
        println!(
            "[PIT] Reload value: {}, frequency: {:.1} Hz, period: {:.2e} s",
            PIT.reload_value,
            PIT.frequency(),
            PIT.period(),
        );
        PIT.init();
    }

    IDT.lock().interrupts[0].set_handler(irq0_handler);
    // PIC.set_irq_mask(IRQ, false);
}

static COUNTER_MS: AtomicU32 = AtomicU32::new(0);

use core::sync::atomic::{AtomicBool, AtomicUsize};
pub static TEMP_SPAWNER_ON: AtomicBool = AtomicBool::new(false);
static NUM_SPAWNED: AtomicUsize = AtomicUsize::new(0);

#[no_mangle]
pub extern "C" fn pit_irq0_handler() {
    let period_ms = unsafe { (PIT.period() * 1.0e+3) as u32 };
    assert_ne!(
        period_ms,
        0,
        "PIT frequency is too high: {:.1} Hz",
        unsafe { PIT.frequency() },
    );
    COUNTER_MS.fetch_add(period_ms, Ordering::SeqCst);

    if TEMP_SPAWNER_ON.load(Ordering::SeqCst)
        && NUM_SPAWNED.load(Ordering::SeqCst) < 2
    {
        println!("[PIT] Creating a new process.");
        use crate::arch::process::Process;
        let new_process = Process::new();
        unsafe {
            SCHEDULER.add_process(new_process);
        }
        NUM_SPAWNED.fetch_add(1, Ordering::SeqCst);
    }

    // Send an EOI before scheduling so that the IRQ will interrupt the next
    // task.  One might just do an iret as a context switch but why bother if
    // this handler will be executed further (including the iret) when it's time
    // for this task.
    PIC.send_eoi(0);

    if COUNTER_MS.load(Ordering::SeqCst) >= 1000 {
        COUNTER_MS.store(0, Ordering::SeqCst);
        // println!("SCHEDULING (period_ms = {})", period_ms);
        unsafe {
            SCHEDULER.schedule(period_ms);
        }
    }
}
