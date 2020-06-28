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

use core::marker::PhantomData;
use core::ops::{BitAnd, BitOr, BitOrAssign};

#[derive(Clone, Copy)]
pub struct BitFlags<T, E>
where
    T: BitOrAssign + BitAnd<Output = T>,
    E: Into<T>,
{
    pub value: T,
    phantom: PhantomData<E>,
}

impl<T, E> BitFlags<T, E>
where
    T: BitOrAssign + BitAnd<Output = T>,
    E: Into<T>,
{
    pub fn new(value: T) -> Self {
        Self {
            value,
            phantom: PhantomData,
        }
    }

    pub fn set_flag(&mut self, flag: E) {
        self.value |= flag.into();
    }
}

impl<T, E> BitOr<E> for BitFlags<T, E>
where
    T: BitOrAssign + BitAnd<Output = T>,
    E: Into<T>,
{
    type Output = BitFlags<T, E>;
    fn bitor(self, rhs: E) -> Self::Output {
        let mut res = self;
        res.set_flag(rhs);
        res
    }
}

impl<T, E> BitAnd<E> for BitFlags<T, E>
where
    T: BitOrAssign + BitAnd<Output = T>,
    E: Into<T>,
{
    type Output = BitFlags<T, E>;
    fn bitand(self, rhs: E) -> Self::Output {
        let mut res = self;
        res.value = res.value & rhs.into();
        res
    }
}

macro_rules! bitflags {
    (#[repr($R:ident)] ($($vis:tt)*) enum $N:ident { $($V:ident = $E:expr,)+ }) => {
        #[allow(dead_code)]
        #[derive(Clone, Copy)]
        #[repr($R)]
        $($vis)* enum $N {
            $($V = $E,)+
        }

        impl Into<$R> for $N {
            fn into(self) -> $R {
                self as $R
            }
        }
    };
    (#[repr($R:ident)] pub enum $N:ident { $($V:ident = $E:expr,)+ }) => {
        bitflags!(#[repr($R)] (pub) enum $N { $($V = $E,)+ });
    };
    (#[repr($R:ident)] enum $N:ident { $($V:ident = $E:expr,)+ }) => {
        bitflags!(#[repr($R)] () enum $N { $($V = $E,)+ });
    }
}
