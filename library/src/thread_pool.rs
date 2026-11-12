//! Defines a thread pool that drops work when full.

use std::{
    error::Error,
    fmt::Display,
    io,
    panic::{AssertUnwindSafe, catch_unwind},
};

use crossbeam_channel::{Receiver, Sender, TrySendError};

type Job = Box<dyn FnOnce() + Send + 'static>;

/// Errors when building a new [`ThreadPool`].
#[derive(Debug)]
pub enum ThreadPoolBuildError {
    /// Invalid number of threads, has to be more than 0.
    ZeroThreads,
    /// Failed to spawn a thread.
    ThreadSpawn(io::Error),
}
impl Display for ThreadPoolBuildError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ThreadPoolBuildError::ZeroThreads => {
                write!(f, "cannot build a thread pool with zero threads in it")
            }
            ThreadPoolBuildError::ThreadSpawn(_) => {
                write!(f, "failed to spawn a thread.")
            }
        }
    }
}
impl Error for ThreadPoolBuildError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            ThreadPoolBuildError::ZeroThreads => None,
            ThreadPoolBuildError::ThreadSpawn(error) => Some(error),
        }
    }
}

/// A pool of threads that moves the cost of spawning threads at the creation of the pool.
pub struct ThreadPool {
    /// Channel to send jobs through.
    sender: Sender<Job>,
}
impl ThreadPool {
    pub fn try_new(thread_number: usize) -> Result<Self, ThreadPoolBuildError> {
        if thread_number == 0 {
            return Err(ThreadPoolBuildError::ZeroThreads);
        }
        let (sender, receiver) = crossbeam_channel::bounded(0);
        for _ in 0..thread_number {
            let rx = receiver.clone();
            std::thread::Builder::new()
                .spawn(move || {
                    thread_loop(rx);
                })
                .map_err(ThreadPoolBuildError::ThreadSpawn)?;
        }
        Ok(ThreadPool { sender })
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
