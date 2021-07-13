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

use alloc::alloc::{alloc, dealloc, Layout};
use core::any::type_name;
use core::mem::{align_of, size_of};
use core::ops::Drop;

use crate::memory_region::Region;

pub struct Stack<T> {
    layout: Layout,
    max_top: *mut T,
    pub top: *mut T,
    pub bottom: *mut T,
}

impl<T> Stack<T> {
    /// Constructs a stack operating over the given memory region.
    ///
    /// # Safety
    /// The region must be valid to be written to and read from, i.e. mapped
    /// with suitable privileges.
    pub unsafe fn from_region(region: Region<usize>) -> Self {
        let layout = Layout::from_size_align(
            region.len(),
            2_usize.pow(region.start.trailing_zeros()),
        )
        .unwrap();
        assert_eq!(
            layout.align() % align_of::<T>(),
            0,
            "region align must be a multiple of align_of::<{}>() = {}",
            type_name::<T>(),
            align_of::<T>(),
        );
        assert_eq!(
            layout.size() % size_of::<T>(),
            0,
            "region length must be a multiple of size_of::<{}>() = {}",
            type_name::<T>(),
            size_of::<T>(),
        );
        Stack {
            layout,
            max_top: region.start as *mut T,
            top: region.end as *mut T,
            bottom: region.end as *mut T,
        }
    }

    pub fn with_layout(layout: Layout) -> Self {
        unsafe {
            let top = alloc(layout) as usize;
            let bottom = top + layout.size();
            Self::from_region(Region {
                start: top,
                end: bottom,
            })
        }
    }

    pub fn push(&mut self, elem: T) -> Result<(), PushErr> {
        unsafe {
            if self.top != self.max_top {
                self.top = self.top.sub(1);
                self.top.write(elem);
                Ok(())
            } else {
                Err(PushErr::Full)
            }
        }
    }

    pub fn pop(&mut self) -> Result<T, PopErr> {
        unsafe {
            if self.top != self.bottom {
                let elem = self.top.read();
                self.top = self.top.add(1);
                Ok(elem)
            } else {
                Err(PopErr::Empty)
            }
        }
    }
}

#[derive(Debug)]
pub enum PushErr {
    Full,
}

#[derive(Debug)]
pub enum PopErr {
    Empty,
}

impl<T> Drop for Stack<T> {
    fn drop(&mut self) {
        unsafe {
            dealloc(self.max_top.cast(), self.layout);
        }
    }
}
