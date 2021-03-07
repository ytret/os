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
use crate::kernel_static::Mutex;

use core::fmt;
use core::fmt::Write;

extern "C" {
    fn get_eflags() -> u32;
}

pub struct CursorPos {
    row: usize,
    col: usize,
}

#[allow(dead_code)]
#[repr(u8)]
enum Color {
    Black,
    Blue,
    Green,
    Cyan,
    Red,
    Purple,
    Brown,
    Gray,
    DarkGray,
    LightBlue,
    LightGreen,
    LightCyan,
    LightRed,
    LightPurple,
    Yellow,
    White,
}

#[derive(Clone, Copy)]
#[repr(transparent)]
pub struct ColorCode(u8);

impl ColorCode {
    fn new(fg: Color, bg: Color) -> Self {
        Self((bg as u8) << 4 | (fg as u8))
    }
}

#[derive(Clone, Copy)]
#[repr(C)]
struct ScreenChar {
    ascii_char: u8,
    color_code: ColorCode,
}

const BUFFER_WIDTH: usize = 80;
const BUFFER_HEIGHT: usize = 25;

#[repr(transparent)]
struct Buffer {
    chars: [[ScreenChar; BUFFER_WIDTH]; BUFFER_HEIGHT],
}

pub struct Writer {
    pos: CursorPos,
    color_code: ColorCode,
    buffer: *mut Buffer,
}

impl Writer {
    pub fn write_char(&mut self, ch: u8) {
        // Duplicate to COM1.
        unsafe {
            port_io::outb(0x3F8, ch);
        }

        match ch {
            b'\n' => self.new_line(),
            ch => {
                if self.pos.col >= BUFFER_WIDTH {
                    self.new_line();
                }
                unsafe {
                    (*self.buffer).chars[self.pos.row][self.pos.col] =
                        ScreenChar {
                            ascii_char: ch,
                            color_code: self.color_code,
                        };
                }
                self.pos.col += 1;
            }
        }
    }

    pub fn write_string(&mut self, s: &str) {
        for ch in s.bytes() {
            self.write_char(ch)
        }
    }

    fn new_line(&mut self) {
        self.pos.col = 0;
        self.pos.row += 1;
        if self.pos.row >= BUFFER_HEIGHT {
            self.scroll_screen(1);
            self.pos.row = BUFFER_HEIGHT - 1;
            self.clear_row(self.pos.row);
        }
    }

    fn scroll_screen(&mut self, num_rows: usize) {
        unsafe {
            for row in num_rows..BUFFER_HEIGHT {
                (*self.buffer).chars[row - num_rows] =
                    (*self.buffer).chars[row];
            }
        }
    }

    fn clear_row(&mut self, row: usize) {
        let blank = ScreenChar {
            ascii_char: b' ',
            color_code: self.color_code,
        };
        for col in 0..BUFFER_WIDTH {
            unsafe {
                (*self.buffer).chars[row][col] = blank;
            }
        }
    }

    fn clear_screen(&mut self) {
        for row in 0..BUFFER_HEIGHT {
            self.clear_row(row);
        }
    }
}

impl fmt::Write for Writer {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        self.write_string(s);
        Ok(())
    }
}

#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => ($crate::vga::_print(format_args!($($arg)*)));
}

#[macro_export]
macro_rules! println {
    () => ($crate::print!("\n"));
    ($($arg:tt)*) => ({
        $crate::print!("{}\n", format_args!($($arg)*));
    })
}

kernel_static! {
    static ref WRITER: Mutex<Writer> = Mutex::new(Writer {
            pos: CursorPos { row: 0, col: 0 },
            color_code: ColorCode::new(Color::White, Color::Black),
            buffer: 0xB8000 as *mut Buffer,
    });
}

pub fn init() {
    WRITER.lock().clear_screen();
}

pub fn _print(args: fmt::Arguments) {
    // The interrupts should be disabled when printing to the screen to prevent
    // a context switch from happening while WRITER is locked.  But using
    // SCHEDULER.stop_scheduling() here is a bit difficult, so we do a slightly
    // different thing.
    let do_sti = unsafe {
        // Check IF and disable it temporarily if it has not been already.
        match get_eflags() & (1 << 9) {
            0 => false,
            _ => {
                asm!("cli");
                true
            }
        }
    };
    {
        WRITER.lock().write_fmt(args).unwrap();
    }
    unsafe {
        // SCHEDULER.keep_scheduling();
        if do_sti {
            asm!("sti");
        }
    }
}
