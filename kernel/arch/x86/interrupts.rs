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

use core::mem::size_of;

use crate::arch::pic::PIC;
use crate::kernel_static::Mutex;

// See interrupts.s
extern "C" {
    // Dummy handlers for exceptions.
    fn dummy_isr_0(stack_frame: &InterruptStackFrame);
    fn dummy_isr_1(stack_frame: &InterruptStackFrame);
    fn dummy_isr_2(stack_frame: &InterruptStackFrame);
    fn dummy_isr_3(stack_frame: &InterruptStackFrame);
    fn dummy_isr_4(stack_frame: &InterruptStackFrame);
    fn dummy_isr_5(stack_frame: &InterruptStackFrame);
    fn dummy_isr_6(stack_frame: &InterruptStackFrame);
    fn dummy_isr_7(stack_frame: &InterruptStackFrame);
    fn dummy_isr_8(stack_frame: &InterruptStackFrame, err_code: u32);
    fn dummy_isr_9(stack_frame: &InterruptStackFrame);
    fn dummy_isr_10(stack_frame: &InterruptStackFrame, err_code: u32);
    fn dummy_isr_11(stack_frame: &InterruptStackFrame, err_code: u32);
    fn dummy_isr_12(stack_frame: &InterruptStackFrame, err_code: u32);
    fn dummy_isr_13(stack_frame: &InterruptStackFrame, err_code: u32);
    fn dummy_isr_14(stack_frame: &InterruptStackFrame, err_code: u32);
    fn dummy_isr_15(stack_frame: &InterruptStackFrame);
    fn dummy_isr_16(stack_frame: &InterruptStackFrame);
    fn dummy_isr_17(stack_frame: &InterruptStackFrame, err_code: u32);
    fn dummy_isr_18(stack_frame: &InterruptStackFrame);
    fn dummy_isr_19(stack_frame: &InterruptStackFrame);
    fn dummy_isr_20(stack_frame: &InterruptStackFrame);
    fn dummy_isr_21(stack_frame: &InterruptStackFrame, err_code: u32);
    fn dummy_isr_22(stack_frame: &InterruptStackFrame);
    fn dummy_isr_23(stack_frame: &InterruptStackFrame);
    fn dummy_isr_24(stack_frame: &InterruptStackFrame);
    fn dummy_isr_25(stack_frame: &InterruptStackFrame);
    fn dummy_isr_26(stack_frame: &InterruptStackFrame);
    fn dummy_isr_27(stack_frame: &InterruptStackFrame);
    fn dummy_isr_28(stack_frame: &InterruptStackFrame);
    fn dummy_isr_29(stack_frame: &InterruptStackFrame);
    fn dummy_isr_30(stack_frame: &InterruptStackFrame);
    fn dummy_isr_31(stack_frame: &InterruptStackFrame);

    // For all other interrupts.
    fn common_isr(stack_frame: &InterruptStackFrame);
    fn common_isr_ec(stack_frame: &InterruptStackFrame, err_code: u32);

    // Probably spurious IRQs.
    fn irq7_handler(stack_frame: &InterruptStackFrame);
    fn irq15_handler(stack_frame: &InterruptStackFrame);
}

#[allow(dead_code)]
#[repr(u8)]
enum Dpl {
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

macro_rules! impl_gate {
    ($t:ty, $d:ident) => {
        impl Gate<$t> {
            fn new(handler: $t, selector: u16, type_attr: TypeAttr) -> Self {
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
                let type_attr = TypeAttr::new(
                    true,
                    Dpl::Kernel,
                    GateType::InterruptGate32Bit,
                );
                Gate::<$t>::new($d, 0x08, type_attr)
            }

            pub fn set_handler(&mut self, handler: $t) {
                let offset = handler as u32;
                self.offset_1 = (offset & 0xFFFF) as u16;
                self.offset_2 = ((offset >> 16) & 0xFFFF) as u16;
            }
        }
    };
}

impl_gate!(HandlerFunc, common_isr);
impl_gate!(HandlerFuncWithErrCode, common_isr_ec);

type HandlerFunc = unsafe extern "C" fn(&InterruptStackFrame);
type HandlerFuncWithErrCode =
    unsafe extern "C" fn(&InterruptStackFrame, err_code: u32);

#[repr(C, packed)]
pub struct InterruptStackFrame {
    eip: u32,
    cs: u32,
    eflags: u32,

    // These values are present only when a privilege level switch happens.
    esp: u32,
    ss: u32,
}

#[repr(C)]
pub struct InterruptDescriptorTable {
    divide_error: Gate<HandlerFunc>,
    debug: Gate<HandlerFunc>,
    non_maskable_int: Gate<HandlerFunc>,
    breakpoint: Gate<HandlerFunc>,
    overflow: Gate<HandlerFunc>,
    bound_range_exceeded: Gate<HandlerFunc>,
    invalid_opcode: Gate<HandlerFunc>,
    device_not_available: Gate<HandlerFunc>,
    double_fault: Gate<HandlerFuncWithErrCode>,
    coprocessor_segment_overrun: Gate<HandlerFunc>, // reserved
    invalid_tss: Gate<HandlerFuncWithErrCode>,
    segment_not_present: Gate<HandlerFuncWithErrCode>,
    stack_fault: Gate<HandlerFuncWithErrCode>,
    general_protection: Gate<HandlerFuncWithErrCode>,
    page_fault: Gate<HandlerFuncWithErrCode>,
    reserved_1: Gate<HandlerFunc>, // reserved
    x87_fpu_floating_point_error: Gate<HandlerFunc>, // reserved
    alignment_check: Gate<HandlerFuncWithErrCode>,
    machine_check: Gate<HandlerFunc>,
    simd_floating_point: Gate<HandlerFunc>,
    virtualization: Gate<HandlerFunc>,
    control_protection: Gate<HandlerFuncWithErrCode>,
    reserved_2: [Gate<HandlerFunc>; 10], // reserved
    pub interrupts: [Gate<HandlerFunc>; 256 - 32],
}

impl InterruptDescriptorTable {
    fn new() -> Self {
        Self {
            divide_error: Gate::<HandlerFunc>::dummy(),
            debug: Gate::<HandlerFunc>::dummy(),
            non_maskable_int: Gate::<HandlerFunc>::dummy(),
            breakpoint: Gate::<HandlerFunc>::dummy(),
            overflow: Gate::<HandlerFunc>::dummy(),
            bound_range_exceeded: Gate::<HandlerFunc>::dummy(),
            invalid_opcode: Gate::<HandlerFunc>::dummy(),
            device_not_available: Gate::<HandlerFunc>::dummy(),
            double_fault: Gate::<HandlerFuncWithErrCode>::dummy(),
            coprocessor_segment_overrun: Gate::<HandlerFunc>::dummy(),
            invalid_tss: Gate::<HandlerFuncWithErrCode>::dummy(),
            segment_not_present: Gate::<HandlerFuncWithErrCode>::dummy(),
            stack_fault: Gate::<HandlerFuncWithErrCode>::dummy(),
            general_protection: Gate::<HandlerFuncWithErrCode>::dummy(),
            page_fault: Gate::<HandlerFuncWithErrCode>::dummy(),
            reserved_1: Gate::<HandlerFunc>::dummy(),
            x87_fpu_floating_point_error: Gate::<HandlerFunc>::dummy(),
            alignment_check: Gate::<HandlerFuncWithErrCode>::dummy(),
            machine_check: Gate::<HandlerFunc>::dummy(),
            simd_floating_point: Gate::<HandlerFunc>::dummy(),
            virtualization: Gate::<HandlerFunc>::dummy(),
            control_protection: Gate::<HandlerFuncWithErrCode>::dummy(),
            reserved_2: [Gate::<HandlerFunc>::dummy(); 10],
            interrupts: [Gate::<HandlerFunc>::dummy(); 256 - 32],
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
        idt.divide_error.set_handler(dummy_isr_0);
        idt.debug.set_handler(dummy_isr_1);
        idt.non_maskable_int.set_handler(dummy_isr_2);
        idt.breakpoint.set_handler(dummy_isr_3);
        idt.overflow.set_handler(dummy_isr_4);
        idt.bound_range_exceeded.set_handler(dummy_isr_5);
        idt.invalid_opcode.set_handler(dummy_isr_6);
        idt.device_not_available.set_handler(dummy_isr_7);
        idt.double_fault.set_handler(dummy_isr_8);
        idt.coprocessor_segment_overrun.set_handler(dummy_isr_9);
        idt.invalid_tss.set_handler(dummy_isr_10);
        idt.segment_not_present.set_handler(dummy_isr_11);
        idt.stack_fault.set_handler(dummy_isr_12);
        idt.general_protection.set_handler(dummy_isr_13);
        idt.page_fault.set_handler(dummy_isr_14);
        idt.reserved_1.set_handler(dummy_isr_15);
        idt.x87_fpu_floating_point_error.set_handler(dummy_isr_16);
        idt.alignment_check.set_handler(dummy_isr_17);
        idt.machine_check.set_handler(dummy_isr_18);
        idt.simd_floating_point.set_handler(dummy_isr_19);
        idt.virtualization.set_handler(dummy_isr_20);
        idt.control_protection.set_handler(dummy_isr_21);
        idt.reserved_2[0].set_handler(dummy_isr_22);
        idt.reserved_2[1].set_handler(dummy_isr_23);
        idt.reserved_2[2].set_handler(dummy_isr_24);
        idt.reserved_2[3].set_handler(dummy_isr_25);
        idt.reserved_2[4].set_handler(dummy_isr_26);
        idt.reserved_2[5].set_handler(dummy_isr_27);
        idt.reserved_2[6].set_handler(dummy_isr_28);
        idt.reserved_2[7].set_handler(dummy_isr_29);
        idt.reserved_2[8].set_handler(dummy_isr_30);
        idt.reserved_2[9].set_handler(dummy_isr_31);

        // Spurios interrupts (probably).  Those may happen because the kernel
        // sends an EOI to the PIT before iret so that it can switch tasks.
        idt.interrupts[7].set_handler(irq7_handler);
        idt.interrupts[15].set_handler(irq15_handler);

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

    if int_num == 14 {
        unsafe {
            let cr2: u32;
            asm!("movl %cr2, %eax", out("eax") cr2, options(att_syntax));
            println!(" cr2: 0x{:08X}", cr2);
        }
    }

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
    if PIC.get_isr() & (1 << 7) == 0 {
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
    if PIC.get_isr() & (1 << 15) == 0 {
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
