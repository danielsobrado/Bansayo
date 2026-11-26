//! On-demand, bounded full-rate observation journal. The solver never waits on a client.
use super::inspection::Point;
use serde::Serialize;
use std::{
    collections::VecDeque,
    sync::atomic::{AtomicU64, Ordering},
    time::{Duration, Instant},
};

const CAPACITY: usize = 16_384;
const BATCH_LIMIT: usize = 4096;
const LEASE: Duration = Duration::from_secs(10);
static NEXT_EPOCH: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Clone, Serialize)]
pub struct Entry {
    cursor: u64,
    channel: &'static str,
    point: Point,
}

#[derive(Debug, Default, Serialize)]
pub struct Batch {
    pub(crate) epoch: u64,
    pub(crate) next: u64,
    pub(crate) lost: u64,
    pub(crate) reset: bool,
    pub(crate) entries: Vec<Entry>,
}

#[derive(Debug, Default)]
pub struct Stream {
    epoch: u64,
    next: u64,
    lease: Option<Instant>,
    entries: VecDeque<Entry>,
}

impl Stream {
    fn expired(&self) -> bool {
        self.lease.is_none_or(|lease| lease.elapsed() >= LEASE)
    }

    pub fn push(&mut self, channel: &'static str, point: &Point) {
        if self.expired() {
            self.entries.clear();
            return;
        }
        self.next += 1;
        if self.entries.len() == CAPACITY {
            self.entries.pop_front();
        }
        self.entries.push_back(Entry {
            cursor: self.next,
            channel,
            point: point.clone(),
        });
    }

    pub fn read(&mut self, epoch: Option<u64>, after: u64) -> Batch {
        if self.expired() {
            self.entries.clear();
            self.epoch = NEXT_EPOCH.fetch_add(1, Ordering::Relaxed);
            self.next = 0;
        }
        self.lease = Some(Instant::now());
        let reset = epoch.is_some_and(|e| e != self.epoch) || after > self.next;
        let after = if reset {
            0
        } else if epoch.is_none() {
            self.next
        } else {
            after
        };
        let oldest = self.entries.front().map_or(self.next + 1, |e| e.cursor);
        let lost = oldest.saturating_sub(after.saturating_add(1));
        let entries: Vec<_> = self
            .entries
            .iter()
            .filter(|e| e.cursor > after)
            .take(BATCH_LIMIT)
            .cloned()
            .collect();
        let next = entries.last().map_or(after, |e| e.cursor);
        Batch {
            epoch: self.epoch,
            next,
            lost,
            reset,
            entries,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lease_cursor_and_overrun() {
        let mut stream = Stream::default();
        let point: Point = serde_json::from_value(serde_json::json!({
            "t_us": 1, "seq": 1, "device_time_us": null, "values": [1.0, 2.0, 3.0]
        }))
        .unwrap();
        stream.push("gyro", &point);
        let start = stream.read(None, 0);
        assert!(start.entries.is_empty());
        for _ in 0..CAPACITY + 7 {
            stream.push("gyro", &point);
        }
        let batch = stream.read(Some(start.epoch), start.next);
        assert_eq!(batch.lost, 7);
        assert_eq!(batch.entries.len(), BATCH_LIMIT);
        assert_eq!(stream.read(Some(start.epoch), batch.next).lost, 0);
        assert!(stream.read(Some(start.epoch + 1), 0).reset);
    }
}
