use core::fmt;

pub struct CursorPos {
    row: usize,
    col: usize,
}

#[allow(dead_code)]
#[repr(u8)]
enum Color
{
    Black, Blue, Green, Cyan, Red, Purple, Brown, Gray, DarkGray, LightBlue,
    LightGreen, LightCyan, LightRed, LightPurple, Yellow, White
}

#[derive(Clone, Copy)]
#[repr(transparent)]
pub struct ColorCode(u8);

impl ColorCode {
    const fn new(fg: Color, bg: Color) -> ColorCode {
        ColorCode((bg as u8) << 4 | (fg as u8))
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
    pub const fn new(pos: CursorPos, color_code: ColorCode) -> Writer {
        Writer {
            pos,
            color_code,
            buffer: 0xB8000 as *mut Buffer,
        }
    }

    pub fn write_char(&mut self, ch: u8) {
        match ch {
            b'\n' => self.new_line(),
            ch => {
                if self.pos.col >= BUFFER_WIDTH {
                    self.new_line();
                }
                unsafe {
                    (*self.buffer).chars[self.pos.row][self.pos.col]
                        = ScreenChar {
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
                (*self.buffer).chars[row-num_rows] = (*self.buffer).chars[row];
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

static mut WRITER: Writer
    = Writer::new(CursorPos { row: 0, col: 0 },
                  ColorCode::new(Color::White, Color::Black));

pub fn init() {
    unsafe {
        WRITER.clear_screen();
    }
}

pub fn _print(args: fmt::Arguments) {
    use core::fmt::Write;
    unsafe {
        WRITER.write_fmt(args).unwrap();
    }
}
