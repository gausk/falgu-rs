use std::collections::VecDeque;
use std::sync::{Arc, Condvar, Mutex};

// Flavors:
//  - Synchronous channels: Channel where send() can block. Limited capacity.
//   - Mutex + Condvar + VecDeque
//   - Atomic VecDeque (atomic queue) + thread::park + thread::Thread::notify
//  - Asynchronous channels: Channel where send() cannot block. Unbounded.
//   - Mutex + Condvar + VecDeque
//   - Mutex + Condvar + LinkedList
//   - Atomic linked list, linked list of T
//   - Atomic block linked list, linked list of atomic VecDeque<T>
//  - Rendezvous channels: Synchronous with capacity = 0. Used for thread synchronization.
//  - Oneshot channels: Any capacity. In practice, only one call to send().

pub struct Sender<T> {
    inner: Arc<Inner<T>>,
}

impl<T> Clone for Sender<T> {
    fn clone(&self) -> Self {
        let mut shared = self.inner.shared.lock().unwrap();
        shared.senders += 1;
        drop(shared);
        Self {
            inner: Arc::clone(&self.inner),
        }
    }
}

impl<T> Drop for Sender<T> {
    fn drop(&mut self) {
        let mut shared = self.inner.shared.lock().unwrap();
        shared.senders -= 1;
        let is_last = shared.senders == 0;
        drop(shared);
        if is_last {
            self.inner.available.notify_one()
        }
    }
}

impl<T> Sender<T> {
    pub fn send(&self, value: T) {
        let mut shared = self.inner.shared.lock().unwrap();
        shared.queue.push_back(value);
        drop(shared);
        self.inner.available.notify_one()
    }
}

pub struct Receiver<T> {
    inner: Arc<Inner<T>>,
    buffer: VecDeque<T>,
}

impl<T> Receiver<T> {
    pub fn recv(&mut self) -> Option<T> {
        if let Some(value) = self.buffer.pop_front() {
            return Some(value);
        }
        let mut shared = self.inner.shared.lock().unwrap();
        loop {
            match shared.queue.pop_front() {
                Some(value) => {
                    std::mem::swap(&mut self.buffer, &mut shared.queue);
                    return Some(value);
                }
                None if shared.senders == 0 => return None,
                None => {
                    shared = self.inner.available.wait(shared).unwrap();
                }
            }
        }
    }
}

impl<T> Iterator for Receiver<T> {
    type Item = T;
    fn next(&mut self) -> Option<Self::Item> {
        self.recv()
    }
}

struct Inner<T> {
    shared: Mutex<Shared<T>>,
    available: Condvar,
}

struct Shared<T> {
    queue: VecDeque<T>,
    senders: usize,
}

pub fn channel<T>() -> (Sender<T>, Receiver<T>) {
    let inner = Arc::new(Inner {
        shared: Mutex::new({
            Shared {
                queue: VecDeque::default(),
                senders: 1,
            }
        }),
        available: Condvar::new(),
    });
    (
        Sender {
            inner: inner.clone(),
        },
        Receiver {
            inner,
            buffer: VecDeque::new(),
        },
    )
}

#[cfg(test)]
mod tests {
    use crate::channel;
    use std::thread;

    #[test]
    fn test_channel() {
        let (tx, mut rx) = channel();
        thread::spawn(move || {
            tx.send(10);
            tx.send(12);
            tx.send(13);
            tx.send(14);
        });
        assert_eq!(rx.recv(), Some(10));
        assert_eq!(rx.recv(), Some(12));
        assert_eq!(rx.recv(), Some(13));
        assert_eq!(rx.recv(), Some(14));
        assert_eq!(rx.recv(), None);
    }

    #[test]
    fn drop_receiver() {
        let (tx, rx) = channel();
        drop(rx);
        tx.send(1);
    }

    #[test]
    fn drop_sender() {
        let (tx, mut rx) = channel::<i32>();
        drop(tx);
        assert_eq!(rx.recv(), None);
    }

    #[test]
    fn recv_iterator() {
        let (tx, rx) = channel();
        tx.send(1);
        tx.send(2);
        drop(tx);
        for val in rx {
            assert!(val == 1 || val == 2);
        }
    }
}
