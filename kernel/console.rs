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
use core::cell::RefCell;

use crate::char_device::{CharDevice, ReadErr, WriteErr};
use crate::kernel_static::Mutex;
use crate::vga;

pub struct Console {
    writer: vga::Writer,
}

impl Console {
    pub fn new() -> Self {
        Console {
            writer: vga::Writer {
                pos: vga::CursorPos::new(24, 0),
                color_code: vga::ColorCode::new(
                    vga::Color::White,
                    vga::Color::Black,
                ),
                buffer: 0xB8000 as *mut vga::Buffer,
            },
        }
    }
}

impl CharDevice for Console {
    fn read(&self) -> Result<u8, ReadErr> {
        Err(ReadErr::NotReadable)
    }

    fn read_many(&self, _len: usize) -> Result<Box<[u8]>, ReadErr> {
        Err(ReadErr::NotReadable)
    }

    fn write(&mut self, byte: u8) -> Result<(), WriteErr> {
        self.writer.write_char(byte);
        Ok(())
    }

    fn write_many(&mut self, bytes: &[u8]) -> Result<(), WriteErr> {
        for byte in bytes {
            self.write(*byte)?;
        }
        Ok(())
    }
}

kernel_static! {
    pub static ref CONSOLE: Mutex<Rc<RefCell<Console>>> = Mutex::new(
        Rc::new(RefCell::new(Console::new())),
    );
}
