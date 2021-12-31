//! Engine for doing CPU heavy work in the background.

use crate::workqueue::WorkQueue;
use futures::stream::{FuturesOrdered, StreamExt};
use tokio::select;
use tokio::sync::mpsc;

/// Do heavy work in the background.
///
/// An engine takes items of work from a work queue, and does the work
/// in the background, using `tokio` blocking tasks. The background
/// work can be CPU intensive or block on I/O. The number of active
/// concurrent tasks is limited to the size of the queue.
///
/// The actual work is done in a function or closure passed in as a
/// parameter to the engine. The worker function is called with a work
/// item as an argument, in a thread dedicated for that worker
/// function.
///
/// The need to move work items between threads puts some restrictions
/// on the types used as work items.
pub struct Engine<T> {
    rx: mpsc::Receiver<T>,
}

impl<T: Send + 'static> Engine<T> {
    /// Create a new engine.
    ///
    /// Each engine gets work from a queue, and calls the same worker
    /// function for each item of work. The results are put into
    /// another, internal queue.
    pub fn new<S, F>(queue: WorkQueue<S>, func: F) -> Self
    where
        F: Send + Copy + 'static + Fn(S) -> T,
        S: Send + 'static,
    {
        let size = queue.size();
        let (tx, rx) = mpsc::channel(size);
        tokio::spawn(manage_workers(queue, size, tx, func));
        Self { rx }
    }

    /// Get the oldest result of the worker function, if any.
    ///
    /// This will block until there is a result, or it's known that no
    /// more results will be forthcoming.
    pub async fn next(&mut self) -> Option<T> {
        self.rx.recv().await
    }
}

// This is a normal (non-blocking) background task that retrieves work
// items, launches blocking background tasks for work to be done, and
// waits on those tasks. Care is taken to not launch too many worker
// tasks.
async fn manage_workers<S, T, F>(
    mut queue: WorkQueue<S>,
    queue_size: usize,
    tx: mpsc::Sender<T>,
    func: F,
) where
    F: Send + 'static + Copy + Fn(S) -> T,
    S: Send + 'static,
    T: Send + 'static,
{
    let mut workers = FuturesOrdered::new();

    'processing: loop {
        // Wait for first of various concurrent things to finish.
        select! {
            biased;

            // Get work to be done.
            maybe_work = queue.next() => {
                if let Some(work) = maybe_work {
                    // We got a work item. Launch background task to
                    // work on it.
                    let tx = tx.clone();
                    workers.push(do_work(work, tx, func));

                    // If queue is full, wait for at least one
                    // background task to finish.
                    while workers.len() >= queue_size {
                        workers.next().await;
                    }
                } else {
                    // Finished with the input queue. Nothing more to do.
                    break 'processing;
                }
            }

            // Wait for background task to finish, if there are any
            // background tasks currently running.
            _ = workers.next(), if !workers.is_empty() => {
                // nothing to do here
            }
        }
    }

    while workers.next().await.is_some() {
        // Finish the remaining work items.
    }
}

// Work on a work item.
//
// This launches a `tokio` blocking background task, and waits for it
// to finish. The caller spawns a normal (non-blocking) async task for
// this function, so it's OK for this function to wait on the task it
// launches.
async fn do_work<S, T, F>(item: S, tx: mpsc::Sender<T>, func: F)
where
    F: Send + 'static + Fn(S) -> T,
    S: Send + 'static,
    T: Send + 'static,
{
    let result = tokio::task::spawn_blocking(move || func(item))
        .await
        .unwrap();
    if let Err(err) = tx.send(result).await {
        panic!("failed to send result to channel: {}", err);
    }
}
