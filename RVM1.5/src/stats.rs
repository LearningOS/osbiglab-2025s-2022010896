#![allow(dead_code)]

#[cfg(feature = "stats")]
pub use _stats::*;

#[cfg(not(feature = "stats"))]
pub use _stats_empty::*;

mod _stats {
    use core::sync::atomic::{AtomicU64, Ordering};

    #[derive(Default)]
    pub struct StatsValue {
        count: AtomicU64,
        sum: AtomicU64,
    }

    impl StatsValue {
        pub fn new() -> Self {
            Self {
                count: AtomicU64::new(0),
                sum: AtomicU64::new(0),
            }
        }

        pub fn add(&mut self, value: u64) {
            *self.count.get_mut() += 1;
            *self.sum.get_mut() += value;
        }

        pub fn atomic_add(&self, value: u64) {
            self.count.fetch_add(1, Ordering::Release);
            self.sum.fetch_add(value, Ordering::Release);
        }

        pub fn as_string(&self) -> alloc::string::String {
            let sum = self.sum.load(Ordering::Acquire);
            let count = self.count.load(Ordering::Acquire);
            let ave = if count == 0 { 0 } else { sum * 1000 / count };
            format!(
                "count = {}, sum = {}, average = {}.{:03}",
                count,
                sum,
                ave / 1000,
                ave % 1000
            )
        }
    }

    pub struct Instant {
        timestamp: u64,
    }

    impl Instant {
        pub fn now() -> Self {
            Self {
                timestamp: crate::arch::cpu::current_cycle(),
            }
        }

        pub fn elapsed(&self) -> u64 {
            Self::now().timestamp - self.timestamp
        }
    }
}

mod _stats_empty {
    #[derive(Default)]
    pub struct StatsValue;
    impl StatsValue {
        pub fn new() -> Self {
            Self
        }
        pub fn add(&mut self, _value: u64) {}
        pub fn atomic_add(&self, _value: u64) {}
    }

    pub struct Instant;
    impl Instant {
        pub fn now() -> Self {
            Self
        }
        pub fn elapsed(&self) -> u64 {
            0
        }
    }
}

#[cfg(all(test, feature = "stats"))]
mod test {
    use super::*;
    #[test]
    fn test_stats() {
        let mut stats = StatsValue::new();
        let now = Instant::now();

        let n = 1_000_000_000;
        let mut a: u64;
        let mut b = 0;
        let mut c = 1;
        for _ in 1..n {
            a = b;
            b = c;
            c = a + b;
        }
        stats.add(now.elapsed());
        println!("fib_{} = {}", n, c);
        println!("stats: {}", stats.as_string());
        assert_eq!(c, 3311503426941990459);
    }
}
