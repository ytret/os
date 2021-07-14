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

macro_rules! bitflags_new {
    ($vis:vis struct $name:ident : $type:ty {
        $(
            const $flag:ident = $value:expr;
        )+
    }) => {
        #[derive(Eq, PartialEq, Clone, Copy)]
        #[repr(transparent)]
        $vis struct $name($type);

        #[allow(dead_code)]
        impl $name {
            $(const $flag: $name = $name($value);)+

            pub fn empty() -> Self {
                Self(0)
            }

            pub fn bits(&self) -> $type {
                self.0
            }

            pub fn from_bits(mut bits: $type) -> Self {
                let mut result = Self::empty();
                $(
                    if bits & $name::$flag.0 != 0 {
                        result.insert($name::$flag);
                        bits &= !$name::$flag.0;
                    }
                )+
                assert_eq!(
                    bits, 0,
                    "{}::from_bits(): unknown bits: {:b}",
                    stringify!($name),
                    bits,
                );
                result
            }

            pub fn from_bits_unchecked(bits: $type) -> Self {
                Self(bits)
            }

            pub fn is_empty(&self) -> bool {
                self.0 == 0
            }

            pub fn contains(&self, flags: $name) -> bool {
                (self.0 & flags.0) == flags.0
            }

            pub fn insert(&mut self, flags: $name) {
                self.0 |= flags.0;
            }

            pub fn remove(&mut self, flags: $name) {
                self.0 &= !flags.0;
            }

            pub fn toggle(&mut self, flags: $name) {
                self.0 ^= flags.0;
            }
        }

        #[allow(unused_assignments)]
        impl core::fmt::Debug for $name {
            fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
            {
                let mut first = true;
                $(
                    if self.0 & $name::$flag.0 != 0 {
                        if first {
                            first = false;
                        } else {
                            f.write_str(" | ")?;
                        }
                        f.write_str(stringify!($flag))?;
                    }
                )+
                Ok(())
            }
        }

        impl core::ops::BitAnd for $name {
            type Output = Self;

            fn bitand(self, rhs: Self) -> Self::Output {
                Self(self.0 & rhs.0)
            }
        }

        impl core::ops::BitOr for $name {
            type Output = Self;

            fn bitor(self, rhs: Self) -> Self::Output {
                Self(self.0 | rhs.0)
            }
        }

        impl core::ops::Not for $name {
            type Output = Self;

            fn not(mut self) -> Self::Output {
                $(
                    self.toggle($name::$flag);
                )+
                self
            }
        }
    }
}
