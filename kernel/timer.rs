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

pub trait Timer {
    fn init_with_period_ms(period_ms: usize) -> Self
    where
        Self: Sized;
    fn period_ms(&self) -> usize;

    fn set_callback(&mut self, callback: TimerCallback);
    fn callback(&self) -> Option<TimerCallback>;
}

pub type TimerCallback = fn();

pub static mut TIMER: Option<Box<dyn Timer>> = None;
