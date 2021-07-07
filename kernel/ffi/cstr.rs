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

use alloc::borrow::ToOwned;
use core::fmt;
use core::{slice, str};

use super::cstring::CString;

/// Representation of a borrowed C string.
///
/// Cf. std::ffi::CStr.
#[repr(transparent)]
pub struct CStr([u8]);

impl CStr {
    /// Safely wraps a C string.
    ///
    /// # Safety
    /// `p` must point to a nul-terminated C string.
    pub unsafe fn from_ptr<'a>(
        p: *const u8,
        max_len: usize,
    ) -> Result<&'a CStr, FromPtrErr> {
        let mut nul_at = None;
        for i in 0..max_len {
            if *p.add(i) == 0 {
                nul_at = Some(i);
                break;
            }
        }
        if let Some(nul_at) = nul_at {
            let bytes = slice::from_raw_parts(p, nul_at + 1);
            Ok(&*(bytes as *const [u8] as *const CStr))
        } else {
            Err(FromPtrErr::MaxLenExceeded)
        }
    }

    pub fn to_str(&self) -> Result<&str, str::Utf8Error> {
        str::from_utf8(self.0.get(0..self.0.len() - 1).unwrap())
    }

    pub fn to_bytes_with_nul(&self) -> &[u8] {
        &self.0
    }
}

impl ToOwned for CStr {
    type Owned = CString;

    fn to_owned(&self) -> Self::Owned {
        CString::new(self.to_bytes_with_nul().get(0..self.0.len() - 1).unwrap())
            .unwrap()
    }
}

impl fmt::Debug for CStr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "\"{}\"",
            self.to_str().expect("CStr::to_str failed").escape_debug(),
        )
    }
}

#[derive(Debug)]
pub enum FromPtrErr {
    MaxLenExceeded,
}
