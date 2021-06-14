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

use alloc::rc::Rc;
use alloc::vec::Vec;
use core::cell::RefCell;

use crate::arch::interrupts::IDT;
use crate::arch::dev::pic::PIC;
use crate::port::{Port, PortBuilder};

extern "C" {
    fn irq1_handler();
}

const IRQ: u8 = 1;

const PORT_DATA: u16 = 0x60;
const PORT_CMD: u16 = 0x64;
const PORT_STATUS: u16 = 0x64;

// const RSP_ACK: u8 = 0xFA;
// const RSP_RESEND: u8 = 0xFE;
// const RSP_ECHO: u8 = 0xEE;

#[derive(Debug)]
#[repr(u8)]
enum Response {
    Ack,
    Resend,
    Error,
    Unknown,
}

impl From<u8> for Response {
    fn from(value: u8) -> Self {
        match value {
            0xFA => Response::Ack,
            0xFE => Response::Resend,
            0x00 | 0xFF => Response::Error,
            _ => Response::Unknown,
        }
    }
}

pub struct Keyboard {
    data: Port,
    _cmd: Port,
    _status: Port,

    scseq: Vec<u8>, // current scancode sequence
    listener: Option<Rc<RefCell<dyn EventListener>>>,
}

impl Keyboard {
    pub fn new() -> Self {
        Keyboard {
            data: PortBuilder::port(PORT_DATA).size(8).done(),
            _cmd: PortBuilder::port(PORT_CMD).write_size(8).done(),
            _status: PortBuilder::port(PORT_STATUS).read_size(8).done(),

            scseq: Vec::new(),
            listener: None,
        }
    }

    unsafe fn feed(&mut self) {
        let sc = self.data.read::<u8>();
        self.scseq.push(sc);
        // println!("[KBD] scseq = {:02X?}", self.scseq);
        let maybe_event = self.try_resolve();
        if let Some(event) = maybe_event {
            // println!("[KBD] event = {:?}", event);
            if self.listener.is_some() {
                self.listener
                    .as_ref()
                    .unwrap()
                    .borrow_mut()
                    .receive_event(event);
            } else {
                println!("[KBD] There is no event listener set.");
            }
        }
    }

    fn try_resolve(&mut self) -> Option<Event> {
        if self.scseq.len() == 0 {
            return None;
        } else if self.scseq.len() == 1 {
            let mut keysc = self.scseq[0];
            let mut released = false;
            if 0x81 <= keysc && keysc <= 0xD8 {
                // FIXME: figure out whether each key-pressed-scancode below
                // indeed has a key-released-scancode counterpart 0x80 above it.
                keysc -= 0x80;
                released = true;
            }
            let maybe_key = match keysc {
                0x01 => Some(Key::Escape),
                0x29 => Some(Key::Backtick),
                0x0F => Some(Key::Tab),
                0x3A => Some(Key::CapsLock),
                0x2A => Some(Key::LeftShift),
                0x36 => Some(Key::RightShift),
                0x1D => Some(Key::LeftCtrl),
                0x38 => Some(Key::LeftAlt),
                0x39 => Some(Key::Space),

                0x3B => Some(Key::F1),
                0x3C => Some(Key::F2),
                0x3D => Some(Key::F3),
                0x3E => Some(Key::F4),
                0x3F => Some(Key::F5),
                0x40 => Some(Key::F6),
                0x41 => Some(Key::F7),
                0x42 => Some(Key::F8),
                0x43 => Some(Key::F9),
                0x44 => Some(Key::F10),
                0x57 => Some(Key::F11),
                0x58 => Some(Key::F12),

                0x45 => Some(Key::NumLock),
                0x46 => Some(Key::ScrollLock),

                0x02 => Some(Key::One),
                0x03 => Some(Key::Two),
                0x04 => Some(Key::Three),
                0x05 => Some(Key::Four),
                0x06 => Some(Key::Five),
                0x07 => Some(Key::Six),
                0x08 => Some(Key::Seven),
                0x09 => Some(Key::Eight),
                0x0A => Some(Key::Nine),
                0x0B => Some(Key::Zero),

                0x0C => Some(Key::Minus),
                0x0D => Some(Key::Equals),
                0x0E => Some(Key::Backspace),

                0x10 => Some(Key::Q),
                0x11 => Some(Key::W),
                0x12 => Some(Key::E),
                0x13 => Some(Key::R),
                0x14 => Some(Key::T),
                0x15 => Some(Key::Y),
                0x16 => Some(Key::U),
                0x17 => Some(Key::I),
                0x18 => Some(Key::O),
                0x19 => Some(Key::P),
                0x1A => Some(Key::LeftSquareBracket),
                0x1B => Some(Key::RightSquareBracket),
                0x2B => Some(Key::Backslash),
                0x1E => Some(Key::A),
                0x1F => Some(Key::S),
                0x20 => Some(Key::D),
                0x21 => Some(Key::F),
                0x22 => Some(Key::G),
                0x23 => Some(Key::H),
                0x24 => Some(Key::J),
                0x25 => Some(Key::K),
                0x26 => Some(Key::L),
                0x27 => Some(Key::Semicolon),
                0x28 => Some(Key::Apostrophe),
                0x1C => Some(Key::Enter),
                0x2C => Some(Key::Z),
                0x2D => Some(Key::X),
                0x2E => Some(Key::C),
                0x2F => Some(Key::V),
                0x30 => Some(Key::B),
                0x31 => Some(Key::N),
                0x32 => Some(Key::M),
                0x33 => Some(Key::Comma),
                0x34 => Some(Key::Period),
                0x35 => Some(Key::Slash),

                0x37 => Some(Key::NumpadAsterisk),
                0x4A => Some(Key::NumpadMinus),
                0x4E => Some(Key::NumpadPlus),
                0x53 => Some(Key::NumpadPeriod),

                0x4F => Some(Key::NumpadOne),
                0x50 => Some(Key::NumpadTwo),
                0x51 => Some(Key::NumpadThree),
                0x4B => Some(Key::NumpadFour),
                0x4C => Some(Key::NumpadFive),
                0x4D => Some(Key::NumpadSix),
                0x47 => Some(Key::NumpadSeven),
                0x48 => Some(Key::NumpadEight),
                0x49 => Some(Key::NumpadNine),
                0x52 => Some(Key::NumpadZero),

                _ => None,
            };

            if let Some(key) = maybe_key {
                self.scseq.truncate(0);
                return Some(Event {
                    key,
                    pressed: !released,
                });
            }
        } else if self.scseq.len() == 2 && self.scseq[0] == 0xE0 {
            let mut keysc = self.scseq[1];
            let mut released = false;
            if 0x99 <= keysc && keysc <= 0xED {
                released = true;
                keysc -= 0x80;
            }

            let maybe_key = match keysc {
                0x1D => Some(Key::RightCtrl),
                0x38 => Some(Key::RightAlt),
                0x5D => Some(Key::Menu),
                0x5B => Some(Key::Logo),

                0x52 => Some(Key::Insert),
                0x53 => Some(Key::Delete),

                0x47 => Some(Key::Home),
                0x4F => Some(Key::End),
                0x49 => Some(Key::PageUp),
                0x51 => Some(Key::PageDown),

                0x4B => Some(Key::LeftArrow),
                0x48 => Some(Key::UpArrow),
                0x50 => Some(Key::DownArrow),
                0x4D => Some(Key::RightArrow),

                0x35 => Some(Key::NumpadSlash),
                0x1C => Some(Key::NumpadEnter),

                _ => None,
            };

            if let Some(key) = maybe_key {
                self.scseq.truncate(0);
                return Some(Event {
                    key,
                    pressed: !released,
                });
            }
        } else if self.scseq.len() == 4 {
            if self.scseq[0] == 0xE0 && self.scseq[2] == 0xE0 {
                if self.scseq[1] == 0x2A && self.scseq[3] == 0x37 {
                    self.scseq.truncate(0);
                    return Some(Event {
                        key: Key::PrintScreenSysRq,
                        pressed: true,
                    });
                } else if self.scseq[1] == 0xB7 && self.scseq[3] == 0xAA {
                    self.scseq.truncate(0);
                    return Some(Event {
                        key: Key::PrintScreenSysRq,
                        pressed: false,
                    });
                }
            }
        } else if self.scseq.len() == 6 {
            if self.scseq[0] == 0xE1
                && self.scseq[1] == 0x1D
                && self.scseq[2] == 0x45
                && self.scseq[3] == 0xE1
                && self.scseq[4] == 0x9D
                && self.scseq[5] == 0xC5
            {
                self.scseq.truncate(0);
                return Some(Event {
                    key: Key::PauseBreak,
                    pressed: true,
                });
            }
        } else if self.scseq.len() > 6 {
            println!("[KBD] Discarding unknown sequence {:02X?}.", self.scseq);
            self.scseq.truncate(0);
        }
        None
    }

    pub fn set_listener(
        &mut self,
        new_listener: Rc<RefCell<dyn EventListener>>,
    ) {
        self.listener = Some(new_listener);
    }
}

#[derive(Debug)]
pub struct Event {
    pub key: Key,
    pub pressed: bool,
}

#[derive(PartialEq, Debug)]
pub enum Key {
    Escape,
    Backtick,
    Tab,
    CapsLock,
    LeftShift,
    RightShift,
    LeftCtrl,
    RightCtrl,
    LeftAlt,
    RightAlt,
    Space,
    Menu,
    Logo,

    F1,
    F2,
    F3,
    F4,
    F5,
    F6,
    F7,
    F8,
    F9,
    F10,
    F11,
    F12,

    PrintScreenSysRq,
    PauseBreak,
    NumLock,
    ScrollLock,

    Insert,
    Delete,

    Home,
    End,
    PageUp,
    PageDown,

    One,
    Two,
    Three,
    Four,
    Five,
    Six,
    Seven,
    Eight,
    Nine,
    Zero,

    Minus,
    Equals,
    Backspace,

    Q,
    W,
    E,
    R,
    T,
    Y,
    U,
    I,
    O,
    P,
    LeftSquareBracket,
    RightSquareBracket,
    Backslash,
    A,
    S,
    D,
    F,
    G,
    H,
    J,
    K,
    L,
    Semicolon,
    Apostrophe,
    Enter,
    Z,
    X,
    C,
    V,
    B,
    N,
    M,
    Comma,
    Period,
    Slash,

    LeftArrow,
    UpArrow,
    DownArrow,
    RightArrow,

    NumpadSlash,
    NumpadAsterisk,
    NumpadMinus,
    NumpadPlus,
    NumpadEnter,
    NumpadPeriod,

    NumpadOne,
    NumpadTwo,
    NumpadThree,
    NumpadFour,
    NumpadFive,
    NumpadSix,
    NumpadSeven,
    NumpadEight,
    NumpadNine,
    NumpadZero,
}

pub trait EventListener {
    fn receive_event(&mut self, event: Event);
}

pub static mut KEYBOARD: Option<Keyboard> = None;

pub fn init() {
    println!("[KBD] Initializing keyboard.");
    unsafe {
        KEYBOARD = Some(Keyboard::new());
    }
    IDT.lock().interrupts[IRQ as usize].set_handler(irq1_handler);
    unsafe {
        PIC.set_irq_mask(IRQ, false);
    }
}

#[no_mangle]
pub extern "C" fn keyboard_irq_handler() {
    unsafe {
        KEYBOARD.as_mut().unwrap().feed();
        PIC.send_eoi(IRQ);
    }
}
