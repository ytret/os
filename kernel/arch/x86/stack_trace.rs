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

extern "C" {
    fn walk_stack(addr_array: *mut u32, max_len: u32) -> u32;
}

pub struct StackTrace {
    pub addresses: [u32; 32], // maybe 32 is enough
    pub length: usize,
}

impl StackTrace {
    pub fn walk_and_get() -> Self {
        let mut ret = Self {
            addresses: [0; 32],
            length: 0,
        };
        unsafe {
            ret.length = walk_stack(
                ret.addresses.as_mut_ptr(),
                ret.addresses.len() as u32,
            ) as usize;
        }
        ret
    }

    pub fn iter(&self) -> Iter {
        Iter {
            stack_trace: self,
            index: 0,
        }
    }
}

pub struct Iter<'a> {
    stack_trace: &'a StackTrace,
    index: usize,
}

impl<'a> Iterator for Iter<'a> {
    type Item = u32;

    fn next(&mut self) -> Option<Self::Item> {
        if self.index < self.stack_trace.length {
            let addr = self.stack_trace.addresses[self.index];
            self.index += 1;
            return Some(addr);
        } else {
            return None;
        }
    }
}
