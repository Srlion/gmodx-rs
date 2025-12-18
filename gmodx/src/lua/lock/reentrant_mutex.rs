use std::cell::Cell;
use std::marker::PhantomData;
use std::mem::ManuallyDrop;
use std::num::NonZeroUsize;
use std::sync::atomic::{AtomicUsize, Ordering};

use xutex::{Mutex, MutexGuard};

#[inline]
fn thread_id() -> usize {
    thread_local!(static KEY: u8 = 0);
    KEY.with(|x| NonZeroUsize::new(x as *const _ as usize).unwrap())
        .get()
}

pub struct ReentrantMutex {
    mutex: Mutex<()>,
    owner: AtomicUsize,
    count: Cell<usize>,
    waiters: AtomicUsize,
}

unsafe impl Send for ReentrantMutex {}
unsafe impl Sync for ReentrantMutex {}

impl ReentrantMutex {
    pub const fn new() -> Self {
        Self {
            mutex: Mutex::new(()),
            owner: AtomicUsize::new(0),
            count: Cell::new(0),
            waiters: AtomicUsize::new(0),
        }
    }

    #[inline]
    pub fn owner(&self) -> usize {
        self.owner.load(Ordering::Relaxed)
    }

    #[inline]
    pub fn has_waiters(&self) -> bool {
        self.waiters.load(Ordering::Relaxed) > 0
    }

    pub fn lock(&self) -> ReentrantMutexGuard<'_> {
        let tid = thread_id();

        if self.owner() == tid {
            self.count.set(self.count.get() + 1);
            return ReentrantMutexGuard {
                mutex: self,
                guard: None,
                _marker: PhantomData,
            };
        }

        self.waiters.fetch_add(1, Ordering::Relaxed);
        let guard = self.mutex.lock();
        self.waiters.fetch_sub(1, Ordering::Relaxed);

        self.owner.store(tid, Ordering::Relaxed);
        self.count.set(1);
        ReentrantMutexGuard {
            mutex: self,
            guard: Some(ManuallyDrop::new(guard)),
            _marker: PhantomData,
        }
    }

    pub async fn lock_async(&self) -> ReentrantMutexGuard<'_> {
        let tid = thread_id();

        if self.owner() == tid {
            self.count.set(self.count.get() + 1);
            return ReentrantMutexGuard {
                mutex: self,
                guard: None,
                _marker: PhantomData,
            };
        }

        self.waiters.fetch_add(1, Ordering::Relaxed);
        let guard = match self.mutex.try_lock() {
            Some(g) => g,
            None => self.mutex.lock_async().await,
        };
        self.waiters.fetch_sub(1, Ordering::Relaxed);

        // Get the current thread ID again in case it changed while awaiting
        // This could happen if the async runtime moved the task to a different thread
        // This should be safe because the lock cannot be held across await points
        let tid = thread_id();
        self.owner.store(tid, Ordering::Relaxed);
        self.count.set(1);
        ReentrantMutexGuard {
            mutex: self,
            guard: Some(ManuallyDrop::new(guard)),
            _marker: PhantomData,
        }
    }
}

pub struct ReentrantMutexGuard<'a> {
    mutex: &'a ReentrantMutex,
    guard: Option<ManuallyDrop<MutexGuard<'a, ()>>>,
    _marker: PhantomData<*const ()>,
}

impl ReentrantMutexGuard<'_> {
    /// Temporarily yields the mutex to a waiting thread if there is one.
    pub fn bump(&mut self) {
        if self.mutex.count.get() == 1 && self.mutex.has_waiters() {
            if let Some(guard) = self.guard.take() {
                let tid = self.mutex.owner();

                self.mutex.owner.store(0, Ordering::Relaxed);
                self.mutex.count.set(0);

                drop(ManuallyDrop::into_inner(guard));

                let new_guard = self.mutex.mutex.lock();

                self.mutex.owner.store(tid, Ordering::Relaxed);
                self.mutex.count.set(1);
                self.guard = Some(ManuallyDrop::new(new_guard));
            }
        }
    }
}

impl Drop for ReentrantMutexGuard<'_> {
    fn drop(&mut self) {
        let count = self.mutex.count.get() - 1;
        self.mutex.count.set(count);
        if count == 0 {
            self.mutex.owner.store(0, Ordering::Relaxed);
            if let Some(guard) = &mut self.guard {
                unsafe { ManuallyDrop::drop(guard) };
            }
        }
    }
}
