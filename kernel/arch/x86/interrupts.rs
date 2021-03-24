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

use crate::arch::pic::PIC;
use crate::kernel_static::Mutex;

// See interrupts.s
extern "C" {
    fn isr_0();
    fn isr_1();
    fn isr_2();
    fn isr_3();
    fn isr_4();
    fn isr_5();
    fn isr_6();
    fn isr_7();
    fn isr_8();
    fn isr_9();
    fn isr_10();
    fn isr_11();
    fn isr_12();
    fn isr_13();
    fn isr_14();
    fn isr_15();
    fn isr_16();
    fn isr_17();
    fn isr_18();
    fn isr_19();
    fn isr_20();
    fn isr_21();
    fn isr_22();
    fn isr_23();
    fn isr_24();
    fn isr_25();
    fn isr_26();
    fn isr_27();
    fn isr_28();
    fn isr_29();
    fn isr_30();
    fn isr_31();

    // For all other interrupts.
    fn common_isr();
    fn common_isr_ec();

    // Probably spurious IRQs.
    fn irq7_handler();
    fn irq15_handler();

    // Syscall.
    fn int0x88_handler();
}

#[allow(dead_code)]
#[repr(u8)]
pub enum Dpl {
    Kernel = 0,
    Userspace = 3,
}

#[allow(dead_code)]
#[repr(u8)]
enum GateType {
    TaskGate32Bit = 0b00101,
    InterruptGate32Bit = 0b01110,
    InterruptGate16Bit = 0b00110,
    TrapGate32Bit = 0b01111,
    TrapGate16Bit = 0b00111,
}

#[derive(Clone, Copy)]
#[repr(transparent)]
struct TypeAttr(u8);

impl TypeAttr {
    fn new(present: bool, dpl: Dpl, gate_type: GateType) -> Self {
        let mut type_attr = TypeAttr(0);
        type_attr.0 |= (present as u8) << 7;
        type_attr.0 |= (dpl as u8) << 5;
        type_attr.0 |= gate_type as u8;
        type_attr
    }

    pub fn set_dpl(&mut self, new_dpl: Dpl) {
        self.0 &= !(0b11 << 5);
        self.0 |= (new_dpl as u8) << 5;
    }
}

impl core::ops::BitOr<u8> for TypeAttr {
    type Output = Self;

    fn bitor(self, rhs: u8) -> Self {
        Self(self.0 | rhs)
    }
}

#[derive(Clone, Copy)]
#[repr(C)]
pub struct Gate<T> {
    offset_1: u16,
    selector: u16,
    zero: u8,
    type_attr: TypeAttr,
    offset_2: u16,
    phantom: core::marker::PhantomData<T>,
}

impl Gate<Isr> {
    fn new(handler: Isr, selector: u16, type_attr: TypeAttr) -> Self {
        let offset = handler as u32;
        Gate {
            offset_1: (offset & 0xFFFF) as u16,
            selector,
            zero: 0,
            type_attr,
            offset_2: ((offset >> 16) & 0xFFFF) as u16,
            phantom: core::marker::PhantomData,
        }
    }

    fn dummy() -> Self {
        let type_attr =
            TypeAttr::new(true, Dpl::Kernel, GateType::InterruptGate32Bit);
        Self::new(common_isr, 0x08, type_attr)
    }

    fn dummy_ec() -> Self {
        let type_attr =
            TypeAttr::new(true, Dpl::Kernel, GateType::InterruptGate32Bit);
        Self::new(common_isr_ec, 0x08, type_attr)
    }

    pub fn set_handler(&mut self, handler: Isr) {
        let offset = handler as u32;
        self.offset_1 = (offset & 0xFFFF) as u16;
        self.offset_2 = ((offset >> 16) & 0xFFFF) as u16;
    }

    pub fn set_dpl(&mut self, new_dpl: Dpl) {
        self.type_attr.set_dpl(new_dpl);
    }
}

type Isr = unsafe extern "C" fn();

#[repr(C, packed)]
pub struct InterruptStackFrame {
    pub eip: u32,
    pub cs: u32,
    pub eflags: u32,

    // These values are present only when a privilege level switch happens.
    pub esp: u32,
    pub ss: u32,
}

#[repr(C)]
pub struct InterruptDescriptorTable {
    divide_error: Gate<Isr>,
    debug: Gate<Isr>,
    non_maskable_int: Gate<Isr>,
    breakpoint: Gate<Isr>,
    overflow: Gate<Isr>,
    bound_range_exceeded: Gate<Isr>,
    invalid_opcode: Gate<Isr>,
    device_not_available: Gate<Isr>,
    double_fault: Gate<Isr>,
    coprocessor_segment_overrun: Gate<Isr>, // reserved
    invalid_tss: Gate<Isr>,
    segment_not_present: Gate<Isr>,
    stack_fault: Gate<Isr>,
    general_protection: Gate<Isr>,
    page_fault: Gate<Isr>,
    reserved_1: Gate<Isr>,                   // reserved
    x87_fpu_floating_point_error: Gate<Isr>, // reserved
    alignment_check: Gate<Isr>,
    machine_check: Gate<Isr>,
    simd_floating_point: Gate<Isr>,
    virtualization: Gate<Isr>,
    control_protection: Gate<Isr>,
    reserved_2: [Gate<Isr>; 10], // reserved
    pub interrupts: [Gate<Isr>; 256 - 32],
}

impl InterruptDescriptorTable {
    fn new() -> Self {
        Self {
            divide_error: Gate::dummy(),
            debug: Gate::dummy(),
            non_maskable_int: Gate::dummy(),
            breakpoint: Gate::dummy(),
            overflow: Gate::dummy(),
            bound_range_exceeded: Gate::dummy(),
            invalid_opcode: Gate::dummy(),
            device_not_available: Gate::dummy(),
            double_fault: Gate::dummy_ec(),
            coprocessor_segment_overrun: Gate::dummy(),
            invalid_tss: Gate::dummy_ec(),
            segment_not_present: Gate::dummy_ec(),
            stack_fault: Gate::dummy_ec(),
            general_protection: Gate::dummy_ec(),
            page_fault: Gate::dummy_ec(),
            reserved_1: Gate::dummy(),
            x87_fpu_floating_point_error: Gate::dummy(),
            alignment_check: Gate::dummy_ec(),
            machine_check: Gate::dummy(),
            simd_floating_point: Gate::dummy(),
            virtualization: Gate::dummy(),
            control_protection: Gate::dummy_ec(),
            reserved_2: [Gate::dummy(); 10],
            interrupts: [Gate::dummy(); 256 - 32],
        }
    }
}

#[repr(C, packed)]
struct IdtDescriptor {
    size: u16,
    offset: u32,
}

kernel_static! {
    pub static ref IDT: Mutex<InterruptDescriptorTable> = Mutex::new({
        let mut idt = InterruptDescriptorTable::new();
        idt.divide_error.set_handler(isr_0);
        idt.debug.set_handler(isr_1);
        idt.non_maskable_int.set_handler(isr_2);
        idt.breakpoint.set_handler(isr_3);
        idt.overflow.set_handler(isr_4);
        idt.bound_range_exceeded.set_handler(isr_5);
        idt.invalid_opcode.set_handler(isr_6);
        idt.device_not_available.set_handler(isr_7);
        idt.double_fault.set_handler(isr_8);
        idt.coprocessor_segment_overrun.set_handler(isr_9);
        idt.invalid_tss.set_handler(isr_10);
        idt.segment_not_present.set_handler(isr_11);
        idt.stack_fault.set_handler(isr_12);
        idt.general_protection.set_handler(isr_13);
        idt.page_fault.set_handler(isr_14);
        idt.reserved_1.set_handler(isr_15);
        idt.x87_fpu_floating_point_error.set_handler(isr_16);
        idt.alignment_check.set_handler(isr_17);
        idt.machine_check.set_handler(isr_18);
        idt.simd_floating_point.set_handler(isr_19);
        idt.virtualization.set_handler(isr_20);
        idt.control_protection.set_handler(isr_21);
        idt.reserved_2[0].set_handler(isr_22);
        idt.reserved_2[1].set_handler(isr_23);
        idt.reserved_2[2].set_handler(isr_24);
        idt.reserved_2[3].set_handler(isr_25);
        idt.reserved_2[4].set_handler(isr_26);
        idt.reserved_2[5].set_handler(isr_27);
        idt.reserved_2[6].set_handler(isr_28);
        idt.reserved_2[7].set_handler(isr_29);
        idt.reserved_2[8].set_handler(isr_30);
        idt.reserved_2[9].set_handler(isr_31);

        // Spurios interrupts (probably).  Those may happen because the kernel
        // sends an EOI to the PIT before iret so that it can switch tasks.
        idt.interrupts[7].set_handler(irq7_handler);
        idt.interrupts[15].set_handler(irq15_handler);

        // Syscall.
        idt.interrupts[0x88 - 32].set_handler(int0x88_handler);
        idt.interrupts[0x88 - 32].set_dpl(Dpl::Userspace);

        idt
    });
}

#[no_mangle]
pub extern "C" fn dummy_exception_handler(
    int_num: u32,
    err_code: u32,
    stack_frame: &InterruptStackFrame,
) {
    println!("Dummy exception handler called.");
    println!(" exception number: {}", int_num);
    println!(
        " error code: {:08b}_{:08b}_{:08b}_{:08b} (0x{:08X})",
        (err_code >> 24) & 0xF,
        (err_code >> 16) & 0xF,
        (err_code >> 08) & 0xF,
        (err_code >> 00) & 0xF,
        err_code
    );

    let eip = stack_frame.eip;
    println!(" eip: 0x{:08X}", eip);

    panic!("Unhandled exception.");
}

pub fn init() {
    let idt_descriptor = IdtDescriptor {
        size: (size_of::<InterruptDescriptorTable>() - 1) as u16,
        offset: &*IDT as *const _ as u32,
    };
    unsafe {
        asm!("lidt ({})", in(reg) &idt_descriptor, options(att_syntax));
        asm!("sti");
    }
}

#[no_mangle]
pub extern "C" fn common_interrupt_handler(stack_frame: &InterruptStackFrame) {
    println!("Common interrupt handler called.");
    let eip = stack_frame.eip;
    println!(" eip: 0x{:08X}", eip);
    panic!("Unhandled interrupt.");
}

pub static mut STAGE2_IRQ7_HANDLER: Option<fn(&InterruptStackFrame)> = None;
pub static mut STAGE2_IRQ15_HANDLER: Option<fn(&InterruptStackFrame)> = None;

#[no_mangle]
pub extern "C" fn stage1_irq7_handler(stack_frame: &InterruptStackFrame) {
    if unsafe { PIC.get_isr() } & (1 << 7) == 0 {
        println!("Ignoring IRQ 7: a spurious interrupt.");
        let eip = stack_frame.eip;
        println!(" eip: 0x{:08X}", eip);
    } else if let Some(handler) = unsafe { STAGE2_IRQ7_HANDLER } {
        println!(
            "IRQ 7 has the stage 2 handler at 0x{:08X}, calling it.",
            handler as *const () as usize,
        );
        handler(stack_frame);
    } else {
        println!("IRQ 7: the stage 2 handler is not set.");
        let eip = stack_frame.eip;
        println!(" eip: 0x{:08X}", eip);
        panic!("Unhandled interrupt.");
    }
}

#[no_mangle]
pub extern "C" fn stage1_irq15_handler(stack_frame: &InterruptStackFrame) {
    if unsafe { PIC.get_isr() } & (1 << 15) == 0 {
        println!("Ignoring IRQ 15: a spurious interrupt.");
        let eip = stack_frame.eip;
        println!(" eip: 0x{:08X}", eip);
    } else if let Some(handler) = unsafe { STAGE2_IRQ15_HANDLER } {
        println!(
            "IRQ 15 has the stage 2 handler at 0x{:08X}, calling it.",
            handler as *const () as usize,
        );
        handler(stack_frame);
    } else {
        println!("IRQ 15: the stage 2 handler is not set.");
        let eip = stack_frame.eip;
        println!(" eip: 0x{:08X}", eip);
        panic!("Unhandled interrupt.");
    }
}
