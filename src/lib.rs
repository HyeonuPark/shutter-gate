use std::thread::{self, ThreadId, JoinHandle};
use std::sync::mpsc::{self, Sender, Receiver};
use std::time::Duration;
use std::cell::RefCell;
use std::collections::HashSet;

#[derive(Debug)]
pub struct Shutter {
    sender: Sender<ThreadId>,
    receiver: Receiver<ThreadId>,
    ids: RefCell<HashSet<ThreadId>>,
}

impl Shutter {
    pub fn new() -> Self {
        let (sender, receiver) = mpsc::channel();

        Shutter {
            sender,
            receiver,
            ids: RefCell::default(),
        }
    }

    pub fn spawn<F, T>(&self, f: F) -> JoinHandle<T> where
        F: FnOnce() -> T,
        F: Send + 'static,
        T: Send + 'static,
    {
        let sender = self.sender.clone();

        let handle = thread::spawn(move|| {
            let res = f();

            let id = thread::current().id();
            sender.send(id).unwrap();

            res
        });

        self.ids.borrow_mut().insert(handle.thread().id());

        handle
    }

    pub fn timeout(&self, dur: Duration) {
        let sender = self.sender.clone();
        let mut ids = self.ids.borrow_mut();

        let timer = thread::spawn(move|| {
            thread::park_timeout(dur);

            let id = thread::current().id();
            sender.send(id).unwrap();
        });
        let timer = timer.thread();

        for id in self.receiver.iter() {
            if id == timer.id() {
                for id in self.receiver.try_iter() {
                    assert!(ids.remove(&id));
                }

                if !ids.is_empty() {
                    panic!("Timeout")
                }
            }

            assert!(ids.remove(&id));

            if ids.is_empty() {
                timer.unpark();
                return
            }
        }
    }
}
