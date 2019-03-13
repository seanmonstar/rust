#![unstable(feature = "futures_api",
            reason = "futures in libcore are unstable",
            issue = "50547")]

use fmt;
use marker::Unpin;

/// A `RawWake` allows the implementor of a task executor to create a [`Waker`]
/// which provides customized wakeup behavior.
pub unsafe trait RawWake: Send + Sync {
    /// This function will be called when the [`RawWake`] gets cloned, e.g. when
    /// the [`Waker`] in which the [`RawWake`] is stored gets cloned.
    ///
    /// The implementation of this function must retain all resources that are
    /// required for this additional instance of a [`RawWake`] and associated
    /// task. Calling `wake` on the resulting [`RawWake`] should result in a wakeup
    /// of the same task that would have been awoken by the original [`RawWake`].
    unsafe fn clone(&self) -> Waker;

    /// This function will be called when `wake` is called on the [`Waker`].
    /// It must wake up the task associated with this [`RawWake`].
    ///
    /// The implemention of this function must not consume the provided data
    /// pointer.
    unsafe fn wake(&self);

    /// This function gets called when a [`RawWake`] gets dropped.
    ///
    /// The implementation of this function must make sure to release any
    /// resources that are associated with this instance of a [`RawWake`] and
    /// associated task.
    unsafe fn drop(&self);
}

/// A `Waker` is a handle for waking up a task by notifying its executor that it
/// is ready to be run.
///
/// This handle encapsulates a [`RawWake`] instance, which defines the
/// executor-specific wakeup behavior.
///
/// Implements [`Clone`], [`Send`], and [`Sync`].
#[repr(transparent)]
pub struct Waker {
    waker: *const RawWake,
}

impl Unpin for Waker {}
unsafe impl Send for Waker {}
unsafe impl Sync for Waker {}

impl Waker {
    /// Wake up the task associated with this `Waker`.
    pub fn wake(&self) {
        // The actual wakeup call is delegated through a virtual function call
        // to the implementation which is defined by the executor.

        // SAFETY: This is safe because `Waker::new_unchecked` is the only way
        // to initialize `wake` and `data` requiring the user to acknowledge
        // that the contract of `RawWake` is upheld.
        unsafe { (*self.waker).wake() }
    }

    /// Returns whether or not this `Waker` and other `Waker` have awaken the same task.
    ///
    /// This function works on a best-effort basis, and may return false even
    /// when the `Waker`s would awaken the same task. However, if this function
    /// returns `true`, it is guaranteed that the `Waker`s will awaken the same task.
    ///
    /// This function is primarily used for optimization purposes.
    pub fn will_wake(&self, other: &Waker) -> bool {
        self.waker == other.waker
    }

    /// Creates a new `Waker` from [`RawWake`].
    ///
    /// The behavior of the returned `Waker` is undefined if the contract defined
    /// in [`RawWake`]'s and [`RawWake`]'s documentation is not upheld.
    /// Therefore this method is unsafe.
    pub unsafe fn new_unchecked(waker: *const RawWake) -> Waker {
        Waker {
            waker,
        }
    }
}

impl Clone for Waker {
    fn clone(&self) -> Self {
        // SAFETY: This is safe because `Waker::new_unchecked` is the only way
        // to initialize `clone` and `data` requiring the user to acknowledge
        // that the contract of [`RawWake`] is upheld.
        unsafe { (*self.waker).clone() }
    }
}

impl Drop for Waker {
    fn drop(&mut self) {
        // SAFETY: This is safe because `Waker::new_unchecked` is the only way
        // to initialize `drop` and `data` requiring the user to acknowledge
        // that the contract of `RawWake` is upheld.
        unsafe { (*self.waker).drop() }
    }
}

impl fmt::Debug for Waker {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("Waker")
            .field("obj", &self.waker)
            .finish()
    }
}
