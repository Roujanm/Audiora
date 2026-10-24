//! Wait-free single-producer / single-consumer ring buffer of f32 samples.
//! Used to hand audio between the real-time thread and disk / IPC threads.

use std::cell::UnsafeCell;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

struct Shared {
    buf: Box<[UnsafeCell<f32>]>,
    mask: usize,
    /// Total samples ever written / read (wrapping); index = value & mask.
    head: AtomicUsize,
    tail: AtomicUsize,
}

// SAFETY: producer only writes slots in [tail+cap-free .. head), consumer only
// reads slots in [tail .. head); the Acquire/Release pairs on head/tail order
// those accesses, and each end is owned by exactly one thread (`&mut self`).
unsafe impl Sync for Shared {}
unsafe impl Send for Shared {}

pub struct Producer(Arc<Shared>);
pub struct Consumer(Arc<Shared>);

/// Capacity is rounded up to a power of two.
pub fn ring(capacity: usize) -> (Producer, Consumer) {
    let cap = capacity.max(2).next_power_of_two();
    let shared = Arc::new(Shared {
        buf: (0..cap).map(|_| UnsafeCell::new(0.0)).collect(),
        mask: cap - 1,
        head: AtomicUsize::new(0),
        tail: AtomicUsize::new(0),
    });
    (Producer(shared.clone()), Consumer(shared))
}

impl Producer {
    pub fn capacity(&self) -> usize {
        self.0.mask + 1
    }

    pub fn free(&self) -> usize {
        let head = self.0.head.load(Ordering::Relaxed);
        let tail = self.0.tail.load(Ordering::Acquire);
        self.capacity() - head.wrapping_sub(tail)
    }

    /// Writes as many samples as fit and returns how many were written.
    pub fn push_slice(&mut self, data: &[f32]) -> usize {
        let head = self.0.head.load(Ordering::Relaxed);
        let tail = self.0.tail.load(Ordering::Acquire);
        let free = self.capacity() - head.wrapping_sub(tail);
        let n = free.min(data.len());
        for (i, &s) in data[..n].iter().enumerate() {
            let slot = &self.0.buf[head.wrapping_add(i) & self.0.mask];
            // SAFETY: slot is outside the consumer's readable range.
            unsafe { *slot.get() = s };
        }
        self.0.head.store(head.wrapping_add(n), Ordering::Release);
        n
    }
}

impl Consumer {
    pub fn available(&self) -> usize {
        let tail = self.0.tail.load(Ordering::Relaxed);
        let head = self.0.head.load(Ordering::Acquire);
        head.wrapping_sub(tail)
    }

    /// Reads up to `out.len()` samples and returns how many were read.
    pub fn pop_slice(&mut self, out: &mut [f32]) -> usize {
        let tail = self.0.tail.load(Ordering::Relaxed);
        let head = self.0.head.load(Ordering::Acquire);
        let n = head.wrapping_sub(tail).min(out.len());
        for (i, o) in out[..n].iter_mut().enumerate() {
            let slot = &self.0.buf[tail.wrapping_add(i) & self.0.mask];
            // SAFETY: slot is inside [tail, head), published by the producer.
            *o = unsafe { *slot.get() };
        }
        self.0.tail.store(tail.wrapping_add(n), Ordering::Release);
        n
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fifo_order_and_wraparound() {
        let (mut p, mut c) = ring(8);
        let mut out = [0f32; 8];
        for round in 0..20 {
            let data: Vec<f32> = (0..5).map(|i| (round * 5 + i) as f32).collect();
            assert_eq!(p.push_slice(&data), 5);
            assert_eq!(c.pop_slice(&mut out[..5]), 5);
            assert_eq!(&out[..5], &data[..]);
        }
    }

    #[test]
    fn full_and_empty() {
        let (mut p, mut c) = ring(4);
        assert_eq!(p.push_slice(&[1.0; 10]), 4);
        assert_eq!(p.free(), 0);
        let mut out = [0f32; 10];
        assert_eq!(c.pop_slice(&mut out), 4);
        assert_eq!(c.pop_slice(&mut out), 0);
    }

    #[test]
    fn two_threads_no_loss_or_reorder() {
        let (mut p, mut c) = ring(1024);
        const TOTAL: usize = 200_000;
        let producer = std::thread::spawn(move || {
            let mut next = 0usize;
            while next < TOTAL {
                let end = (next + 100).min(TOTAL);
                let chunk: Vec<f32> = (next..end).map(|v| v as f32).collect();
                next += p.push_slice(&chunk);
                // push_slice may take a prefix; `next` advances by what was taken.
            }
        });
        let mut expect = 0usize;
        let mut buf = [0f32; 64];
        while expect < TOTAL {
            let n = c.pop_slice(&mut buf);
            for &v in &buf[..n] {
                assert_eq!(v, expect as f32);
                expect += 1;
            }
        }
        producer.join().unwrap();
    }
}
