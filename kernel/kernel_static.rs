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

// The ideas of these structs and impls are based on spin 0.5.2 and lazy_static
// 1.4.0.  The former is licensed under the MIT License
// <https://raw.githubusercontent.com/mvdnes/spin-rs/master/LICENSE>; the latter
// is dual-licensed under the MIT License
// <https://raw.githubusercontent.com/rust-lang-nursery/lazy-static.rs/master/LICENSE-MIT>
// and the Apache License
// <https://raw.githubusercontent.com/rust-lang-nursery/lazy-static.rs/master/LICENSE-APACHE>.

use core::cell::UnsafeCell;
use core::hint::spin_loop;
use core::mem::MaybeUninit;
use core::ops::{Deref, DerefMut, Drop};
use core::sync::atomic::{AtomicBool, Ordering};

pub struct StaticCell<T> {
    initialized: AtomicBool,
    data: UnsafeCell<MaybeUninit<T>>,
}

unsafe impl<T: Sync> Sync for StaticCell<T> {}

impl<T> StaticCell<T> {
    pub const UNINITIALIZED: Self = StaticCell {
        initialized: AtomicBool::new(false),
        data: UnsafeCell::new(MaybeUninit::uninit()),
    };

    pub fn call_once<F>(&self, constructor: F) -> &T
    where
        F: FnOnce() -> T,
    {
        if !self.initialized.load(Ordering::SeqCst) {
            unsafe {
                (*self.data.get()).as_mut_ptr().write(constructor());
            }
            self.initialized.store(true, Ordering::SeqCst);
        }
        unsafe { &*(*self.data.get()).as_ptr() }
    }
}

pub struct Mutex<T> {
    locked: AtomicBool,
    data: UnsafeCell<T>,
}

impl<T> Mutex<T> {
    pub fn new(data: T) -> Self {
        Self {
            locked: AtomicBool::new(false),
            data: UnsafeCell::new(data),
        }
    }

    pub fn lock(&self) -> MutexWrapper<T> {
        while self
            .locked
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_err()
        {
            let mut color: u8 = 0;
            while self.locked.load(Ordering::Relaxed) {
                unsafe {
                    let ch_ptr = 0xB8000 as *mut u8;
                    let color_ptr = 0xB8001 as *mut u8;
                    *ch_ptr = 0x25; // %
                    *color_ptr = color;
                }
                spin_loop();
                color = match color {
                    _max if color == 255 => 0,
                    not_max => not_max + 1,
                };
            }
        }
        MutexWrapper {
            locked: &self.locked,
            data: unsafe { &mut *self.data.get() },
        }
    }

    pub fn try_lock(&self) -> Option<MutexWrapper<T>> {
        if self
            .locked
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_ok()
        {
            Some(MutexWrapper {
                locked: &self.locked,
                data: unsafe { &mut *self.data.get() },
            })
        } else {
            None
        }
    }
}

unsafe impl<T> Sync for Mutex<T> {}

pub struct MutexWrapper<'a, T: 'a> {
    locked: &'a AtomicBool,
    data: &'a mut T,
}

impl<'a, T> Deref for MutexWrapper<'a, T> {
    type Target = T;
    fn deref(&self) -> &T {
        &*self.data
    }
}

impl<'a, T> DerefMut for MutexWrapper<'a, T> {
    fn deref_mut(&mut self) -> &mut T {
        self.data
    }
}

impl<'a, T> Drop for MutexWrapper<'a, T> {
    fn drop(&mut self) {
        self.locked.store(false, Ordering::Release);
    }
}

macro_rules! kernel_static {
    (($($vis:tt)*) static ref $N:ident : $T:ty = $E:expr; $($t:tt)*) => {
        #[allow(non_camel_case_types)]
        $($vis)* struct $N {}
        $($vis)* static $N: $N = $N {};
        impl core::ops::Deref for $N {
            type Target = $T;
            fn deref(&self) -> &$T {
                static $N: $crate::kernel_static::StaticCell<$T> =
                    $crate::kernel_static::StaticCell::UNINITIALIZED;
                fn constructor() -> $T { $E }
                $N.call_once(constructor)
            }
        }
        kernel_static!($($t)*);
    };
    (static ref $N:ident : $T:ty = $E:expr; $($t:tt)*) => {
        kernel_static!(() static ref $N : $T = $E; $($t)*);
    };
    (pub static ref $N:ident : $T:ty = $E:expr; $($t:tt)*) => {
        kernel_static!((pub) static ref $N : $T = $E; $($t)*);
    };
    () => ()
}
