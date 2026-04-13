//! # Buddy System — 确定性身份生成
//!
//! 为每个用户生成一个稳定的「伙伴」身份，包括名称和表情符号。
//!
//! # 设计思想
//! 参考 reference 中的 Buddy System：
//! - 使用 hash(userId + SALT) 生成确定性种子
//! - 通过 Mulberry32 PRNG 从预设列表中选择名称/表情
//! - 同一用户每次启动都获得相同的 buddy，建立「个性化」感
//! - 不依赖外部状态，纯函数计算

use ring::digest;

// ---------------------------------------------------------------------------
// Mulberry32 PRNG
// ---------------------------------------------------------------------------

/// Mulberry32 伪随机数生成器
///
/// 为什么使用 Mulberry32 而不是更复杂的 PRNG：
/// - 只需要确定性的序列选择，不需要密码学安全
/// - 算法简单，单文件内可完整实现
/// - 与 reference 实现保持一致，便于对比验证
struct Mulberry32 {
    state: u32,
}

impl Mulberry32 {
    fn new(seed: u32) -> Self {
        Self { state: seed }
    }

    /// 生成下一个 0..1 之间的浮点数
    fn next_f64(&mut self) -> f64 {
        self.state = self.state.wrapping_add(0x6D2B_79F5);
        let mut t = self.state;
        t = (t ^ (t >> 15)).wrapping_mul(t | 1);
        t ^= t.wrapping_add((t ^ (t >> 7)).wrapping_mul(t | 61));
        let result = t ^ (t >> 14);
        (result >> 0) as f64 / u32::MAX as f64
    }

    /// 从切片中随机选择一个元素
    fn choose<'a, T>(&mut self, items: &'a [T]) -> &'a T {
        let idx = (self.next_f64() * items.len() as f64) as usize;
        // 防止浮点精度导致越界
        &items[idx.min(items.len() - 1)]
    }
}

// ---------------------------------------------------------------------------
// 预设名称和表情
// ---------------------------------------------------------------------------

/// 伙伴名称列表
///
/// 选用友好、中性、易记的名称
const BUDDY_NAMES: &[&str] = &[
    "Atlas", "Beacon", "Circuit", "Dash", "Echo", "Flux", "Glyph", "Helix", "Ion", "Jade", "Kite",
    "Link", "Mesh", "Nova", "Orbit", "Pixel", "Quartz", "Relay", "Spark", "Trace", "Unity", "Vox",
    "Wave", "Xenon", "Zenith",
];

/// 伙伴表情列表
const BUDDY_EMOJIS: &[&str] = &[
    "🦊", "🐙", "🦉", "🐬", "🦋", "🐢", "🦈", "🐝", "🦜", "🐺", "🦁", "🐧", "🦄", "🐸", "🦅", "🐨",
    "🦎", "🐳", "🦇", "🦀",
];

// ---------------------------------------------------------------------------
// BuddyIdentity
// ---------------------------------------------------------------------------

/// 伙伴身份
///
/// 包含从用户 ID 确定性生成的名称和表情。
/// 同一 user_id 始终产生相同结果。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BuddyIdentity {
    /// 伙伴名称
    pub name: String,
    /// 伙伴表情符号
    pub emoji: String,
}

impl BuddyIdentity {
    /// 从用户 ID 生成确定性的伙伴身份
    ///
    /// 使用 SHA-256(user_id + SALT) 的前 4 字节作为 Mulberry32 种子，
    /// 然后从预设列表中选择名称和表情。
    pub fn from_user_id(user_id: &str) -> Self {
        // 盐值确保即使 user_id 简单也能产生合理的哈希分布
        const SALT: &str = "chengcoding-buddy-v1";

        let input = format!("{}{}", user_id, SALT);
        let hash = digest::digest(&digest::SHA256, input.as_bytes());
        let bytes = hash.as_ref();

        // 取前 4 字节作为 u32 种子
        let seed = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
        let mut rng = Mulberry32::new(seed);

        let name = rng.choose(BUDDY_NAMES).to_string();
        let emoji = rng.choose(BUDDY_EMOJIS).to_string();

        Self { name, emoji }
    }

    /// 获取格式化的显示名（表情 + 名称）
    pub fn display(&self) -> String {
        format!("{} {}", self.emoji, self.name)
    }
}

impl std::fmt::Display for BuddyIdentity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} {}", self.emoji, self.name)
    }
}

// ===========================================================================
// 单元测试
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    /// 测试确定性：同一 user_id 始终生成相同 buddy
    #[test]
    fn test_deterministic() {
        let b1 = BuddyIdentity::from_user_id("user-123");
        let b2 = BuddyIdentity::from_user_id("user-123");
        assert_eq!(b1, b2, "同一 user_id 应生成相同 buddy");
    }

    /// 测试不同 user_id 生成不同 buddy（统计测试）
    #[test]
    fn test_different_users_different_buddies() {
        let users: Vec<String> = (0..100).map(|i| format!("user-{}", i)).collect();
        let buddies: Vec<BuddyIdentity> = users
            .iter()
            .map(|u| BuddyIdentity::from_user_id(u))
            .collect();

        // 至少应有多个不同的名称（100 个用户中不可能全部相同）
        let unique_names: std::collections::HashSet<&str> =
            buddies.iter().map(|b| b.name.as_str()).collect();
        assert!(
            unique_names.len() > 5,
            "100 个用户应产生多个不同名称，实际只有 {}",
            unique_names.len()
        );
    }

    /// 测试名称来自预设列表
    #[test]
    fn test_name_from_preset() {
        let buddy = BuddyIdentity::from_user_id("test-user");
        assert!(
            BUDDY_NAMES.contains(&buddy.name.as_str()),
            "名称 '{}' 不在预设列表中",
            buddy.name
        );
    }

    /// 测试表情来自预设列表
    #[test]
    fn test_emoji_from_preset() {
        let buddy = BuddyIdentity::from_user_id("test-user");
        assert!(
            BUDDY_EMOJIS.contains(&buddy.emoji.as_str()),
            "表情 '{}' 不在预设列表中",
            buddy.emoji
        );
    }

    /// 测试 Display 格式
    #[test]
    fn test_display_format() {
        let buddy = BuddyIdentity::from_user_id("test-user");
        let display = format!("{}", buddy);
        assert!(display.contains(&buddy.name));
        assert!(display.contains(&buddy.emoji));
        assert_eq!(display, buddy.display());
    }

    /// 测试空 user_id 不会 panic
    #[test]
    fn test_empty_user_id() {
        let buddy = BuddyIdentity::from_user_id("");
        assert!(!buddy.name.is_empty());
        assert!(!buddy.emoji.is_empty());
    }

    /// 测试特殊字符 user_id
    #[test]
    fn test_special_chars_user_id() {
        let buddy = BuddyIdentity::from_user_id("用户@#$%^&*()");
        assert!(!buddy.name.is_empty());
        assert!(!buddy.emoji.is_empty());
    }

    /// 测试 Mulberry32 的确定性
    #[test]
    fn test_mulberry32_deterministic() {
        let mut rng1 = Mulberry32::new(42);
        let mut rng2 = Mulberry32::new(42);

        for _ in 0..10 {
            assert_eq!(rng1.next_f64(), rng2.next_f64());
        }
    }

    /// 测试 Mulberry32 输出范围在 [0, 1)
    #[test]
    fn test_mulberry32_range() {
        let mut rng = Mulberry32::new(12345);
        for _ in 0..1000 {
            let v = rng.next_f64();
            assert!(v >= 0.0 && v <= 1.0, "值 {} 超出范围", v);
        }
    }

    /// 测试 choose 不会 panic
    #[test]
    fn test_mulberry32_choose() {
        let items = vec!["a", "b", "c"];
        let mut rng = Mulberry32::new(99);
        for _ in 0..100 {
            let chosen = rng.choose(&items);
            assert!(items.contains(chosen));
        }
    }
}
