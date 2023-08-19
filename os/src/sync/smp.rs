//! Safe Cell for multiprocessor
//!
//! UPSafeCell is used to wrap a static data structure which can access safely.
//!
//! TODO: the kernel can not support task preempting in kernel mode （or trap in kernel mode）.

use core::{cell::RefCell, sync::atomic::AtomicBool};

/// Use RAII to guard the SMPSafeCell
pub struct SMPSafeCellGuard<'a, T: 'a> {
    cell: &'a SMPSafeCell<T>,
}

#[allow(unused)]
/// SMPSafeCell, modified from UPSafeCell
pub struct SMPSafeCell<T> {
    /// inner data
    inner: RefCell<T>,
    lock: AtomicBool,
}

unsafe impl<T> Sync for SMPSafeCell<T> {}

impl<'a, T: 'a> Drop for SMPSafeCellGuard<'a, T> {
    fn drop(&mut self) {
        println!("D");
        self.cell.lock.store(false, core::sync::atomic::Ordering::Release);
    }
}

impl<T> SMPSafeCell<T> {
    /// User is responsible to guarantee that inner struct is only used in
    /// uniprocessor.
    pub unsafe fn new(value: T) -> Self {
        Self {
            inner: RefCell::new(value),
            lock: AtomicBool::new(false),
        }
    }
    /// Panic if the data has been borrowed.
    pub fn exclusive_access(&self) -> SMPSafeCellGuard<'_, T> {
        println!("A");
        while self.lock.compare_exchange(false, true, core::sync::atomic::Ordering::Acquire, core::sync::atomic::Ordering::Acquire).is_err() {}
        println!("G");
        SMPSafeCellGuard { cell: self }
    }
}
