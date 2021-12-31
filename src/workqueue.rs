//! A queue of work for [`crate::engine::Engine`].

use tokio::sync::mpsc;

/// A queue of work items.
///
/// An abstraction for producing items of work. For example, chunks of
/// data in a file. The work items are put into an ordered queue to be
/// worked on by another task. The queue is limited in size so that it
/// doesn't grow impossibly large. This acts as a load-limiting
/// synchronizing mechanism.
///
/// One async task produces work items and puts them into the queue,
/// another consumes them from the queue. If the producer is too fast,
/// the queue fills up, and the producer blocks when putting an item
/// into the queue. If the queue is empty, the consumer blocks until
/// there is something added to the queue.
///
/// The work items need to be abstracted as a type, and that type is
/// given as a type parameter.
pub struct WorkQueue<T> {
    rx: mpsc::Receiver<T>,
    tx: Option<mpsc::Sender<T>>,
    size: usize,
}

impl<T> WorkQueue<T> {
    /// Create a new work queue of a given maximum size.
    pub fn new(queue_size: usize) -> Self {
        let (tx, rx) = mpsc::channel(queue_size);
        Self {
            rx,
            tx: Some(tx),
            size: queue_size,
        }
    }

    /// Get maximum size of queue.
    pub fn size(&self) -> usize {
        self.size
    }

    /// Add an item of work to the queue.
    pub fn push(&self) -> mpsc::Sender<T> {
        self.tx.as_ref().unwrap().clone()
    }

    /// Signal that no more work items will be added to the queue.
    ///
    /// You **must** call this, as otherwise the `next` function will
    /// wait indefinitely.
    pub fn close(&mut self) {
        // println!("Chunkify::close closing sender");
        self.tx = None;
    }

    /// Get the oldest work item from the queue, if any.
    pub async fn next(&mut self) -> Option<T> {
        // println!("next called");
        self.rx.recv().await
    }
}
