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

use core::cmp;
use core::fmt;
use core::ops;

#[derive(Clone, Copy)]
pub struct Region<T: RegionType> {
    pub start: T,
    pub end: T,
}

impl<T: RegionType> Region<T> {
    pub fn from_start_len(start: T, len: T) -> Self {
        Region {
            start,
            end: start + len,
        }
    }

    pub fn range(&self) -> ops::Range<T> {
        self.start..self.end
    }

    pub fn len(&self) -> T {
        self.end - self.start
    }

    pub fn overlapping_with(&self, region: &Self) -> OverlappingWith {
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
        } else if self.end > region.start && self.end < region.end {
            return OverlappingWith::EndsIn;
        }
        return OverlappingWith::NoOverlap;
    }

    pub fn conflicts_with(&self, region: &Self) -> bool {
        self.overlapping_with(region) != OverlappingWith::NoOverlap
    }

    pub fn contains(&self, something: &T) -> bool {
        self.range().contains(something)
    }

    pub fn align_boundaries_at(&self, at: T) -> Region<T> {
        // assert_eq!(at.count_ones(), 1, "at must be a power of two");
        Region {
            start: self.start & !(at - T::one()),
            end: (self.end + at - T::one()) & !(at - T::one()),
        }
    }

    pub fn is_in(&self, other: &Self) -> bool {
        self.overlapping_with(other) == OverlappingWith::IsIn
    }
}

impl<T: RegionType + fmt::UpperHex> fmt::Debug for Region<T> {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt.write_fmt(format_args!("0x{:08X}..0x{:08X}", self.start, self.end))
    }
}

#[derive(PartialEq, Debug)]
pub enum OverlappingWith {
    Covers,
    StartsIn,
    IsIn,
    EndsIn,
    NoOverlap,
}

pub trait RegionType:
    Copy
    + ops::Add<Output = Self>
    + ops::Sub<Output = Self>
    + ops::Not<Output = Self>
    + ops::BitAnd<Output = Self>
    + cmp::PartialOrd
{
    fn one() -> Self;
}

impl RegionType for u32 {
    fn one() -> Self {
        1
    }
}
impl RegionType for u64 {
    fn one() -> Self {
        1
    }
}
impl RegionType for usize {
    fn one() -> Self {
        1
    }
}
