use std::cmp::Ordering;
use std::collections::BinaryHeap;

use tokio::time::Instant;

#[derive(Debug)]
pub struct EventQueueItem<T> {
    time: Instant,
    item: T,
}
impl<T> PartialEq for EventQueueItem<T> {
    fn eq(&self, other: &Self) -> bool {
        self.time == other.time
    }
}

impl<T> Eq for EventQueueItem<T> {}

impl<T> PartialOrd for EventQueueItem<T> {
    /// Weird ordering, where first value is the one with earlier timestamp.
    /// This works because BinaryHeap is a max-heap.
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(other.time.cmp(&self.time))
    }
}

impl<T> Ord for EventQueueItem<T> {
    /// Weird ordering, where first value is the one with earlier timestamp.
    /// This works because BinaryHeap is a max-heap.
    fn cmp(&self, other: &Self) -> Ordering {
        other.time.partial_cmp(&self.time).unwrap()
    }
}

pub struct EventQueue<T> {
    queue: BinaryHeap<EventQueueItem<T>>,
}

impl<T> EventQueue<T> {
    pub fn new() -> Self {
        Self {
            queue: BinaryHeap::new(),
        }
    }

    pub fn add(&mut self, item: T, time: Instant) {
        self.queue.push(EventQueueItem { time, item });
    }

    pub fn next_timeout(&mut self) -> Option<Instant> {
        Some(self.queue.peek()?.time)
    }

    /// Pop a single, completed event, if any available.
    pub fn pop_completed(&mut self) -> Option<T> {
        let now = Instant::now();
        if let Some(item) = self.queue.peek() {
            if item.time <= now {
                return self.queue.pop().map(|item| item.item);
            }
        }
        None
    }
}
