//! Simple test utility to panic when spawned threads are blocked too long.

#![deny(missing_docs)]

use std::thread::{self, ThreadId, JoinHandle};
use std::sync::mpsc::{self, Sender, Receiver};
use std::time::Duration;
use std::cell::RefCell;
use std::collections::HashSet;
use std::ops::Drop;

/// Examples
///
/// ```
/// # extern crate shutter_gate;
/// # use shutter_gate::Shutter;
/// use std::thread;
/// use std::time::Duration;
///
/// #[test]
/// fn test_func() {
///   let shutter = Shutter::new();
///
///   shutter.spawn(|| println!("testing"));
///   shutter.spawn(|| thread::sleep(Duration::from_millis(300)));
///
///   shutter.timeout(Duration::from_millis(500));
/// }
/// ```
#[derive(Debug)]
pub struct Shutter {
    sender: Sender<ThreadId>,
    receiver: Receiver<ThreadId>,
    ids: RefCell<HashSet<ThreadId>>,
}

#[derive(Debug)]
struct Guard(Sender<ThreadId>);

impl Drop for Guard {
    fn drop(&mut self) {
        let id = thread::current().id();
        self.0.send(id).ok();
    }
}

impl Shutter {
    /// Create a shutter.
    pub fn new() -> Self {
        let (sender, receiver) = mpsc::channel();

        Shutter {
            sender,
            receiver,
            ids: RefCell::default(),
        }
    }

    /// Spawn a thread and track it.
    pub fn spawn<F, T>(&self, f: F) -> JoinHandle<T> where
        F: FnOnce() -> T,
        F: Send + 'static,
        T: Send + 'static,
    {
        let sender = self.sender.clone();

        let handle = thread::spawn(move|| {
            let _guard = Guard(sender);
            f()
        });

        assert!(self.ids.borrow_mut().insert(handle.thread().id()));

        handle
    }

    /// Ensure every spawned threads are not blocked after given duration.
    ///
    /// Returns early if every spawned threads are terminated.
    ///
    /// # Panics
    ///
    /// It panics after given duration if at least one thread spawned by this shutter is blocked.
    pub fn timeout(&self, dur: Duration) {
        let sender = self.sender.clone();

        let timer = thread::spawn(move|| {
            let _guard = Guard(sender);
            thread::park_timeout(dur);
        });
        let timer = timer.thread();

        for msg in self.receiver.iter() {
            let mut ids = self.ids.borrow_mut();

            if msg == timer.id() {
                for msg in self.receiver.try_iter() {
                    assert!(ids.remove(&msg));
                }

                if ids.is_empty() {
                    return
                } else {
                    panic!("Timeout")
                }
            }

            assert!(ids.remove(&msg));

            if ids.is_empty() {
                timer.unpark();
                return
            }
        }

        unreachable!()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ms(num: u64) -> Duration {
        Duration::from_millis(num)
    }

    #[test]
    fn test_success() {
        let shutter = Shutter::new();

        shutter.spawn(|| thread::sleep(ms(200)));
        shutter.spawn(|| thread::sleep(ms(300)));

        shutter.timeout(ms(500));
    }

    #[test]
    #[should_panic(expected = "Timeout")]
    fn test_timeout() {
        let shutter = Shutter::new();

        shutter.spawn(|| { thread::sleep(ms(500)); });
        shutter.timeout(ms(100));
    }

    #[test]
    fn test_child_panic() {
        let shutter = Shutter::new();

        shutter.spawn(|| { panic!("unbeleavable!"); });
        shutter.timeout(ms(100));
    }
}
