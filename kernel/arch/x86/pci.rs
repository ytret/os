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

use alloc::vec::Vec;

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
                let conf_space_option = info_device.functions[bus_num];
                if let Some(_) = conf_space_option {
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
            if let Some(conf_space) = device.functions[0] {
                match conf_space {
                    ConfSpace::Device(_) => devices.push(device),
                    ConfSpace::PciToPciBridge(conf_space) => {
                        let secondary_bus_num = conf_space.secondary_bus_num;
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
    class: DeviceClass,
    configured: bool,
    functions: Vec<Option<ConfSpace>>,
}

impl Device {
    fn new(bus_num: u8, device_num: u8) -> Self {
        let mut device = Device {
            bus_num,
            device_num,
            class: DeviceClass::Unknown,
            configured: false,
            functions: Vec::with_capacity(8),
        };
        device.functions.push(device.conf_space(0));
        if device.is_multifunctional() {
            for function_num in 1..8 {
                device.functions.push(device.conf_space(function_num));
            }
        }
        device
    }

    fn is_multifunctional(&self) -> bool {
        let is_mf = (self.header_type(0) & (1 << 7)) != 0;
        is_mf
    }

    fn conf_space(&self, function_num: u8) -> Option<ConfSpace> {
        let register = |offset| self.register(function_num, offset);
        let vendor_id = register(0x00) as u16;
        if vendor_id == 0xFFFF {
            return None;
        }
        let header_type = self.header_type(function_num) & !(1 << 7);
        match header_type {
            0x00 => {
                let conf_space = DeviceConfSpace {
                    vendor_id,
                    device_id: (register(0x00) >> 16) as u16,
                    command: register(0x04) as u16,
                    status: (register(0x04) >> 16) as u16,
                    revision_id: register(0x08) as u8,
                    prog_if: (register(0x08) >> 8) as u8,
                    subclass: (register(0x08) >> 16) as u8,
                    class_code: (register(0x08) >> 24) as u8,
                    cache_line_size: register(0x0C) as u8,
                    latency_timer: (register(0x0C) >> 8) as u8,
                    header_type: (register(0x0C) >> 16) as u8,
                    bist: (register(0x0C) >> 24) as u8,
                    bar0: register(0x10),
                    bar1: register(0x14),
                    bar2: register(0x18),
                    bar3: register(0x1C),
                    bar4: register(0x20),
                    bar5: register(0x24),
                    cardbus_cis_ptr: register(0x28),
                    subsystem_vendor_id: register(0x2C) as u16,
                    subsystem_id: (register(0x2C) >> 16) as u16,
                    expansion_rom_base_addr: register(0x30),
                    capabilites_ptr: register(0x34) as u8,
                    interrupt_line: register(0x3C) as u8,
                    interrupt_pin: (register(0x3C) >> 8) as u8,
                    min_grant: (register(0x3C) >> 16) as u8,
                    max_latency: (register(0x3C) >> 24) as u8,
                };
                Some(ConfSpace::Device(conf_space))
            }
            0x01 => {
                let conf_space = PciToPciBridgeConfSpace {
                    vendor_id,
                    device_id: (register(0x00) >> 16) as u16,
                    command: register(0x04) as u16,
                    status: (register(0x04) >> 16) as u16,
                    revision_id: register(0x08) as u8,
                    prog_if: (register(0x08) >> 8) as u8,
                    subclass: (register(0x08) >> 16) as u8,
                    class_code: (register(0x08) >> 24) as u8,
                    cache_line_size: register(0x0C) as u8,
                    latency_timer: (register(0x0C) >> 8) as u8,
                    header_type: (register(0x0C) >> 16) as u8,
                    bist: (register(0x0C) >> 24) as u8,
                    bar0: register(0x10),
                    bar1: register(0x14),
                    primary_bus_num: register(0x18) as u8,
                    secondary_bus_num: (register(0x18) >> 8) as u8,
                    subordinate_bus_num: (register(0x18) >> 16) as u8,
                    secondary_latency_timer: (register(0x18) >> 24) as u8,
                    io_base: register(0x1C) as u8,
                    io_limit: (register(0x1C) >> 8) as u8,
                    secondary_status: (register(0x1C) >> 16) as u16,
                    memory_base: register(0x20) as u16,
                    memory_limit: (register(0x20) >> 16) as u16,
                    prefetchable_memory_base: register(0x24) as u16,
                    prefetchable_memory_limit: (register(0x24) >> 16) as u16,
                    prefetchable_base_upper_32_bits: register(0x28),
                    prefetchable_limit_upper_32_bits: register(0x2C),
                    io_limit_upper_16_bits: register(0x30) as u16,
                    io_base_upper_16_bits: (register(0x30) >> 16) as u16,
                    capability_ptr: register(0x34) as u8,
                    expansion_rom_base_addr: register(0x38),
                    interrupt_line: register(0x3C) as u8,
                    interrupt_pin: (register(0x3C) >> 8) as u8,
                    bridge_control: (register(0x3C) >> 16) as u16,
                };
                Some(ConfSpace::PciToPciBridge(conf_space))
            }
            other => {
                println!("PCI: ignoring header type 0x{:02X}", other);
                None
            }
        }
    }

    fn header_type(&self, function_num: u8) -> u8 {
        (self.register(function_num, 0x0C) >> 16) as u8
    }

    fn register(&self, function_num: u8, offset: u8) -> u32 {
        let addr = ConfAddressBuilder::new()
            .enable_bit(true)
            .bus_num(self.bus_num)
            .device_num(self.device_num)
            .function_num(function_num)
            .register_offset(offset)
            .done();
        unsafe {
            port_io::outl(PORT_CONFIG_ADDRESS, addr);
        }
        let value = unsafe { port_io::inl(PORT_CONFIG_DATA) };
        value
    }
}

#[derive(Clone)]
enum DeviceClass {
    Unknown,
}

#[derive(Clone, Copy, Debug)]
enum ConfSpace {
    Device(DeviceConfSpace),
    PciToPciBridge(PciToPciBridgeConfSpace),
}

#[allow(dead_code)]
#[derive(Clone, Copy, Debug)]
struct DeviceConfSpace {
    vendor_id: u16,
    device_id: u16,
    command: u16,
    status: u16,
    revision_id: u8,
    prog_if: u8,
    subclass: u8,
    class_code: u8,
    cache_line_size: u8,
    latency_timer: u8,
    header_type: u8,
    bist: u8,
    bar0: u32,
    bar1: u32,
    bar2: u32,
    bar3: u32,
    bar4: u32,
    bar5: u32,
    cardbus_cis_ptr: u32,
    subsystem_vendor_id: u16,
    subsystem_id: u16,
    expansion_rom_base_addr: u32,
    capabilites_ptr: u8,
    interrupt_line: u8,
    interrupt_pin: u8,
    min_grant: u8,
    max_latency: u8,
}

#[allow(dead_code)]
#[derive(Clone, Copy, Debug)]
struct PciToPciBridgeConfSpace {
    vendor_id: u16,
    device_id: u16,
    command: u16,
    status: u16,
    revision_id: u8,
    prog_if: u8,
    subclass: u8,
    class_code: u8,
    cache_line_size: u8,
    latency_timer: u8,
    header_type: u8,
    bist: u8,
    bar0: u32,
    bar1: u32,
    primary_bus_num: u8,
    secondary_bus_num: u8,
    subordinate_bus_num: u8,
    secondary_latency_timer: u8,
    io_base: u8,
    io_limit: u8,
    secondary_status: u16,
    memory_base: u16,
    memory_limit: u16,
    prefetchable_memory_base: u16,
    prefetchable_memory_limit: u16,
    prefetchable_base_upper_32_bits: u32,
    prefetchable_limit_upper_32_bits: u32,
    io_limit_upper_16_bits: u16,
    io_base_upper_16_bits: u16,
    capability_ptr: u8,
    expansion_rom_base_addr: u32,
    interrupt_line: u8,
    interrupt_pin: u8,
    bridge_control: u16,
}

const PORT_CONFIG_ADDRESS: u16 = 0xCF8;
const PORT_CONFIG_DATA: u16 = 0xCFC;

static mut PCI: Pci = Pci::new();

pub fn init() {
    unsafe {
        PCI.enumerate();
    }

    for (host_bus_num, host_bus) in unsafe { &PCI }.host_buses.iter() {
        print!("Host bus 0x{:02X} : ", host_bus_num);
        print_bus(16, host_bus);
    }
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
            if j != 0 {
                for _ in 0..offset + 14 {
                    print!(" ");
                }
            }
            if let Some(conf_space) = device.functions[function_num] {
                match conf_space {
                    ConfSpace::Device(cs) => {
                        println!(
                            "Function {} {:04X}:{:04X}",
                            function_num, cs.vendor_id, cs.device_id,
                        );
                    }
                    ConfSpace::PciToPciBridge(_) => {
                        // FIXME: can a function be a PCI-to-PCI bridge?
                        println!("PCI to PCI bridge not in a right place");
                    }
                    _ => println!("unreachable"),
                }
            } else {
                println!(
                    "Ignoring {} {:04X}:{:04X} Header Type 0x{:02X}",
                    function_num,
                    device.register(function_num as u8, 0x00) as u16,
                    (device.register(function_num as u8, 0x00) >> 16) as u16,
                    device.header_type(function_num as u8),
                );
            }
        }
    }
}
