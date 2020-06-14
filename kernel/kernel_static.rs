use core::cell::Cell;
use core::mem::MaybeUninit;
use core::ops::{Deref, DerefMut, Drop};
use core::sync::atomic::{spin_loop_hint, AtomicBool, Ordering};

pub struct StaticCell<T> {
    initialized: AtomicBool, // NB: the kernel is one-threaded
    data: Cell<MaybeUninit<T>>,
}

unsafe impl<T: Sync> Sync for StaticCell<T> {}

impl<T> StaticCell<T> {
    pub const UNINITIALIZED: Self = StaticCell {
        initialized: AtomicBool::new(false),
        data: Cell::new(MaybeUninit::uninit()),
    };

    pub fn call_once<F>(&self, constructor: F) -> &T
    where
        F: FnOnce() -> T,
    {
        if !self.initialized.load(Ordering::Relaxed) {
            unsafe {
                *(*self.data.as_ptr()).as_mut_ptr() = constructor();
            }
            self.initialized.store(true, Ordering::Relaxed);
        }
        unsafe { &*(*self.data.as_ptr()).as_ptr() }
    }
}

pub struct Mutex<T> {
    locked: AtomicBool,
    data: Cell<T>,
}

pub struct MutexWrapper<'a, T: 'a> {
    locked: &'a AtomicBool,
    data: &'a mut T,
}

impl<T> Mutex<T> {
    pub fn new(data: T) -> Self {
        Self {
            locked: AtomicBool::new(false),
            data: Cell::new(data),
        }
    }

    pub fn lock(&self) -> MutexWrapper<T> {
        while self.locked.compare_and_swap(false, true, Ordering::Relaxed)
            != false
        {
            while self.locked.load(Ordering::Relaxed) {
                spin_loop_hint();
            }
        }
        MutexWrapper {
            locked: &self.locked,
            data: unsafe { &mut *self.data.as_ptr() },
        }
    }
}

unsafe impl<T> Sync for Mutex<T> {}

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
        self.locked.store(false, Ordering::Relaxed);
    }
}

macro_rules! kernel_static {
    (($($vis:tt)*) static ref $N:ident : $T:ty = $E:expr; $($t:tt)*) => {
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
