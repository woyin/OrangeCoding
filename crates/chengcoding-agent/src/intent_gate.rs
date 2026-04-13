//! # 意图门控模块
//!
//! 在 Agent 执行用户请求之前，先对用户的真实意图进行分类。
//! 意图分类结果用于指导后续的 Category 选择和 Agent 路由。
//!
//! ## 意图类型
//!
//! | 意图 | 描述 | 推荐类别 |
//! |------|------|---------|
//! | Research | 信息搜集、调研 | deep |
//! | Implementation | 功能实现 | unspecified-high |
//! | Fix | Bug修复 | quick / unspecified-low |
//! | Investigation | 深度调查 | ultrabrain |
//! | Refactor | 代码重构 | deep |
//! | Planning | 规划设计 | unspecified-high |
//! | QuickFix | 快速修复 | quick |

use std::fmt;

// ============================================================
// IntentKind — 意图类型枚举
// ============================================================

/// 用户请求的意图分类。
///
/// Agent 通过分析用户的输入文本，判断其真实意图类别，
/// 然后据此选择最合适的处理策略和模型。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum IntentKind {
    /// 信息搜集、调研分析
    Research,
    /// 功能实现、代码编写
    Implementation,
    /// Bug 修复
    Fix,
    /// 深度调查、根因分析
    Investigation,
    /// 代码重构
    Refactor,
    /// 规划设计、架构设计
    Planning,
    /// 快速简单修复
    QuickFix,
}

impl IntentKind {
    /// 返回该意图推荐的默认 Category 名称
    pub fn recommended_category(&self) -> &'static str {
        match self {
            Self::Research => "deep",
            Self::Implementation => "unspecified-high",
            Self::Fix => "unspecified-low",
            Self::Investigation => "ultrabrain",
            Self::Refactor => "deep",
            Self::Planning => "unspecified-high",
            Self::QuickFix => "quick",
        }
    }

    /// 该意图是否需要深度推理
    pub fn requires_deep_thinking(&self) -> bool {
        matches!(self, Self::Investigation | Self::Refactor | Self::Planning)
    }
}

impl fmt::Display for IntentKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Research => write!(f, "research"),
            Self::Implementation => write!(f, "implementation"),
            Self::Fix => write!(f, "fix"),
            Self::Investigation => write!(f, "investigation"),
            Self::Refactor => write!(f, "refactor"),
            Self::Planning => write!(f, "planning"),
            Self::QuickFix => write!(f, "quick-fix"),
        }
    }
}

// ============================================================
// ClassifiedIntent — 分类结果
// ============================================================

/// 意图分类结果，包含分类的意图和置信度。
#[derive(Debug, Clone)]
pub struct ClassifiedIntent {
    /// 分类得到的意图类型
    pub kind: IntentKind,
    /// 分类置信度（0.0-1.0）
    pub confidence: f32,
    /// 推荐的 Category 名称
    pub recommended_category: String,
    /// 是否检测到关键词触发（如 "ultrawork", "ulw"）
    pub keyword_triggered: bool,
    /// 触发的关键词（如有）
    pub trigger_keyword: Option<String>,
}

// ============================================================
// IntentGate — 意图分类器
// ============================================================

/// 意图门控——在执行前分析用户请求的真实意图。
///
/// 使用关键词匹配和上下文分析来分类用户的请求，
/// 确保 Agent 理解的不只是字面意思，而是用户的真实目标。
pub struct IntentGate {
    /// 实现关键词 → 意图类型的映射规则
    keyword_rules: Vec<KeywordRule>,
}

/// 关键词匹配规则
struct KeywordRule {
    /// 关键词列表（匹配任一即触发）
    keywords: Vec<&'static str>,
    /// 匹配后的意图类型
    intent: IntentKind,
    /// 该规则的权重（高权重优先）
    weight: u8,
}

impl IntentGate {
    /// 创建新的意图分类器，加载所有关键词规则
    pub fn new() -> Self {
        Self {
            keyword_rules: Self::build_keyword_rules(),
        }
    }

    /// 分类用户输入文本的意图
    ///
    /// 分析流程：
    /// 1. 检查特殊关键词触发（ultrawork/ulw/search 等）
    /// 2. 基于关键词匹配计算每种意图的得分
    /// 3. 选择得分最高的意图
    /// 4. 如果无明确匹配，回退到 Implementation
    pub fn classify(&self, input: &str) -> ClassifiedIntent {
        let lower = input.to_lowercase();

        // 第一步：检查特殊关键词触发
        if let Some(triggered) = self.check_special_keywords(&lower) {
            return triggered;
        }

        // 第二步：基于规则匹配计算得分
        let mut scores: Vec<(IntentKind, u32)> = Vec::new();

        for rule in &self.keyword_rules {
            let match_count: u32 = rule
                .keywords
                .iter()
                .filter(|kw| lower.contains(**kw))
                .count() as u32;

            if match_count > 0 {
                let score = match_count * rule.weight as u32;
                scores.push((rule.intent, score));
            }
        }

        // 第三步：选择得分最高的意图
        if let Some((kind, score)) = scores.iter().max_by_key(|(_, s)| *s) {
            let max_possible = 10u32; // 粗略最大值
            let confidence = ((*score as f32) / max_possible as f32).min(1.0);
            return ClassifiedIntent {
                kind: *kind,
                confidence,
                recommended_category: kind.recommended_category().to_string(),
                keyword_triggered: false,
                trigger_keyword: None,
            };
        }

        // 第四步：无明确匹配——回退到 Implementation
        ClassifiedIntent {
            kind: IntentKind::Implementation,
            confidence: 0.3,
            recommended_category: IntentKind::Implementation
                .recommended_category()
                .to_string(),
            keyword_triggered: false,
            trigger_keyword: None,
        }
    }

    /// 检查特殊关键词触发（如 ultrawork、search 等）
    fn check_special_keywords(&self, lower: &str) -> Option<ClassifiedIntent> {
        // ultrawork / ulw 触发——最高优先级
        if lower.starts_with("ultrawork") || lower.starts_with("ulw ") || lower == "ulw" {
            return Some(ClassifiedIntent {
                kind: IntentKind::Implementation,
                confidence: 1.0,
                recommended_category: "unspecified-high".to_string(),
                keyword_triggered: true,
                trigger_keyword: Some("ultrawork".to_string()),
            });
        }

        // search / find 触发——并行探索
        if lower.starts_with("search ") || lower.starts_with("find ") {
            return Some(ClassifiedIntent {
                kind: IntentKind::Research,
                confidence: 0.9,
                recommended_category: "deep".to_string(),
                keyword_triggered: true,
                trigger_keyword: Some("search".to_string()),
            });
        }

        // analyze / investigate 触发——深度分析
        if lower.starts_with("analyze ") || lower.starts_with("investigate ") {
            return Some(ClassifiedIntent {
                kind: IntentKind::Investigation,
                confidence: 0.9,
                recommended_category: "ultrabrain".to_string(),
                keyword_triggered: true,
                trigger_keyword: Some("analyze".to_string()),
            });
        }

        None
    }

    /// 构建关键词匹配规则表
    fn build_keyword_rules() -> Vec<KeywordRule> {
        vec![
            // 研究类关键词
            KeywordRule {
                keywords: vec![
                    "research",
                    "explore",
                    "look into",
                    "study",
                    "what is",
                    "how does",
                    "explain",
                    "describe",
                    "调研",
                    "研究",
                    "探索",
                    "了解",
                ],
                intent: IntentKind::Research,
                weight: 3,
            },
            // 实现类关键词
            KeywordRule {
                keywords: vec![
                    "build",
                    "create",
                    "implement",
                    "add",
                    "develop",
                    "make",
                    "write",
                    "code",
                    "construct",
                    "实现",
                    "创建",
                    "构建",
                    "开发",
                    "编写",
                ],
                intent: IntentKind::Implementation,
                weight: 3,
            },
            // 修复类关键词
            KeywordRule {
                keywords: vec![
                    "fix",
                    "bug",
                    "broken",
                    "not working",
                    "error",
                    "crash",
                    "issue",
                    "problem",
                    "wrong",
                    "修复",
                    "修正",
                    "错误",
                    "问题",
                    "故障",
                ],
                intent: IntentKind::Fix,
                weight: 4,
            },
            // 调查类关键词
            KeywordRule {
                keywords: vec![
                    "investigate",
                    "debug",
                    "trace",
                    "root cause",
                    "why does",
                    "how come",
                    "analyze",
                    "调查",
                    "排查",
                    "分析",
                    "根因",
                ],
                intent: IntentKind::Investigation,
                weight: 5,
            },
            // 重构类关键词
            KeywordRule {
                keywords: vec![
                    "refactor",
                    "restructure",
                    "reorganize",
                    "clean up",
                    "simplify",
                    "optimize",
                    "improve",
                    "重构",
                    "优化",
                    "简化",
                    "整理",
                ],
                intent: IntentKind::Refactor,
                weight: 4,
            },
            // 规划类关键词
            KeywordRule {
                keywords: vec![
                    "plan",
                    "design",
                    "architect",
                    "strategy",
                    "propose",
                    "blueprint",
                    "roadmap",
                    "规划",
                    "设计",
                    "架构",
                    "策略",
                ],
                intent: IntentKind::Planning,
                weight: 4,
            },
            // 快速修复关键词（高权重，短文本优先）
            KeywordRule {
                keywords: vec![
                    "typo",
                    "rename",
                    "quick",
                    "simple",
                    "just change",
                    "just update",
                    "trivial",
                    "简单",
                    "改个",
                    "快速",
                ],
                intent: IntentKind::QuickFix,
                weight: 5,
            },
        ]
    }
}

impl Default for IntentGate {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================
// 单元测试
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;

    /// 测试实现类意图分类
    #[test]
    fn test_classify_implementation() {
        let gate = IntentGate::new();
        let intent = gate.classify("Build a REST API with JWT auth");
        assert_eq!(intent.kind, IntentKind::Implementation);
    }

    /// 测试修复类意图分类
    #[test]
    fn test_classify_fix() {
        let gate = IntentGate::new();
        let intent = gate.classify("Fix the login button not responding");
        assert_eq!(intent.kind, IntentKind::Fix);
    }

    /// 测试研究类意图分类
    #[test]
    fn test_classify_research() {
        let gate = IntentGate::new();
        let intent = gate.classify("Research how does the auth system work");
        assert_eq!(intent.kind, IntentKind::Research);
    }

    /// 测试调查类意图分类
    #[test]
    fn test_classify_investigation() {
        let gate = IntentGate::new();
        let intent = gate.classify("Investigate why the test fails intermittently");
        assert_eq!(intent.kind, IntentKind::Investigation);
    }

    /// 测试重构类意图分类
    #[test]
    fn test_classify_refactor() {
        let gate = IntentGate::new();
        let intent = gate.classify("Refactor the database module to simplify queries");
        assert_eq!(intent.kind, IntentKind::Refactor);
    }

    /// 测试规划类意图分类
    #[test]
    fn test_classify_planning() {
        let gate = IntentGate::new();
        let intent = gate.classify("Design a new architecture for the plugin system");
        assert_eq!(intent.kind, IntentKind::Planning);
    }

    /// 测试快速修复意图分类
    #[test]
    fn test_classify_quick_fix() {
        let gate = IntentGate::new();
        let intent = gate.classify("Fix this typo in the README");
        // typo 的高权重使其倾向 QuickFix
        assert!(
            intent.kind == IntentKind::QuickFix || intent.kind == IntentKind::Fix,
            "应为 QuickFix 或 Fix，实际为 {:?}",
            intent.kind
        );
    }

    /// 测试 ultrawork 关键词触发
    #[test]
    fn test_ultrawork_keyword_trigger() {
        let gate = IntentGate::new();

        let intent = gate.classify("ultrawork fix the failing tests");
        assert!(intent.keyword_triggered);
        assert_eq!(intent.confidence, 1.0);
        assert_eq!(intent.trigger_keyword.as_deref(), Some("ultrawork"));

        let intent = gate.classify("ulw add input validation");
        assert!(intent.keyword_triggered);
        assert_eq!(intent.trigger_keyword.as_deref(), Some("ultrawork"));
    }

    /// 测试 search 关键词触发
    #[test]
    fn test_search_keyword_trigger() {
        let gate = IntentGate::new();
        let intent = gate.classify("search for auth implementations");
        assert!(intent.keyword_triggered);
        assert_eq!(intent.kind, IntentKind::Research);
    }

    /// 测试 analyze 关键词触发
    #[test]
    fn test_analyze_keyword_trigger() {
        let gate = IntentGate::new();
        let intent = gate.classify("analyze why this race condition happens");
        assert!(intent.keyword_triggered);
        assert_eq!(intent.kind, IntentKind::Investigation);
    }

    /// 测试无匹配时回退到 Implementation
    #[test]
    fn test_fallback_to_implementation() {
        let gate = IntentGate::new();
        let intent = gate.classify("do something with the data");
        assert_eq!(intent.kind, IntentKind::Implementation);
        assert!(intent.confidence < 0.5); // 低置信度
    }

    /// 测试推荐类别映射
    #[test]
    fn test_recommended_categories() {
        assert_eq!(IntentKind::Research.recommended_category(), "deep");
        assert_eq!(
            IntentKind::Implementation.recommended_category(),
            "unspecified-high"
        );
        assert_eq!(IntentKind::Fix.recommended_category(), "unspecified-low");
        assert_eq!(
            IntentKind::Investigation.recommended_category(),
            "ultrabrain"
        );
        assert_eq!(IntentKind::Refactor.recommended_category(), "deep");
        assert_eq!(
            IntentKind::Planning.recommended_category(),
            "unspecified-high"
        );
        assert_eq!(IntentKind::QuickFix.recommended_category(), "quick");
    }

    /// 测试需要深度思考的意图
    #[test]
    fn test_requires_deep_thinking() {
        assert!(IntentKind::Investigation.requires_deep_thinking());
        assert!(IntentKind::Refactor.requires_deep_thinking());
        assert!(IntentKind::Planning.requires_deep_thinking());
        assert!(!IntentKind::QuickFix.requires_deep_thinking());
        assert!(!IntentKind::Implementation.requires_deep_thinking());
    }

    /// 测试中文关键词分类
    #[test]
    fn test_chinese_keywords() {
        let gate = IntentGate::new();

        let intent = gate.classify("实现一个新的 API 接口");
        assert_eq!(intent.kind, IntentKind::Implementation);

        let intent = gate.classify("修复登录页面的错误");
        assert_eq!(intent.kind, IntentKind::Fix);

        let intent = gate.classify("重构数据库模块");
        assert_eq!(intent.kind, IntentKind::Refactor);
    }

    /// 测试 IntentKind 的 Display 格式
    #[test]
    fn test_intent_kind_display() {
        assert_eq!(format!("{}", IntentKind::Research), "research");
        assert_eq!(format!("{}", IntentKind::QuickFix), "quick-fix");
    }

    /// 测试分类结果包含推荐类别
    #[test]
    fn test_classified_intent_has_category() {
        let gate = IntentGate::new();
        let intent = gate.classify("Build a new feature");
        assert!(!intent.recommended_category.is_empty());
    }

    /// 测试 ulw 单独使用
    #[test]
    fn test_ulw_standalone() {
        let gate = IntentGate::new();
        let intent = gate.classify("ulw");
        assert!(intent.keyword_triggered);
    }
}
