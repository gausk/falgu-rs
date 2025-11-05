use std::collections::VecDeque;
use std::sync::{Arc, Condvar, Mutex};

pub struct Sender<T> {
    inner: Arc<Inner<T>>,
}

impl<T> Sender<T> {
    pub fn send(&self, value: T) {
        let mut queue = self.inner.queue.lock().unwrap();
        queue.push_back(value);
        self.inner.available.notify_one()
    }
}

pub struct Receiver<T> {
    inner: Arc<Inner<T>>,
}

impl<T> Receiver<T> {
    pub fn recv(&self) -> T {
        let mut queue = self.inner.queue.lock().unwrap();
        loop {
            match queue.pop_front() {
                Some(value) => return value,
                None => {
                    queue = self.inner.available.wait(queue).unwrap();
                }
            }
        }
    }
}

struct Inner<T> {
    queue: Mutex<VecDeque<T>>,
    available: Condvar,
}

pub fn channel<T>() -> (Sender<T>, Receiver<T>) {
    let inner = Arc::new(Inner {
        queue: Mutex::new(VecDeque::new()),
        available: Condvar::new(),
    });
    (
        Sender {
            inner: inner.clone(),
        },
        Receiver { inner },
    )
}
