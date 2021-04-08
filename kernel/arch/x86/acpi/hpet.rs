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

use core::fmt;

use crate::arch::interrupts::{IDT, IRQ0_RUST_HANDLER};
use crate::arch::pic::PIC;
use crate::timer::TIMER;
use crate::KERNEL_INFO;

use super::AcpiAddr;
use crate::memory_region::Region;
use crate::timer::{Timer, TimerCallback};

extern "C" {
    fn irq0_handler(); // interrupts.s
}

const IRQ: u8 = 0;

#[derive(Clone, Copy, Debug)]
#[repr(C, packed)]
pub struct HpetDt {
    event_timer_block_id: u32,
    pub base_addr: AcpiAddr,
    pub hpet_num: u8,
    pub main_counter_min_tick: u16,
    page_protection_oem_attr: u8,
}

impl HpetDt {
    pub fn region_to_map(&self) -> Region<usize> {
        // Check some assumptions about the base address.
        assert_eq!(self.base_addr.addr_space_id, 0);
        assert!(
            self.base_addr.register_bit_width == 0
                || self.base_addr.register_bit_width == 64,
        );
        assert_eq!(self.base_addr.register_bit_offset, 0);
        assert_eq!(self.base_addr.address >> 32, 0);

        let start = self.base_addr.address as usize;
        let len = 0x117 + 0x20 * self.num_comparators();
        Region::from_start_len(start, (len + 4095) & !4095)
    }

    pub fn hardware_rev_id(&self) -> u8 {
        self.event_timer_block_id as u8
    }

    pub fn num_comparators(&self) -> usize {
        ((self.event_timer_block_id >> 8) & 0b11111) as usize
    }

    pub fn pci_vendor_id(&self) -> u16 {
        (self.event_timer_block_id >> 16) as u16
    }
}

pub struct Hpet {
    base_addr: u32,
    period_ms: u32,
    callback: Option<TimerCallback>,
}

impl Hpet {
    pub fn new(hpet_dt: &HpetDt, period_ms: u32) -> Self {
        println!("[HPET] {:#X?}", hpet_dt);
        println!("[HPET] Hardware rev ID: {}", hpet_dt.hardware_rev_id());
        println!(
            "[HPET] Number of comparators: {}",
            hpet_dt.num_comparators()
        );
        println!("[HPET] PCI vendor ID: 0x{:04X}", hpet_dt.pci_vendor_id());

        Hpet {
            base_addr: unsafe {
                KERNEL_INFO.arch.hpet_region.unwrap().start as u32
            },
            period_ms,
            callback: None,
        }
    }

    pub fn dump_registers(&self) {
        println!("{:#X?}", self.gen_caps_and_id_reg());
        println!("{:#X?}", self.gen_conf_reg());
        println!("{:#X?}", self.gen_int_status_reg());
        println!("Main Counter Value: 0x{:#016X?}", self.main_counter_value());
        for i in 0..self.gen_caps_and_id_reg().num_timers() + 1 {
            println!("Timer {} {:#X?}", i, self.timer_conf_and_cap_reg(i),);
            println!(
                "Timer {} Comparator Value: 0x{:016X}",
                i,
                self.timer_comparator_value(i),
            );
        }
    }

    pub fn gen_caps_and_id_reg(&self) -> GenCapsAndIdReg {
        let reg_addr = self.base_addr;
        let reg_ptr = reg_addr as *const GenCapsAndIdReg;
        unsafe { reg_ptr.read_volatile() }
    }

    pub fn gen_conf_reg(&self) -> GenConfReg {
        let reg_addr = self.base_addr + 0x10;
        let reg_ptr = reg_addr as *const GenConfReg;
        unsafe { reg_ptr.read_volatile() }
    }

    pub fn write_gen_conf_reg(&self, new_value: GenConfReg) {
        if new_value.uses_legacy_routing() {
            assert!(self.gen_caps_and_id_reg().capable_of_legacy_routing());
        }

        let reg_addr = self.base_addr + 0x10;
        let reg_ptr = reg_addr as *mut GenConfReg;
        unsafe {
            reg_ptr.write_volatile(new_value);
        }
    }

    pub fn gen_int_status_reg(&self) -> GenIntStatusReg {
        let reg_addr = self.base_addr + 0x20;
        let reg_ptr = reg_addr as *const GenIntStatusReg;
        unsafe { reg_ptr.read_volatile() }
    }

    pub fn main_counter_value(&self) -> u64 {
        let reg_addr = self.base_addr + 0xF0;
        let reg_ptr = reg_addr as *const u64;
        unsafe { reg_ptr.read_volatile() }
    }

    pub fn write_main_counter_value(&self, new_value: u64) {
        assert!(!self.gen_conf_reg().is_enabled());
        // FIXME: also check whether the timer is operating in 32-bit mode.

        let reg_addr = self.base_addr + 0xF0;
        let reg_ptr = reg_addr as *mut u64;
        unsafe { reg_ptr.write_volatile(new_value) }
    }

    pub fn timer_conf_and_cap_reg(&self, timer_n: usize) -> TimerConfAndCapReg {
        assert!(timer_n <= self.gen_caps_and_id_reg().num_timers());
        let reg_addr = self.base_addr + 0x100 + 0x20 * (timer_n as u32);
        let reg_ptr = reg_addr as *const TimerConfAndCapReg;
        unsafe { reg_ptr.read_volatile() }
    }

    pub fn write_timer_conf_and_cap_reg(
        &self,
        timer_n: usize,
        new_value: TimerConfAndCapReg,
    ) {
        // FIXME: no checks needed?
        assert!(timer_n <= self.gen_caps_and_id_reg().num_timers());
        let reg_addr = self.base_addr + 0x100 + 0x20 * (timer_n as u32);
        let reg_ptr = reg_addr as *mut TimerConfAndCapReg;
        unsafe { reg_ptr.write_volatile(new_value) }
    }

    pub fn timer_comparator_value(&self, timer_n: usize) -> u64 {
        assert!(timer_n <= self.gen_caps_and_id_reg().num_timers());
        let reg_addr = self.base_addr + 0x108 + 0x20 * (timer_n as u32);
        let reg_ptr = reg_addr as *const u64;
        unsafe { reg_ptr.read_volatile() }
    }

    pub fn write_timer_comparator_value(&self, timer_n: usize, new_value: u64) {
        assert!(timer_n <= self.gen_caps_and_id_reg().num_timers());
        let reg_addr = self.base_addr + 0x108 + 0x20 * (timer_n as u32);
        let reg_ptr = reg_addr as *mut u64;
        unsafe { reg_ptr.write_volatile(new_value) }
    }
}

#[repr(transparent)]
pub struct GenCapsAndIdReg(u64);

impl GenCapsAndIdReg {
    pub fn rev_id(&self) -> u8 {
        self.0 as u8
    }

    pub fn num_timers(&self) -> usize {
        ((self.0 >> 8) & 0b11111) as usize
    }

    pub fn main_counter_64bit(&self) -> bool {
        ((self.0 >> 13) & 1) != 0
    }

    pub fn capable_of_legacy_routing(&self) -> bool {
        ((self.0 >> 15) & 1) != 0
    }

    pub fn vendor_id(&self) -> u16 {
        (self.0 >> 16) as u16
    }

    pub fn main_counter_tick_period(&self) -> u32 {
        (self.0 >> 32) as u32
    }
}

impl fmt::Debug for GenCapsAndIdReg {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("GenCapsAndIdReg")
            .field("rev_id", &self.rev_id())
            .field("num_timers", &self.num_timers())
            .field("main_counter_64bit", &self.main_counter_64bit())
            .field(
                "capable_of_legacy_routing",
                &self.capable_of_legacy_routing(),
            )
            .field("vendor_id", &self.vendor_id())
            .field("main_counter_tick_period", &self.main_counter_tick_period())
            .finish()
    }
}

#[repr(transparent)]
pub struct GenConfReg(u64);

impl GenConfReg {
    pub fn is_enabled(&self) -> bool {
        (self.0 & 1) != 0
    }

    pub fn uses_legacy_routing(&self) -> bool {
        ((self.0 >> 1) & 1) != 0
    }

    pub fn set_enabled(&mut self, new_value: bool) {
        if new_value {
            self.0 |= 1;
        } else {
            self.0 &= !1;
        }
    }

    pub fn set_legacy_routing(&mut self, new_value: bool) {
        if new_value {
            self.0 |= 1 << 1;
        } else {
            self.0 &= !(1 << 1);
        }
    }
}

impl fmt::Debug for GenConfReg {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("GenConfReg")
            .field("is_enabled", &self.is_enabled())
            .field("uses_legacy_routing", &self.uses_legacy_routing())
            .finish()
    }
}

#[repr(transparent)]
pub struct GenIntStatusReg(u64);

impl GenIntStatusReg {
    pub fn timer_int(&self, n: usize) -> bool {
        (self.0 >> (n as u64)) & 1 != 0
    }
}

impl fmt::Debug for GenIntStatusReg {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut timer_int: u32 = 0;
        for i in 0..32 {
            timer_int |= (self.timer_int(i) as u32) << i;
        }
        f.debug_struct("GenIntStatusReg")
            .field("timer_int", &timer_int)
            .finish()
    }
}

#[repr(transparent)]
pub struct TimerConfAndCapReg(u64);

impl TimerConfAndCapReg {
    pub fn int_type(&self) -> TimerIntType {
        match (self.0 >> 1) & 1 {
            0 => TimerIntType::EdgeTriggered,
            1 => TimerIntType::LevelTriggered,
            _ => unreachable!(),
        }
    }

    pub fn set_int_type(&mut self, new_type: TimerIntType) {
        match new_type {
            TimerIntType::EdgeTriggered => {
                self.0 &= !(1 << 1);
            }
            TimerIntType::LevelTriggered => {
                self.0 |= 1 << 1;
            }
        };
    }

    pub fn has_int_enabled(&self) -> bool {
        (self.0 & (1 << 2)) != 0
    }

    pub fn set_int_enabled(&mut self, new_value: bool) {
        if new_value {
            self.0 |= 1 << 2;
        } else {
            self.0 &= !(1 << 2);
        }
    }

    pub fn _type(&self) -> TimerType {
        match (self.0 >> 3) & 1 {
            0 => TimerType::NonPeriodic,
            1 => TimerType::Periodic,
            _ => unreachable!(),
        }
    }

    pub fn set_type(&mut self, new_type: TimerType) {
        match new_type {
            TimerType::NonPeriodic => {
                self.0 &= !(1 << 3);
            }
            TimerType::Periodic => {
                assert!(self.may_be_periodic());
                self.0 |= 1 << 3;
            }
        }
    }

    pub fn may_be_periodic(&self) -> bool {
        ((self.0 >> 4) & 1) != 0
    }

    pub fn is_64bit(&self) -> bool {
        ((self.0 >> 5) & 1) != 0
    }

    // pub fn FIXME: Tn_VAL_SET_CNF
    pub fn allows_setting_acc_value(&self) -> bool {
        ((self.0 >> 6) & 1) != 0
    }

    pub fn allow_setting_acc_value(&mut self, allow: bool) {
        if allow {
            assert_eq!(self._type(), TimerType::Periodic);
            self.0 |= 1 << 6;
        } else {
            self.0 &= !(1 << 6);
        }
    }

    pub fn is_in_32bit_mode(&self) -> bool {
        ((self.0 >> 8) & 1) != 0
    }

    pub fn set_32bit_mode(&mut self, new_value: bool) {
        assert!(self.is_64bit());
        if new_value {
            self.0 |= 1 << 8;
        } else {
            self.0 &= !(1 << 8);
        }
    }

    pub fn ioapic_routing(&self) -> u8 {
        ((self.0 >> 9) & 0b11111) as u8
    }

    pub fn set_ioapic_routing(&mut self, new_route: u8) {
        assert_eq!(new_route >> 5, 0, "new_route must be less than 32");
        assert!(self.supports_ioapic_routing(new_route));
        self.0 &= !(0b11111 << 9);
        self.0 |= ((new_route as u64) << 9) & 0b11111;
    }

    pub fn uses_fsb_int_delivery(&self) -> bool {
        ((self.0 >> 14) & 1) != 0
    }

    pub fn use_fsb_int_delivery(&mut self, use_fsb: bool) {
        if use_fsb {
            assert!(self.capable_of_fsb_int_delivery());
            self.0 |= 1 << 14;
        } else {
            self.0 &= !(1 << 14);
        }
    }

    pub fn capable_of_fsb_int_delivery(&self) -> bool {
        ((self.0 >> 15) & 1) != 0
    }

    pub fn supports_ioapic_routing(&self, irq: u8) -> bool {
        assert_eq!(irq >> 5, 0, "irq must be less than 32");
        let bits = (self.0 >> 32) as u32;
        ((bits & (1 << irq)) & 1) != 0
    }
}

impl fmt::Debug for TimerConfAndCapReg {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut supports_ioapic_routing: u32 = 0;
        for i in 0..32 {
            supports_ioapic_routing |=
                (self.supports_ioapic_routing(i) as u32) << i;
        }
        f.debug_struct("TimerConfAndCapReg")
            .field("int_type", &self.int_type())
            .field("has_int_enabled", &self.has_int_enabled())
            .field("_type", &self._type())
            .field("may_be_periodic", &self.may_be_periodic())
            .field("is_64bit", &self.is_64bit())
            .field("allows_setting_acc_value", &self.allows_setting_acc_value())
            .field("is_in_32bit_mode", &self.is_in_32bit_mode())
            .field("ioapic_routing", &self.ioapic_routing())
            .field("uses_fsb_int_delivery", &self.uses_fsb_int_delivery())
            .field(
                "capable_of_fsb_int_delivery",
                &self.capable_of_fsb_int_delivery(),
            )
            .field("supports_ioapic_routing", &supports_ioapic_routing)
            .finish()
    }
}

#[derive(Debug)]
pub enum TimerIntType {
    EdgeTriggered,
    LevelTriggered,
}

#[derive(PartialEq, Debug)]
pub enum TimerType {
    NonPeriodic,
    Periodic,
}

pub static mut HPET: Option<Hpet> = None;

impl Timer for Hpet {
    fn init_with_period_ms(period_ms: usize) -> Self {
        let hpet_dt = unsafe { KERNEL_INFO.arch.hpet_dt.unwrap() };
        let hpet = Hpet::new(&hpet_dt, period_ms as u32);

        let mut gen_conf = hpet.gen_conf_reg();
        gen_conf.set_enabled(true);
        gen_conf.set_legacy_routing(true);
        hpet.write_gen_conf_reg(gen_conf);

        // Make timer 0 run in periodic mode with interrupts on IRQ0.
        let mut t0_conf = hpet.timer_conf_and_cap_reg(0);
        t0_conf.set_int_enabled(true);
        t0_conf.set_type(TimerType::Periodic);
        t0_conf.allow_setting_acc_value(true);
        t0_conf.set_32bit_mode(true);
        hpet.write_timer_conf_and_cap_reg(0, t0_conf);

        // Calculate the period in ticks.
        let tick_fs =
            hpet.gen_caps_and_id_reg().main_counter_tick_period() as u64;
        let period_fs = period_ms as u64 * 1_000_000_000_000; // 1e12
        let period_ticks = period_fs / tick_fs;
        assert_ne!(period_ticks, 0);
        assert!(period_ticks >= hpet_dt.main_counter_min_tick as u64);

        let main_counter = hpet.main_counter_value();
        hpet.write_timer_comparator_value(0, main_counter + period_ticks);
        hpet.write_timer_comparator_value(0, period_ticks);

        println!("[HPET] Registers dump:");
        hpet.dump_registers();
        println!("[HPET] End of registers dump.");

        IDT.lock().interrupts[IRQ as usize].set_handler(irq0_handler);
        unsafe {
            IRQ0_RUST_HANDLER = hpet_irq_handler;
            PIC.set_irq_mask(IRQ, false);
        }

        hpet
    }

    fn period_ms(&self) -> usize {
        self.period_ms as usize
    }

    fn set_callback(&mut self, callback: TimerCallback) {
        self.callback = Some(callback);
    }

    fn callback(&self) -> Option<TimerCallback> {
        self.callback
    }
}

#[no_mangle]
pub extern "C" fn hpet_irq_handler() {
    unsafe {
        PIC.send_eoi(0);

        if let Some(timer) = TIMER.as_ref() {
            if let Some(callback) = timer.callback() {
                callback();
            }
        }
    }
}
