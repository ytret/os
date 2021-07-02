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

use alloc::vec::Vec;

/// A type representing an owned, C-compatible, null-terminated string with no
/// null bytes in the middle.
///
/// Cf. std::ffi::CString.
pub struct CString {
    bytes: Vec<u8>,
}

impl CString {
    pub fn new<T: Into<Vec<u8>>>(t: T) -> Result<Self, NulError> {
        let mut bytes = t.into();

        if let Some(pos) = bytes.iter().position(|&x| x == 0) {
            return Err(NulError(pos));
        }

        bytes.push(0);
        Ok(CString { bytes })
    }

    pub fn as_ptr(&self) -> *const u8 {
        self.bytes.as_ptr()
    }
}

#[derive(Debug)]
pub struct NulError(usize);
