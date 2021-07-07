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

use alloc::borrow::{Borrow, ToOwned};
use alloc::vec::Vec;
use core::convert::From;
use core::fmt;

use super::cstr::CStr;

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

    pub fn as_cstr(&self) -> &CStr {
        (*self).borrow()
    }
}

impl Borrow<CStr> for CString {
    fn borrow(&self) -> &CStr {
        unsafe { &*(self.bytes.as_slice() as *const [u8] as *const CStr) }
    }
}

impl From<&CStr> for CString {
    fn from(s: &CStr) -> Self {
        s.to_owned()
    }
}

impl fmt::Debug for CString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(Borrow::<CStr>::borrow(self), f)
    }
}

#[derive(Debug)]
pub struct NulError(usize);
