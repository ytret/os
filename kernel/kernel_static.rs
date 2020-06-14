use core::cell::Cell;
use core::mem::MaybeUninit;
use core::sync::atomic::{AtomicBool, Ordering};

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
