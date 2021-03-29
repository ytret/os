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

use alloc::boxed::Box;
use alloc::collections::vec_deque::VecDeque;
use alloc::rc::Rc;
use core::cell::RefCell;

use crate::arch::keyboard::{Event, EventListener, Key, KEYBOARD};
use crate::char_device::{CharDevice, ReadErr, WriteErr};
use crate::kernel_static::Mutex;
use crate::vga;

const MAX_KBD_EVENTS: usize = 64;

pub struct Console {
    writer: vga::Writer,
    kbd_events: VecDeque<Event>,

    shift: bool,
    caps_lock: bool,
    num_lock: bool,
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
            kbd_events: VecDeque::new(),

            shift: false,
            caps_lock: false,
            num_lock: false,
        }
    }

    fn try_resolve_into_ascii(&mut self) -> Option<u8> {
        loop {
            if self.kbd_events.is_empty() {
                // println!("[CONSOLE] Empty keyboard events buffer.");
                return None;
            }
            let res = self.resolve_event();
            if let ResolveEvent::Ascii(ascii) = res {
                return Some(ascii);
            }
        }
    }

    fn resolve_event(&mut self) -> ResolveEvent {
        let event = self.kbd_events.pop_front().unwrap();
        let symbol = |s1: &str, s2: &str| {
            if event.pressed {
                let ch = if !self.shift {
                    s1.as_bytes()[0]
                } else {
                    s2.as_bytes()[0]
                };
                ResolveEvent::Ascii(ch)
            } else {
                ResolveEvent::None
            }
        };
        let letter = |s: &str| {
            if event.pressed {
                let mut ch = s.as_bytes()[0];
                if self.is_uppercase() {
                    ch -= 32;
                }
                ResolveEvent::Ascii(ch)
            } else {
                ResolveEvent::None
            }
        };
        let no_numlock_symbol = |s: &str| {
            if event.pressed {
                if !self.num_lock {
                    let ch = s.as_bytes()[0];
                    return ResolveEvent::Ascii(ch);
                }
            }
            ResolveEvent::None
        };
        match event.key {
            Key::CapsLock => {
                if !event.pressed {
                    self.caps_lock = !self.caps_lock;
                }
                ResolveEvent::FlagUpdate
            }
            Key::LeftShift | Key::RightShift => {
                self.shift = event.pressed;
                ResolveEvent::FlagUpdate
            }

            Key::Backtick => symbol("`", "~"),
            Key::Space => symbol(" ", " "),

            Key::One => symbol("1", "!"),
            Key::Two => symbol("2", "@"),
            Key::Three => symbol("3", "#"),
            Key::Four => symbol("4", "$"),
            Key::Five => symbol("5", "%"),
            Key::Six => symbol("6", "^"),
            Key::Seven => symbol("7", "&"),
            Key::Eight => symbol("8", "*"),
            Key::Nine => symbol("9", "("),
            Key::Zero => symbol("0", ")"),

            Key::Minus => symbol("-", "_"),
            Key::Equals => symbol("=", "+"),

            Key::A => letter("a"),
            Key::B => letter("b"),
            Key::C => letter("c"),
            Key::D => letter("d"),
            Key::E => letter("e"),
            Key::F => letter("f"),
            Key::G => letter("g"),
            Key::H => letter("h"),
            Key::I => letter("i"),
            Key::J => letter("j"),
            Key::K => letter("k"),
            Key::L => letter("l"),
            Key::M => letter("m"),
            Key::N => letter("n"),
            Key::O => letter("o"),
            Key::P => letter("p"),
            Key::Q => letter("q"),
            Key::R => letter("r"),
            Key::S => letter("s"),
            Key::T => letter("t"),
            Key::U => letter("u"),
            Key::V => letter("v"),
            Key::W => letter("w"),
            Key::X => letter("x"),
            Key::Y => letter("y"),
            Key::Z => letter("z"),

            Key::LeftSquareBracket => symbol("[", "{"),
            Key::RightSquareBracket => symbol("]", "}"),
            Key::Backslash => symbol("\\", "|"),
            Key::Semicolon => symbol(";", ":"),
            Key::Apostrophe => symbol("'", "\""),
            Key::Enter => symbol("\n", "\n"),

            Key::Comma => symbol(",", "<"),
            Key::Period => symbol(".", ">"),
            Key::Slash => symbol("/", "?"),

            Key::NumpadSlash => symbol("/", "/"),
            Key::NumpadAsterisk => symbol("*", "*"),
            Key::NumpadMinus => symbol("-", "-"),
            Key::NumpadPlus => symbol("+", "+"),
            Key::NumpadEnter => symbol("\n", "\n"),
            Key::NumpadPeriod => no_numlock_symbol("."),

            Key::NumpadOne => no_numlock_symbol("1"),
            Key::NumpadTwo => no_numlock_symbol("2"),
            Key::NumpadThree => no_numlock_symbol("3"),
            Key::NumpadFour => no_numlock_symbol("4"),
            Key::NumpadFive => no_numlock_symbol("5"),
            Key::NumpadSix => no_numlock_symbol("6"),
            Key::NumpadSeven => no_numlock_symbol("7"),
            Key::NumpadEight => no_numlock_symbol("8"),
            Key::NumpadNine => no_numlock_symbol("9"),
            Key::NumpadZero => no_numlock_symbol("0"),

            _ => ResolveEvent::None,
        }
    }

    fn is_uppercase(&self) -> bool {
        self.shift || self.caps_lock
    }
}

impl EventListener for Console {
    fn receive_event(&mut self, event: Event) {
        if self.kbd_events.len() < MAX_KBD_EVENTS {
            self.kbd_events.push_back(event);
        } else {
            println!("[CONSOLE] Keyboard event buffer is full.");
        }
    }
}

impl CharDevice for Console {
    fn read(&mut self) -> Result<u8, ReadErr> {
        let maybe_ascii = self.try_resolve_into_ascii();
        if let Some(ascii) = maybe_ascii {
            // println!("[CONSOLE] ascii = 0x{:02X}", ascii);
            Ok(ascii)
        } else {
            // FIXME: block the thread
            Ok(0x00)
            // Err(ReadErr::NotReadable)
        }
        // Err(ReadErr::NotReadable)
    }

    fn read_many(&mut self, len: usize) -> Result<Box<[u8]>, ReadErr> {
        assert_eq!(len, 1);
        Ok(Box::new([self.read().unwrap()]))
        // Err(ReadErr::NotReadable)
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

enum ResolveEvent {
    None,
    Ascii(u8),
    FlagUpdate,
}

kernel_static! {
    pub static ref CONSOLE: Mutex<Option<Rc<RefCell<Console>>>>
        = Mutex::new(Some(Rc::new(RefCell::new(Console::new()))));
}

pub fn init() {
    unsafe {
        let rc_console = Rc::clone(&CONSOLE.lock().as_ref().unwrap());
        KEYBOARD.as_mut().unwrap().set_listener(rc_console);
    }
}
