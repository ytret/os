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
use alloc::vec::Vec;

use crate::arch::port_io;

#[derive(Clone)]
pub struct Port {
    port: u16,
    read_sizes: Vec<u8>,
    write_sizes: Vec<u8>,
}

impl Port {
    pub unsafe fn read<T: ReadableFromPort>(&self) -> T {
        let size = 8 * size_of::<T>();
        if self.can_read_size(size) {
            T::read_from_port(self.port)
        } else {
            panic!("Cannot read size {} from port 0x{:02X}", size, self.port);
        }
    }

    pub unsafe fn write<T: WritableToPort>(&self, value: T) {
        let size = 8 * size_of::<T>();
        if self.can_write_size(size) {
            value.write_to_port(self.port)
        } else {
            panic!("Cannot write size {} to port 0x{:02X}", size, self.port);
        }
    }

    fn can_read_size(&self, size: usize) -> bool {
        assert_eq!(size & !0xFF, 0, "too big size provided");
        let size = size as u8;
        self.read_sizes.iter().any(|&x| x == size)
    }

    fn can_write_size(&self, size: usize) -> bool {
        assert_eq!(size & !0xFF, 0, "too big size provided");
        let size = size as u8;
        self.write_sizes.iter().any(|&x| x == size)
    }
}

pub struct PortBuilder {
    port: Port,
}

impl PortBuilder {
    pub fn port(port_num: u16) -> Self {
        PortBuilder {
            port: Port {
                port: port_num,
                read_sizes: Vec::new(),
                write_sizes: Vec::new(),
            }
        }
    }

    pub fn size(&mut self, size: u8) -> &mut Self {
        self.read_size(size);
        self.write_size(size);
        self
    }

    pub fn read_size(&mut self, size: u8) -> &mut Self {
        self.port.read_sizes.push(size);
        self
    }

    pub fn write_size(&mut self, size: u8) -> &mut Self {
        self.port.write_sizes.push(size);
        self
    }

    pub fn done(&mut self) -> Port {
        self.port.read_sizes.shrink_to_fit();
        self.port.write_sizes.shrink_to_fit();
        self.port.clone()
    }
}

pub trait ReadableFromPort: Sized {
    unsafe fn read_from_port(port: u16) -> Self;
}

impl ReadableFromPort for u8 {
    unsafe fn read_from_port(port: u16) -> u8 {
        port_io::inb(port)
    }
}

impl ReadableFromPort for u16 {
    unsafe fn read_from_port(port: u16) -> u16 {
        port_io::inw(port)
    }
}

impl ReadableFromPort for u32 {
    unsafe fn read_from_port(port: u16) -> u32 {
        port_io::inl(port)
    }
}

pub trait WritableToPort: Sized {
    unsafe fn write_to_port(self, port: u16);
}

impl WritableToPort for u8 {
    unsafe fn write_to_port(self, port: u16) {
        port_io::outb(port, self);
    }
}

impl WritableToPort for u16 {
    unsafe fn write_to_port(self, port: u16) {
        port_io::outw(port, self);
    }
}

impl WritableToPort for u32 {
    unsafe fn write_to_port(self, port: u16) {
        port_io::outl(port, self);
    }
}
