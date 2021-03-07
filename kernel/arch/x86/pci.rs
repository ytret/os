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
use alloc::rc::Rc;
use alloc::vec::Vec;
use core::marker::PhantomData;

use crate::arch::port_io;
use crate::disk;

#[derive(Clone)]
struct Pci {
    host_buses: Vec<(usize, Bus)>,
}

impl Pci {
    const fn new() -> Self {
        Pci {
            host_buses: Vec::new(),
        }
    }

    fn enumerate(&mut self) {
        let info_device = Device::new(0, 0);
        let multiple_host_buses = info_device.is_multifunctional();
        if !multiple_host_buses {
            let host_bus = Bus::new(0);
            self.host_buses.push((0, host_bus));
        } else {
            for bus_num in 0..8 {
                if let Some(_) = info_device.functions[bus_num].conf_space {
                    let host_bus = Bus::new(bus_num as u8);
                    self.host_buses.push((bus_num, host_bus));
                }
            }
        }
    }

    fn all_devices(&self) -> Vec<Device> {
        let mut devices = Vec::new();
        for (_, host_bus) in self.host_buses.iter() {
            for device in host_bus.all_devices() {
                devices.push(device)
            }
        }
        devices
    }
}

#[derive(Clone)]
struct Bus {
    bus_num: u8,
    devices: Vec<Device>,
    secondary_buses: Vec<(u8, Bus)>,
}

impl Bus {
    fn new(bus_num: u8) -> Self {
        let mut devices = Vec::new();
        let mut secondary_buses = Vec::new();
        for device_num in 0..32 {
            let device = Device::new(bus_num, device_num);
            if let Some(conf_space) = device.functions[0].conf_space {
                match conf_space {
                    ConfSpace::Device(_) => devices.push(device),
                    ConfSpace::PciToPciBridge(conf_space) => {
                        let secondary_bus_num = conf_space
                            .secondary_bus_num
                            .read(&device.functions[0]);
                        secondary_buses
                            .push((device_num, Bus::new(secondary_bus_num)));
                    }
                }
            }
        }
        Bus {
            bus_num,
            devices,
            secondary_buses,
        }
    }

    fn all_devices(&self) -> Vec<Device> {
        let mut devices = self.devices.clone();
        for (_, secondary_bus) in self.secondary_buses.iter() {
            for device in secondary_bus.devices.iter() {
                devices.push(device.clone());
            }
        }
        devices
    }
}

#[repr(transparent)]
struct ConfAddressBuilder(u32);

impl ConfAddressBuilder {
    fn new() -> Self {
        ConfAddressBuilder(0)
    }

    fn enable_bit(&mut self, value: bool) -> &mut Self {
        if value {
            self.0 |= 1 << 31;
        } else {
            self.0 &= !(1 << 31);
        }
        self
    }

    fn bus_num(&mut self, bus_num: u8) -> &mut Self {
        self.0 &= !(0xFF << 16);
        self.0 |= (bus_num as u32) << 16;
        self
    }

    fn device_num(&mut self, device_num: u8) -> &mut Self {
        assert_eq!(device_num & !(0b11111), 0, "invalid device number");
        self.0 &= !(0b11111 << 11);
        self.0 |= (device_num as u32) << 11;
        self
    }

    fn function_num(&mut self, function_num: u8) -> &mut Self {
        assert_eq!(function_num & !(0b111), 0, "invalid function number");
        self.0 &= !(0b111 << 8);
        self.0 |= (function_num as u32) << 8;
        self
    }

    fn register_offset(&mut self, offset: u8) -> &mut Self {
        assert_eq!(offset & 0b11, 0, "invalid register offset");
        self.0 &= !0xFF;
        self.0 |= offset as u32;
        self
    }

    fn done(&self) -> u32 {
        self.0
    }
}

#[derive(Clone)]
struct Device {
    bus_num: u8,
    device_num: u8,
    functions: Vec<Function>,
}

impl Device {
    fn new(bus_num: u8, device_num: u8) -> Self {
        let mut device = Device {
            bus_num,
            device_num,
            functions: Vec::new(),
        };

        for function_num in 0..8 {
            let function = Function::new(bus_num, device_num, function_num);
            device.functions.push(function);
            if !device.is_multifunctional() {
                break;
            }
        }
        device
    }

    fn is_multifunctional(&self) -> bool {
        let is_mf = (self.functions[0].header_type() & (1 << 7)) != 0;
        is_mf
    }
}

#[derive(Clone)]
struct Function {
    bus_num: u8,
    device_num: u8,
    function_num: u8,
    class: DeviceClass,
    conf_space: Option<ConfSpace>,
}

impl Function {
    fn new(bus_num: u8, device_num: u8, function_num: u8) -> Self {
        let mut function = Function {
            bus_num,
            device_num,
            function_num,
            class: DeviceClass::Unknown,
            conf_space: None,
        };

        let register = |offset| function.register(offset); // for short

        // If the vendor ID is not valid, the function does not exist and we
        // don't bother with its configuration.
        let vendor_id = register(0x00) as u16;
        if vendor_id == 0xFFFF {
            function.conf_space = None;
            return function;
        }

        // Read the configuration space.
        let header_type = function.header_type() & !(1 << 7);
        function.conf_space = match header_type {
            0x00 => {
                let conf_space = DeviceConfSpace::new();
                Some(ConfSpace::Device(conf_space))
            }
            0x01 => {
                let conf_space = PciToPciBridgeConfSpace::new();
                Some(ConfSpace::PciToPciBridge(conf_space))
            }
            other => {
                println!("PCI: ignoring header type 0x{:02X}", other);
                None
            }
        };

        // Try to recognize the device function.
        if let Some(ConfSpace::Device(conf_space)) = function.conf_space {
            let class_code = conf_space.class_code.read(&function);
            let subclass = conf_space.subclass.read(&function);
            let prog_if = conf_space.prog_if.read(&function);
            function.class = match class_code {
                0x00 => match subclass {
                    0x00 => DeviceClass::Unclassified(UnclassifiedSubclass::NonVgaCompatible),
                    0x01 => DeviceClass::Unclassified(UnclassifiedSubclass::VgaCompatible),
                    _ => DeviceClass::Unclassified(UnclassifiedSubclass::Unknown),
                }

                0x01 => match subclass {
                    0x01 => match prog_if {
                        0x80 => DeviceClass::MassStorageController(MassStorageControllerSubclass::IdeController(IdeControllerInterface::IsaCompatibilityModeOnlyWithBusMastering)),
                        _ => DeviceClass::MassStorageController(MassStorageControllerSubclass::IdeController(IdeControllerInterface::Unknown)),
                    }
                    0x06 => match prog_if {
                        0x01 => DeviceClass::MassStorageController(MassStorageControllerSubclass::SerialAta(SerialAtaInterface::Ahci1_0)),
                        _ => DeviceClass::MassStorageController(MassStorageControllerSubclass::SerialAta(SerialAtaInterface::Unknown)),
                    }
                    // _ => DeviceClass::MassStorageController(MassStorageControllerSubclass::Other),
                    _ => DeviceClass::MassStorageController(MassStorageControllerSubclass::Unknown),
                }

                0x02 => match subclass {
                    0x00 => DeviceClass::NetworkController(NetworkControllerSubclass::EthernetController),
                    0x80 => DeviceClass::NetworkController(NetworkControllerSubclass::Other),
                    _ => DeviceClass::NetworkController(NetworkControllerSubclass::Unknown),
                }

                0x03 => match subclass {
                    0x00 => match prog_if {
                        0x00 => DeviceClass::DisplayController(DisplayControllerSubclass::VgaCompatible(VgaCompatibleInterface::VgaController)),
                        _ => DeviceClass::DisplayController(DisplayControllerSubclass::VgaCompatible(VgaCompatibleInterface::Unknown)),
                    },
                    0x80 => DeviceClass::DisplayController(DisplayControllerSubclass::Other),
                    _ => DeviceClass::DisplayController(DisplayControllerSubclass::Unknown),
                }

                0x06 => match subclass {
                    0x00 => DeviceClass::BridgeDevice(BridgeDeviceSubclass::HostBridge),
                    0x01 => DeviceClass::BridgeDevice(BridgeDeviceSubclass::IsaBridge),
                    0x80 => DeviceClass::BridgeDevice(BridgeDeviceSubclass::Other),
                    _ => DeviceClass::BridgeDevice(BridgeDeviceSubclass::Unknown),
                }

                _ => DeviceClass::Unknown,
            };
        }

        function
    }

    fn header_type(&self) -> u8 {
        (self.register(0x0C) >> 16) as u8
    }

    fn register(&self, offset: u8) -> u32 {
        let addr = ConfAddressBuilder::new()
            .enable_bit(true)
            .bus_num(self.bus_num)
            .device_num(self.device_num)
            .function_num(self.function_num)
            .register_offset(offset)
            .done();
        unsafe {
            port_io::outl(PORT_CONFIG_ADDRESS, addr);
        }
        let value = unsafe { port_io::inl(PORT_CONFIG_DATA) };
        value
    }

    fn set_register(&self, offset: u8, value: u32) {
        let addr = ConfAddressBuilder::new()
            .enable_bit(true)
            .bus_num(self.bus_num)
            .device_num(self.device_num)
            .function_num(self.function_num)
            .register_offset(offset)
            .done();
        unsafe {
            port_io::outl(PORT_CONFIG_ADDRESS, addr);
            let before = port_io::inl(PORT_CONFIG_DATA);
            port_io::outl(PORT_CONFIG_DATA, value);
            let after = port_io::inl(PORT_CONFIG_DATA);
            println!(
                "before: 0x{:08X}, value: 0x{:08X}, after: 0x{:08X}",
                before, value, after,
            );
            if after == before {
                println!("set_register: could not change");
            }
        }
    }

    fn exists(&self) -> bool {
        if let Some(conf_space) = self.conf_space {
            conf_space.has_valid_vendor_id(self)
        } else {
            false
        }
    }
}

#[derive(Clone, Debug)]
enum DeviceClass {
    Unknown,
    Unclassified(UnclassifiedSubclass),
    MassStorageController(MassStorageControllerSubclass),
    NetworkController(NetworkControllerSubclass),
    DisplayController(DisplayControllerSubclass),
    BridgeDevice(BridgeDeviceSubclass),
}

#[derive(Clone, Debug)]
enum UnclassifiedSubclass {
    Unknown,
    NonVgaCompatible,
    VgaCompatible,
}

#[derive(Clone, Debug)]
enum MassStorageControllerSubclass {
    Unknown,
    IdeController(IdeControllerInterface),
    SerialAta(SerialAtaInterface),
    // Other,
}

#[derive(Clone, Debug)]
enum IdeControllerInterface {
    Unknown,
    IsaCompatibilityModeOnlyWithBusMastering,
}

#[derive(Clone, Debug)]
enum SerialAtaInterface {
    Unknown,
    Ahci1_0,
}

#[derive(Clone, Debug)]
enum NetworkControllerSubclass {
    Unknown,
    EthernetController,
    Other,
}

#[derive(Clone, Debug)]
enum DisplayControllerSubclass {
    Unknown,
    VgaCompatible(VgaCompatibleInterface),
    Other,
}

#[derive(Clone, Debug)]
enum VgaCompatibleInterface {
    Unknown,
    VgaController,
}

#[derive(Clone, Debug)]
enum BridgeDeviceSubclass {
    Unknown,
    HostBridge,
    IsaBridge,
    Other,
}

#[derive(Clone, Copy, Debug)]
enum ConfSpace {
    Device(DeviceConfSpace),
    PciToPciBridge(PciToPciBridgeConfSpace),
}

impl ConfSpace {
    fn has_valid_vendor_id(&self, of_function: &Function) -> bool {
        match self {
            ConfSpace::Device(conf_space) => {
                conf_space.vendor_id.read(of_function) != 0xFFFF
            }
            ConfSpace::PciToPciBridge(conf_space) => {
                conf_space.vendor_id.read(of_function) != 0xFFFF
            }
        }
    }
}

trait RegisterType: Sized + Into<u32> {
    fn mask_u32(value: u32) -> Self;

    fn mask() -> u32 {
        let num_bits = 8 * core::mem::size_of::<Self>() as u32;
        2u32.pow(num_bits) - 1
    }
}

impl RegisterType for u8 {
    fn mask_u32(value: u32) -> Self {
        value as u8
    }
}

impl RegisterType for u16 {
    fn mask_u32(value: u32) -> Self {
        value as u16
    }
}

impl RegisterType for u32 {
    fn mask_u32(value: u32) -> Self {
        value
    }
}

#[derive(Clone, Copy, Debug)]
struct Register<T: RegisterType> {
    offset: u8,
    shift_left: u8,
    read_only: bool,
    reserved: bool,
    phantom_data: PhantomData<T>,
}

impl<T: RegisterType> Register<T> {
    fn read_only(offset: u8, shift_left: u8) -> Self {
        Register {
            offset,
            shift_left,
            read_only: true,
            reserved: false,
            phantom_data: PhantomData,
        }
    }

    fn read_write(offset: u8, shift_left: u8) -> Self {
        Register {
            offset,
            shift_left,
            read_only: false,
            reserved: false,
            phantom_data: PhantomData,
        }
    }

    fn reserved(offset: u8, shift_left: u8) -> Self {
        Register {
            offset,
            shift_left,
            read_only: true,
            reserved: true,
            phantom_data: PhantomData,
        }
    }

    fn read(&self, of_function: &Function) -> T {
        if self.reserved {
            panic!("It is not allowed to read a reserved field.");
        } else {
            let addr = ConfAddressBuilder::new()
                .enable_bit(true)
                .bus_num(of_function.bus_num)
                .device_num(of_function.device_num)
                .function_num(of_function.function_num)
                .register_offset(self.offset)
                .done();
            unsafe {
                port_io::outl(PORT_CONFIG_ADDRESS, addr);
            }
            let mut value = unsafe { port_io::inl(PORT_CONFIG_DATA) };
            value = value >> self.shift_left as u32;
            T::mask_u32(value)
        }
    }

    fn write(&self, of_function: &Function, value: T) {
        if self.reserved {
            panic!("It is not allowed to read a reserved field.");
        } else if self.read_only {
            panic!("Cannot write to a read-only register.");
        } else {
            let addr = ConfAddressBuilder::new()
                .enable_bit(true)
                .bus_num(of_function.bus_num)
                .device_num(of_function.device_num)
                .function_num(of_function.function_num)
                .register_offset(self.offset)
                .done();
            unsafe {
                port_io::outl(PORT_CONFIG_ADDRESS, addr);

                let before = port_io::inl(PORT_CONFIG_DATA);
                let mut new_value = before;
                new_value &= !(T::mask() << self.shift_left);
                new_value |= value.into() << self.shift_left as u32;
                if new_value == before {
                    return;
                }
                port_io::outl(PORT_CONFIG_DATA, new_value);

                let after = port_io::inl(PORT_CONFIG_DATA);
                assert_ne!(
                    after, before,
                    "wrote to a register, but it did not change",
                );
            }
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct DeviceConfSpace {
    vendor_id: Register<u16>,
    device_id: Register<u16>,
    command: Register<u16>,
    status: Register<u16>,
    revision_id: Register<u8>,
    prog_if: Register<u8>,
    subclass: Register<u8>,
    class_code: Register<u8>,
    cache_line_size: Register<u8>,
    latency_timer: Register<u8>,
    header_type: Register<u8>,
    bist: Register<u8>,
    bar0: Register<u32>,
    bar1: Register<u32>,
    bar2: Register<u32>,
    bar3: Register<u32>,
    bar4: Register<u32>,
    bar5: Register<u32>,
    cardbus_cis_ptr: Register<u32>,
    subsystem_vendor_id: Register<u16>,
    subsystem_id: Register<u16>,
    expansion_rom_base_addr: Register<u32>,
    capabilites_ptr: Register<u8>,
    interrupt_line: Register<u8>,
    interrupt_pin: Register<u8>,
    min_grant: Register<u8>,
    max_latency: Register<u8>,
}

impl DeviceConfSpace {
    fn new() -> Self {
        DeviceConfSpace {
            vendor_id: Register::read_only(0x00, 0),
            device_id: Register::read_only(0x00, 16),
            command: Register::read_write(0x04, 0),
            status: Register::read_only(0x04, 16),
            revision_id: Register::read_only(0x08, 0),
            prog_if: Register::read_only(0x08, 8),
            subclass: Register::read_only(0x08, 16),
            class_code: Register::read_only(0x08, 24),
            cache_line_size: Register::read_write(0x0C, 0),
            latency_timer: Register::read_only(0x0C, 8),
            header_type: Register::read_only(0x0C, 16),
            bist: Register::read_only(0x0C, 24),
            bar0: Register::read_write(0x10, 0),
            bar1: Register::read_write(0x14, 0),
            bar2: Register::read_write(0x18, 0),
            bar3: Register::read_write(0x1C, 0),
            bar4: Register::read_write(0x20, 0),
            bar5: Register::read_write(0x24, 0),
            cardbus_cis_ptr: Register::read_only(0x28, 0),
            subsystem_vendor_id: Register::read_only(0x2C, 0),
            subsystem_id: Register::read_only(0x2C, 16),
            expansion_rom_base_addr: Register::read_only(0x30, 0),
            capabilites_ptr: Register::read_only(0x34, 0),
            interrupt_line: Register::read_write(0x3C, 0),
            interrupt_pin: Register::read_write(0x3C, 8),
            min_grant: Register::read_only(0x3C, 16),
            max_latency: Register::read_only(0x3C, 24),
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct PciToPciBridgeConfSpace {
    vendor_id: Register<u16>,
    device_id: Register<u16>,
    command: Register<u16>,
    status: Register<u16>,
    revision_id: Register<u8>,
    prog_if: Register<u8>,
    subclass: Register<u8>,
    class_code: Register<u8>,
    cache_line_size: Register<u8>,
    latency_timer: Register<u8>,
    header_type: Register<u8>,
    bist: Register<u8>,
    bar0: Register<u32>,
    bar1: Register<u32>,
    primary_bus_num: Register<u8>,
    secondary_bus_num: Register<u8>,
    subordinate_bus_num: Register<u8>,
    secondary_latency_timer: Register<u8>,
    io_base: Register<u8>,
    io_limit: Register<u8>,
    secondary_status: Register<u16>,
    memory_base: Register<u16>,
    memory_limit: Register<u16>,
    prefetchable_memory_base: Register<u16>,
    prefetchable_memory_limit: Register<u16>,
    prefetchable_base_upper_32_bits: Register<u32>,
    prefetchable_limit_upper_32_bits: Register<u32>,
    io_limit_upper_16_bits: Register<u16>,
    io_base_upper_16_bits: Register<u16>,
    capability_ptr: Register<u8>,
    expansion_rom_base_addr: Register<u32>,
    interrupt_line: Register<u8>,
    interrupt_pin: Register<u8>,
    bridge_control: Register<u16>,
}

impl PciToPciBridgeConfSpace {
    fn new() -> Self {
        PciToPciBridgeConfSpace {
            vendor_id: Register::read_only(0x00, 0),
            device_id: Register::read_only(0x00, 16),
            command: Register::read_only(0x04, 0),
            status: Register::read_only(0x04, 16),
            revision_id: Register::read_only(0x08, 0),
            prog_if: Register::read_only(0x08, 8),
            subclass: Register::read_only(0x08, 16),
            class_code: Register::read_only(0x08, 24),
            cache_line_size: Register::read_write(0x0C, 0),
            latency_timer: Register::read_only(0x0C, 8),
            header_type: Register::read_only(0x0C, 16),
            bist: Register::read_only(0x0C, 24),
            bar0: Register::read_write(0x10, 0),
            bar1: Register::read_write(0x14, 0),
            primary_bus_num: Register::read_only(0x18, 0),
            secondary_bus_num: Register::read_only(0x18, 8),
            subordinate_bus_num: Register::read_only(0x18, 16),
            secondary_latency_timer: Register::read_only(0x18, 24),
            io_base: Register::read_only(0x1C, 0),
            io_limit: Register::read_only(0x1C, 8),
            secondary_status: Register::read_only(0x1C, 16),
            memory_base: Register::read_only(0x20, 0),
            memory_limit: Register::read_only(0x20, 16),
            prefetchable_memory_base: Register::read_only(0x24, 0),
            prefetchable_memory_limit: Register::read_only(0x24, 16),
            prefetchable_base_upper_32_bits: Register::read_only(0x28, 0),
            prefetchable_limit_upper_32_bits: Register::read_only(0x2C, 0),
            io_limit_upper_16_bits: Register::read_only(0x30, 0),
            io_base_upper_16_bits: Register::read_only(0x30, 16),
            capability_ptr: Register::read_only(0x34, 0),
            expansion_rom_base_addr: Register::read_only(0x38, 0),
            interrupt_line: Register::read_write(0x3C, 0),
            interrupt_pin: Register::read_write(0x3C, 8),
            bridge_control: Register::read_only(0x3C, 16),
        }
    }
}

const PORT_CONFIG_ADDRESS: u16 = 0xCF8;
const PORT_CONFIG_DATA: u16 = 0xCFC;

static mut PCI: Pci = Pci::new();

pub static mut TEST_VFS: Option<crate::fs::Node> = None;

pub fn init() {
    unsafe {
        PCI.enumerate();
    }

    for (host_bus_num, host_bus) in unsafe { &PCI }.host_buses.iter() {
        print!("Host bus 0x{:02X} : ", host_bus_num);
        print_bus(16, host_bus);
    }

    // Initialize devices.
    for device in unsafe { &PCI }.all_devices() {
        for function in device.functions.iter().filter(|x| x.exists()) {
            match &function.class {
                DeviceClass::MassStorageController(MassStorageControllerSubclass::IdeController(IdeControllerInterface::IsaCompatibilityModeOnlyWithBusMastering)) => {
                    println!("[PCI] Initializing an IDE controller.");
                    unsafe {
                        let drives = disk::ata::init();
                        for drive in drives {
                            let mut disk = disk::Disk {
                                id: disk::DISKS.lock().len(),
                                rw_interface: Rc::new(Box::new(drive)),
                                file_system: None,
                            };
                            println!("[PCI] Probing a file system on the detected disk.");
                            let maybe_root_node = disk.try_init_fs();
                            // println!("[PCI] Result: {:?}", maybe_root_node);
                            TEST_VFS = Some(maybe_root_node.unwrap());
                            disk::DISKS.lock().push(Rc::new(disk));
                        }
                    }
                }
                _ => {}
            }
        }
    }

    println!("[PCI] Init end.");
}

fn print_bus(offset: usize, bus: &Bus) {
    let print_offset = || {
        for _ in 0..offset {
            print!(" ");
        }
    };
    for (i, (sec_bus_num, sec_bus)) in bus.secondary_buses.iter().enumerate() {
        if i != 0 {
            print_offset();
        }
        println!("Secondary bus 0x{:02X} : ", sec_bus_num);
        print_bus(offset + 21, sec_bus);
    }
    for (i, device) in bus.devices.iter().enumerate() {
        if i != 0 {
            print_offset();
        }
        print!("Device 0x{:02X} : ", device.device_num);
        for (j, function_num) in (0..device.functions.len()).enumerate() {
            if let Some(conf_space) = device.functions[function_num].conf_space
            {
                if j != 0 {
                    for _ in 0..offset + 14 {
                        print!(" ");
                    }
                }
                match conf_space {
                    ConfSpace::Device(cs) => {
                        println!(
                            "Function {} {:04X}:{:04X} \
                             Class {:02X}:{:02X}:{:02X} {:?}",
                            function_num,
                            cs.vendor_id.read(&device.functions[function_num]),
                            cs.device_id.read(&device.functions[function_num]),
                            cs.class_code.read(&device.functions[function_num]),
                            cs.subclass.read(&device.functions[function_num]),
                            cs.prog_if.read(&device.functions[function_num]),
                            device.functions[function_num].class,
                        );
                    }
                    ConfSpace::PciToPciBridge(_) => {
                        // FIXME: can a function be a PCI-to-PCI bridge?
                        println!("PCI to PCI bridge not in a right place");
                    }
                    _ => unreachable!(),
                }
            } else {
                /*
                println!(
                    "Ignoring {} {:04X}:{:04X} Header Type 0x{:02X}",
                    function_num,
                    device.register(function_num as u8, 0x00) as u16,
                    (device.register(function_num as u8, 0x00) >> 16) as u16,
                    device.header_type(function_num as u8),
                );
                */
            }
        }
    }
}
