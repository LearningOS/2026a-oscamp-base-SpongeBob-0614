//! # Select and Timeout
//!
//! In this exercise, you will use `tokio::select!` macro to implement race selection and timeout control.
//!
//! ## Concepts
//! - `tokio::select!` waits for multiple async operations simultaneously
//! - `tokio::time::timeout` timeout control
//! - The first completed branch is executed, others are cancelled

use std::future::Future;
use tokio::time::Duration;

/// Async operation with timeout.
/// If `future` completes within `timeout_ms` milliseconds, returns Some(result).
/// Otherwise returns None.
///
/// Hint: Use `tokio::select!` or `tokio::time::timeout`.
pub async fn with_timeout<F, T>(future: F, timeout_ms: u64) -> Option<T>
where
    F: Future<Output = T>,
{
    tokio::time::timeout(Duration::from_millis(timeout_ms), future)
        .await
        .ok()
}

/// Race two async tasks, return the result of whichever finishes first.
///
/// Hint: Use `tokio::select!` macro.
pub async fn race<F1, F2, T>(f1: F1, f2: F2) -> T
where
    F1: Future<Output = T>,
    F2: Future<Output = T>,
{
    tokio::select! {
        result = f1 => result,
        result = f2 => result,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::sleep;

    #[tokio::test]
    async fn test_timeout_success() {
        let result = with_timeout(async { 42 }, 100).await;
        assert_eq!(result, Some(42));
    }

    #[tokio::test]
    async fn test_timeout_expired() {
        let result = with_timeout(
            async {
                sleep(Duration::from_millis(200)).await;
                42
            },
            50,
        )
        .await;
        assert_eq!(result, None);
    }

    #[tokio::test]
    async fn test_race_first_wins() {
        let result = race(
            async {
                sleep(Duration::from_millis(10)).await;
                "fast"
            },
            async {
                sleep(Duration::from_millis(200)).await;
                "slow"
            },
        )
        .await;
        assert_eq!(result, "fast");
    }

    #[tokio::test]
    async fn test_race_second_wins() {
        let result = race(
            async {
                sleep(Duration::from_millis(200)).await;
                "slow"
            },
            async {
                sleep(Duration::from_millis(10)).await;
                "fast"
            },
        )
        .await;
        assert_eq!(result, "fast");
    }

    #[tokio::test]
    async fn test_timeout_cancels_pending_future() {
        struct MarkDropped(std::sync::Arc<std::sync::atomic::AtomicBool>);
        impl Drop for MarkDropped {
            fn drop(&mut self) {
                self.0.store(true, std::sync::atomic::Ordering::Relaxed);
            }
        }
        let dropped = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let marker = MarkDropped(std::sync::Arc::clone(&dropped));
        let future = async move {
            let _marker = marker;
            std::future::pending::<()>().await;
        };
        assert_eq!(with_timeout(future, 1).await, None);
        assert!(dropped.load(std::sync::atomic::Ordering::Relaxed));
    }

    #[tokio::test]
    async fn test_race_accepts_futures_that_are_not_unpin() {
        assert_eq!(race(std::future::pending(), async { 7 }).await, 7);
    }
}
