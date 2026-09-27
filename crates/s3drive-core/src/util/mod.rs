pub mod key;
pub mod region;
pub mod time;

/// 指数バックオフの待ち時間（初回 1 秒、上限 30 秒、ジッターあり。01 §7.2）。
pub fn backoff_delay(attempt: u32) -> std::time::Duration {
    use rand::Rng;
    let base_ms = 1000u64.saturating_mul(1u64 << attempt.min(5));
    let capped = base_ms.min(30_000);
    let jitter = rand::rng().random_range(0..=capped / 2);
    std::time::Duration::from_millis(capped / 2 + jitter)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backoff_grows_and_is_capped() {
        for attempt in 0..10 {
            let d = backoff_delay(attempt).as_millis() as u64;
            let cap = (1000u64 << attempt.min(5)).min(30_000);
            assert!(d >= cap / 2 && d <= cap, "attempt {attempt}: {d}");
        }
    }
}
