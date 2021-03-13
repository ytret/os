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

use core::cmp;
use core::fmt;
use core::ops;

#[derive(Clone, Copy)]
pub struct Region<T: Copy + ops::Sub<Output = T> + cmp::PartialOrd = u64> {
    pub start: T,
    pub end: T,
}

impl<T: fmt::UpperHex + Copy + ops::Sub<Output = T> + cmp::PartialOrd>
    fmt::Debug for Region<T>
{
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt.write_fmt(format_args!("0x{:08X}..0x{:08X}", self.start, self.end))
    }
}

pub enum OverlappingWith {
    Covers,
    StartsIn,
    IsIn,
    EndsIn,
    NoOverlap,
}

impl<T: Copy + ops::Sub<Output = T> + cmp::PartialOrd> Region<T> {
    pub fn overlapping_with(&self, region: Region<T>) -> OverlappingWith {
        if self.start < region.start && self.end > region.end {
            return OverlappingWith::Covers;
        }
        if self.start >= region.start {
            if self.end > region.end && self.start < region.end {
                return OverlappingWith::StartsIn;
            }
            if self.end <= region.end {
                return OverlappingWith::IsIn;
            }
        } else if self.end >= region.start && self.end <= region.end {
            return OverlappingWith::EndsIn;
        }
        return OverlappingWith::NoOverlap;
    }
}
