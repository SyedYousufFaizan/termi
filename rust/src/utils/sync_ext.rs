//! Poison-safe synchronization helpers
//!
//! Part of the Phase 1b safety cleanup.
//!
//! `Mutex::lock().unwrap()` is a classic Rust footgun: if *any* thread ever
//! panics while holding the lock, the mutex becomes "poisoned" and every
//! future `.lock().unwrap()` on it panics too — including on a completely
//! unrelated thread. On the PTY read-timeout path, that thread is spawned
//! per read call, so one bad read on one thread could permanently brick a
//! shared buffer for the rest of the session, and because Rust panics can
//! unwind across `thread::spawn` boundaries, the failure mode is a silent
//! wedge rather than a clean error.
//!
//! `LockExt::lock_safe` recovers from poisoning instead of panicking: it
//! logs a warning (so the *original* panic doesn't go unnoticed) and hands
//! back the guard anyway. For the plain data buffers this crate protects
//! with mutexes (PTY scratch buffers, shared read results), the data itself
//! is still structurally valid even if a panic happened elsewhere while the
//! lock was held — there's no meaningful "invalid state" for a `Vec<u8>` or
//! an `Option<T>` to be poisoned into, so recovering is strictly safer than
//! propagating the panic further.
//!
//! Don't use this for mutexes protecting genuine invariants that a partial
//! write could violate — in that case, propagate a proper error instead of
//! recovering, since the recovered data could be structurally inconsistent.

use log::warn;
use std::sync::{Mutex, MutexGuard};

/// Extension trait providing a panic-free alternative to `Mutex::lock().unwrap()`.
pub trait LockExt<T> {
    /// Lock the mutex, recovering from poison instead of panicking.
    ///
    /// If the mutex was poisoned by a panic on another thread, this logs a
    /// warning and returns the guard anyway (the inner data is handed back
    /// as-is). Use this only for mutexes where a torn write can't leave the
    /// protected data in a semantically invalid state — see module docs.
    fn lock_safe(&self) -> MutexGuard<'_, T>;
}

impl<T> LockExt<T> for Mutex<T> {
    fn lock_safe(&self) -> MutexGuard<'_, T> {
        match self.lock() {
            Ok(guard) => guard,
            Err(poisoned) => {
                warn!(
                    "Mutex was poisoned by a panic on another thread; recovering data. \
                     This means something panicked elsewhere — check logs above for the \
                     original panic, since that's the real bug to fix."
                );
                poisoned.into_inner()
            }
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::thread;

    #[test]
    fn test_lock_safe_normal_path() {
        let m = Mutex::new(5);
        {
            let mut guard = m.lock_safe();
            *guard += 1;
        }
        assert_eq!(*m.lock_safe(), 6);
    }

    #[test]
    fn test_lock_safe_recovers_from_poison() {
        let m = Arc::new(Mutex::new(vec![1, 2, 3]));
        let m2 = m.clone();

        // Poison the mutex by panicking while holding the lock.
        let handle = thread::spawn(move || {
            let _guard = m2.lock().unwrap();
            panic!("intentional panic to poison the mutex for this test");
        });
        let _ = handle.join(); // join returns Err because the thread panicked; that's expected

        assert!(m.is_poisoned());

        // A plain lock().unwrap() would now panic on THIS thread too, even
        // though this thread did nothing wrong. lock_safe() must not.
        let guard = m.lock_safe();
        assert_eq!(*guard, vec![1, 2, 3]);
    }
}
