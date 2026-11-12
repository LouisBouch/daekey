//! Defines a thread pool that drops work when full.

use std::{panic::{AssertUnwindSafe, catch_unwind}, thread::JoinHandle};

use crossbeam_channel::{Receiver, Sender, TrySendError};

type Job = Box<dyn FnOnce() + Send + 'static>;



/// A pool of threads that moves the cost of spawning threads at the creation of the pool.
pub struct ThreadPool {
    /// Channel to send jobs through.
    sender: Sender<Job>,
}
impl ThreadPool {
    pub fn new(number_threads: usize) -> Self {
        let (sender, receiver) = crossbeam_channel::bounded(0);
        for _ in 0..number_threads {
            let rx = receiver.clone();
            std::thread::spawn(move || {
                thread_loop(rx);
            });
        }
        ThreadPool { sender }
    }
    /// Use a thread from the pool to run a job. If none remain, drop the job.
    pub fn try_spawn<T>(&self, job: T) -> Result<(), TrySendError<Job>>
    where
        T: FnOnce() + Send + 'static,
    {
        self.sender.try_send(Box::new(job))
    }
}
/// The loop
fn thread_loop(receiver: Receiver<Job>) {
    loop {
        let Ok(job) = receiver.recv() else {
            return;
        };
        if let Err(e) = catch_unwind(AssertUnwindSafe(job)) {
            // 'e' is the payload of the job's panic.
            if let Err(e2) = catch_unwind(AssertUnwindSafe(|| drop(e))) {
                // 'e2' is the payload of the panic from the drop implementation of 'e'.
                // At that point, just leak it, a drop shouldn't panic.
                std::mem::forget(e2);
            }
        }
    }
}
