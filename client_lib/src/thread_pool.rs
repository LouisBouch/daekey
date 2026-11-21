//! Defines a thread pool that drops work when full.

use std::{
    error::Error,
    fmt::{Debug, Display},
    io,
    panic::{AssertUnwindSafe, catch_unwind},
};

use crossbeam_channel::{Receiver, Sender, TrySendError};

type Job<T> = Box<dyn FnOnce(&mut T) + Send + 'static>;

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

/// Failed to start a new job, all threads are currently busy.
#[derive(Debug)]
pub struct NoAvailableThread;
impl Display for NoAvailableThread {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Failed to start job, no threads are available")
    }
}
impl Error for NoAvailableThread {}

/// A pool of threads that moves the cost of spawning threads at the creation of the pool.
pub struct ThreadPool<T> {
    /// Channel to send jobs through.
    sender: Sender<Job<T>>,
}
impl<T: Send + 'static> ThreadPool<T> {
    /// Try to creata new thread pool.
    ///
    /// # Paramters
    ///
    /// * `number_threads` - How many threads to park in the pool.
    /// * `ctx_maker` - Creates context per thread that will be given to the running job.
    ///                 There will be as many context instances as there are threads.
    pub fn try_new(
        number_threads: usize,
        mut ctx_maker: impl FnMut() -> T,
    ) -> Result<Self, ThreadPoolBuildError> {
        if number_threads == 0 {
            return Err(ThreadPoolBuildError::ZeroThreads);
        }
        let (sender, receiver) = crossbeam_channel::bounded(0);
        for i in 0..number_threads {
            let rx = receiver.clone();
            let context = ctx_maker();
            std::thread::Builder::new()
                .name(format!("pool-thread-{i}"))
                .spawn(move || {
                    Self::thread_loop(rx, context);
                })
                .map_err(ThreadPoolBuildError::ThreadSpawn)?;
        }
        Ok(ThreadPool { sender })
    }
    /// Use a thread from the pool to run a job. If none remain, drop the job.
    ///
    /// Can fail if no thread is ready to take a job.
    pub fn try_spawn<J>(&self, job: J) -> Result<(), NoAvailableThread>
    where
        J: FnOnce(&mut T) + Send + 'static,
    {
        self.sender.try_send(Box::new(job)).map_err(|e| match e {
            TrySendError::Full(_) => NoAvailableThread,
            TrySendError::Disconnected(_) => unreachable!("worker pool threads died"),
        })
    }
    /// The loop
    fn thread_loop(receiver: Receiver<Job<T>>, mut context: T) {
        loop {
            let Ok(job) = receiver.recv() else {
                return;
            };
            if let Err(e) = catch_unwind(AssertUnwindSafe(|| job(&mut context))) {
                // 'e' is the payload of the job's panic.
                if let Err(e2) = catch_unwind(AssertUnwindSafe(|| drop(e))) {
                    // 'e2' is the payload of the panic from the drop implementation of 'e'.
                    // At that point, just leak it, a drop shouldn't panic.
                    std::mem::forget(e2);
                }
            }
        }
    }
}
