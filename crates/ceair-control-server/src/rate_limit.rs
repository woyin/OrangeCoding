use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// 简单的滑动窗口速率限制器
pub struct RateLimiter {
    /// 窗口大小
    window: Duration,
    /// 窗口内最大请求数
    max_requests: usize,
    /// 每个 key 的请求记录
    entries: Mutex<HashMap<String, Vec<Instant>>>,
}

/// 速率限制检查结果
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RateLimitResult {
    Allowed,
    Limited { retry_after_ms: u64 },
}

impl RateLimiter {
    pub fn new(window: Duration, max_requests: usize) -> Self {
        Self {
            window,
            max_requests,
            entries: Mutex::new(HashMap::new()),
        }
    }

    /// 检查并记录请求，返回是否允许
    pub fn check(&self, key: &str) -> RateLimitResult {
        let now = Instant::now();
        let mut entries = self.entries.lock().unwrap();
        let timestamps = entries.entry(key.to_string()).or_default();

        // 清除窗口外的旧记录
        timestamps.retain(|t| now.duration_since(*t) < self.window);

        if timestamps.len() < self.max_requests {
            timestamps.push(now);
            RateLimitResult::Allowed
        } else {
            // 计算最早记录过期的时间
            let oldest = timestamps[0];
            let elapsed = now.duration_since(oldest);
            let retry_after = self.window.saturating_sub(elapsed);
            RateLimitResult::Limited {
                retry_after_ms: retry_after.as_millis() as u64,
            }
        }
    }

    /// 返回窗口内剩余可用请求数
    pub fn remaining(&self, key: &str) -> usize {
        let now = Instant::now();
        let mut entries = self.entries.lock().unwrap();
        let timestamps = entries.entry(key.to_string()).or_default();
        timestamps.retain(|t| now.duration_since(*t) < self.window);
        self.max_requests.saturating_sub(timestamps.len())
    }

    /// 清除指定 key 的所有记录
    pub fn reset(&self, key: &str) {
        let mut entries = self.entries.lock().unwrap();
        entries.remove(key);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    #[test]
    fn allows_under_limit() {
        let limiter = RateLimiter::new(Duration::from_secs(60), 5);
        for _ in 0..5 {
            assert_eq!(limiter.check("user1"), RateLimitResult::Allowed);
        }
        assert_eq!(limiter.remaining("user1"), 0);
    }

    #[test]
    fn blocks_over_limit() {
        let limiter = RateLimiter::new(Duration::from_secs(60), 2);
        assert_eq!(limiter.check("user1"), RateLimitResult::Allowed);
        assert_eq!(limiter.check("user1"), RateLimitResult::Allowed);
        match limiter.check("user1") {
            RateLimitResult::Limited { retry_after_ms } => {
                assert!(retry_after_ms > 0);
            }
            _ => panic!("Expected Limited"),
        }
    }

    #[test]
    fn window_expiry_allows_again() {
        // 使用非常短的窗口
        let limiter = RateLimiter::new(Duration::from_millis(50), 1);
        assert_eq!(limiter.check("user1"), RateLimitResult::Allowed);
        assert!(matches!(
            limiter.check("user1"),
            RateLimitResult::Limited { .. }
        ));
        // 等待窗口过期
        thread::sleep(Duration::from_millis(60));
        assert_eq!(limiter.check("user1"), RateLimitResult::Allowed);
    }

    #[test]
    fn different_keys_independent() {
        let limiter = RateLimiter::new(Duration::from_secs(60), 1);
        assert_eq!(limiter.check("user1"), RateLimitResult::Allowed);
        assert_eq!(limiter.check("user2"), RateLimitResult::Allowed);
        // user1 被限制，user2 不受影响
        assert!(matches!(
            limiter.check("user1"),
            RateLimitResult::Limited { .. }
        ));
    }

    #[test]
    fn reset_clears_history() {
        let limiter = RateLimiter::new(Duration::from_secs(60), 1);
        assert_eq!(limiter.check("user1"), RateLimitResult::Allowed);
        assert!(matches!(
            limiter.check("user1"),
            RateLimitResult::Limited { .. }
        ));
        limiter.reset("user1");
        assert_eq!(limiter.check("user1"), RateLimitResult::Allowed);
    }
}
