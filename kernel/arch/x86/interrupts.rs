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
struct Gate<T> {
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

            #[allow(dead_code)]
            fn set_handler(&mut self, handler: $t) {
                let offset = handler as u32;
                self.offset_1 = (offset & 0xFFFF) as u16;
                self.offset_2 = ((offset >> 16) & 0xFFFF) as u16;
            }
        }
    };
}

impl_gate!(HandlerFunc, dummy_handler);
impl_gate!(HandlerFuncWithErrCode, dummy_handler_with_err_code);

#[repr(C)]
struct InterruptStackFrame {
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
    _reserved_1: Gate<HandlerFunc>, // reserved
    x87_fpu_floating_point_error: Gate<HandlerFunc>, // reserved
    alignment_check: Gate<HandlerFuncWithErrCode>,
    machine_check: Gate<HandlerFunc>,
    simd_floating_point: Gate<HandlerFunc>,
    virtualization: Gate<HandlerFunc>,
    _reserved_2: [Gate<HandlerFunc>; 11], // reserved
    interrupts: [Gate<HandlerFunc>; 256 - 32],
}

type HandlerFunc = extern "x86-interrupt" fn(&InterruptStackFrame);
type HandlerFuncWithErrCode =
    extern "x86-interrupt" fn(&InterruptStackFrame, err_code: u32);

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
            _reserved_1: Gate::<HandlerFunc>::dummy(),
            x87_fpu_floating_point_error: Gate::<HandlerFunc>::dummy(),
            alignment_check: Gate::<HandlerFuncWithErrCode>::dummy(),
            machine_check: Gate::<HandlerFunc>::dummy(),
            simd_floating_point: Gate::<HandlerFunc>::dummy(),
            virtualization: Gate::<HandlerFunc>::dummy(),
            _reserved_2: [Gate::<HandlerFunc>::dummy(); 11],
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
    pub static ref IDT: InterruptDescriptorTable = {
        let mut idt = InterruptDescriptorTable::new();
        idt.divide_error.set_handler(divide_error_handler);
        idt.debug.set_handler(debug_handler);
        idt.non_maskable_int.set_handler(non_maskable_int_handler);
        idt.breakpoint.set_handler(breakpoint_handler);
        idt.overflow.set_handler(overflow_handler);
        idt.bound_range_exceeded.set_handler(bound_range_exceeded_handler);
        idt.invalid_opcode.set_handler(invalid_opcode_handler);
        idt.device_not_available.set_handler(device_not_available_handler);
        idt.double_fault.set_handler(double_fault_handler);
        idt.invalid_tss.set_handler(invalid_tss_handler);
        idt.segment_not_present.set_handler(segment_not_present_handler);
        idt.stack_fault.set_handler(stack_fault_handler);
        idt.general_protection.set_handler(general_protection_handler);
        idt.page_fault.set_handler(page_fault_handler);
        idt.alignment_check.set_handler(alignment_check_handler);
        idt.machine_check.set_handler(machine_check_handler);
        idt.simd_floating_point.set_handler(simd_floating_point_handler);
        idt.virtualization.set_handler(virtualization_handler);
        idt
    };
}

extern "x86-interrupt" fn divide_error_handler(
    _stack_frame: &InterruptStackFrame,
) {
    panic!("Divide by zero exception.");
}

extern "x86-interrupt" fn debug_handler(_stack_frame: &InterruptStackFrame) {
    panic!("Debug exception.");
}

extern "x86-interrupt" fn non_maskable_int_handler(
    _stack_frame: &InterruptStackFrame,
) {
    panic!("Non-maskable interrupt exception.");
}

extern "x86-interrupt" fn breakpoint_handler(
    _stack_frame: &InterruptStackFrame,
) {
    panic!("Breakpoint exception.");
}

extern "x86-interrupt" fn overflow_handler(_stack_frame: &InterruptStackFrame) {
    panic!("Overflow exception.");
}

extern "x86-interrupt" fn bound_range_exceeded_handler(
    _stack_frame: &InterruptStackFrame,
) {
    panic!("Bound range exceeded exception.");
}

extern "x86-interrupt" fn invalid_opcode_handler(
    _stack_frame: &InterruptStackFrame,
) {
    panic!("Invalid opcode exception.");
}

extern "x86-interrupt" fn device_not_available_handler(
    _stack_frame: &InterruptStackFrame,
) {
    panic!("Device not available exception.");
}

extern "x86-interrupt" fn double_fault_handler(
    _stack_frame: &InterruptStackFrame,
    _err_code: u32,
) {
    panic!("Double fault.");
}

extern "x86-interrupt" fn invalid_tss_handler(
    _stack_frame: &InterruptStackFrame,
    _err_code: u32,
) {
    panic!("Invalid TSS exception.");
}

extern "x86-interrupt" fn segment_not_present_handler(
    _stack_frame: &InterruptStackFrame,
    _err_code: u32,
) {
    panic!("Segment not present exception.");
}

extern "x86-interrupt" fn stack_fault_handler(
    _stack_frame: &InterruptStackFrame,
    _err_code: u32,
) {
    panic!("Stack fault.");
}

extern "x86-interrupt" fn general_protection_handler(
    _stack_frame: &InterruptStackFrame,
    _err_code: u32,
) {
    panic!("General protection exception.");
}

extern "x86-interrupt" fn page_fault_handler(
    _stack_frame: &InterruptStackFrame,
    _err_code: u32,
) {
    panic!("Page fault.");
}

extern "x86-interrupt" fn alignment_check_handler(
    _stack_frame: &InterruptStackFrame,
    _err_code: u32,
) {
    panic!("Alignment check exception.");
}

extern "x86-interrupt" fn machine_check_handler(
    _stack_frame: &InterruptStackFrame,
) {
    panic!("Machine check exception.");
}

extern "x86-interrupt" fn simd_floating_point_handler(
    _stack_frame: &InterruptStackFrame,
) {
    panic!("SIMD floating point exception.");
}

extern "x86-interrupt" fn virtualization_handler(
    _stack_frame: &InterruptStackFrame,
) {
    panic!("Virtualization exception.");
}

extern "x86-interrupt" fn dummy_handler(_stack_frame: &InterruptStackFrame) {
    panic!("Dummy exception handler called.");
}

extern "x86-interrupt" fn dummy_handler_with_err_code(
    _stack_frame: &InterruptStackFrame,
    _err_code: u32,
) {
    panic!("Dummy exception handler (with an error code) called.");
}

pub fn init() {
    use core::mem::size_of;
    let idt_descriptor = IdtDescriptor {
        size: (size_of::<InterruptDescriptorTable>() - 1) as u16,
        offset: &*IDT as *const _ as u32,
    };
    unsafe {
        asm!("lidt ({})", in(reg) &idt_descriptor, options(att_syntax));
        asm!("sti");
    }
}
