//! Enhanced Iterative Code Review: 增强迭代式代码审查模块
//!
//! 分析 git diff，检测常见问题模式，迭代审查直至质量达标。

use serde::{Deserialize, Serialize};

// ─── 枚举与数据结构 ───────────────────────────────────────────

/// 审查严重性
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum ReviewSeverity {
    Critical,
    High,
    Medium,
    Low,
}

/// 审查维度
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReviewDimension {
    /// 正确性（bug、逻辑错误、边界条件）
    Correctness,
    /// 安全性（注入、认证缺陷、数据泄露）
    Security,
    /// 性能（低效、阻塞、冗余）
    Performance,
    /// 可维护性（结构、可读性、耦合）
    Maintainability,
    /// 测试覆盖
    Testing,
}

/// 单条审查发现
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewFinding {
    /// 严重性
    pub severity: ReviewSeverity,
    /// 文件路径
    pub file: String,
    /// 行范围
    pub line_range: String,
    /// 问题标题
    pub issue: String,
    /// 问题解释
    pub explanation: String,
    /// 修复建议
    pub suggestion: String,
    /// 审查维度
    pub dimension: ReviewDimension,
}

/// 审查裁决
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReviewVerdict {
    /// 代码质量可接受
    Correct,
    /// 代码存在问题
    Incorrect,
}

/// 完整审查报告
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewReport {
    /// 最终发现
    pub findings: Vec<ReviewFinding>,
    /// 改进后的 diff
    pub improved_diff: Option<String>,
    /// 总结
    pub summary: String,
    /// 裁决
    pub verdict: ReviewVerdict,
    /// 置信度 (0.0‥1.0)
    pub confidence: f64,
    /// 迭代次数
    pub iterations: usize,
}

// ─── 模式匹配器 ───────────────────────────────────────────────

/// 单条审查模式
#[derive(Debug, Clone)]
pub struct ReviewPattern {
    /// 模式名称
    pub name: String,
    /// 简单字符串匹配模式
    pub pattern: String,
    /// 对应的维度
    pub dimension: ReviewDimension,
    /// 严重性
    pub severity: ReviewSeverity,
    /// 解释
    pub explanation: String,
    /// 建议
    pub suggestion: String,
}

/// 审查模式匹配器 — 检测代码中的常见问题模式
pub struct PatternMatcher {
    patterns: Vec<ReviewPattern>,
}

/// 迭代式代码审查引擎
pub struct CodeReviewer {
    matcher: PatternMatcher,
    max_iterations: usize,
}

// ─── PatternMatcher 实现 ──────────────────────────────────────

impl PatternMatcher {
    /// 创建带默认模式的匹配器
    pub fn with_defaults() -> Self {
        let patterns = vec![
            // ── 正确性 ──
            ReviewPattern {
                name: "unwrap_usage".into(),
                pattern: ".unwrap()".into(),
                dimension: ReviewDimension::Correctness,
                severity: ReviewSeverity::Medium,
                explanation: "unwrap() 在遇到 None/Err 时会 panic".into(),
                suggestion: "考虑使用 ? 或 expect".into(),
            },
            ReviewPattern {
                name: "todo_macro".into(),
                pattern: "todo!()".into(),
                dimension: ReviewDimension::Correctness,
                severity: ReviewSeverity::High,
                explanation: "存在未完成的代码占位符".into(),
                suggestion: "实现 todo!() 标记的功能或移除".into(),
            },
            ReviewPattern {
                name: "unimplemented_macro".into(),
                pattern: "unimplemented!()".into(),
                dimension: ReviewDimension::Correctness,
                severity: ReviewSeverity::High,
                explanation: "存在未实现的代码".into(),
                suggestion: "实现该功能或提供回退逻辑".into(),
            },
            ReviewPattern {
                name: "panic_in_lib".into(),
                pattern: "panic!(".into(),
                dimension: ReviewDimension::Correctness,
                severity: ReviewSeverity::High,
                explanation: "库代码不应 panic，应返回 Result".into(),
                suggestion: "将 panic!() 替换为返回 Result<T, E>".into(),
            },
            // ── 安全性 ──
            ReviewPattern {
                name: "hardcoded_password".into(),
                pattern: "password = \"".into(),
                dimension: ReviewDimension::Security,
                severity: ReviewSeverity::Critical,
                explanation: "硬编码凭证存在安全风险".into(),
                suggestion: "使用环境变量或配置管理存储凭证".into(),
            },
            ReviewPattern {
                name: "hardcoded_secret".into(),
                pattern: "secret = \"".into(),
                dimension: ReviewDimension::Security,
                severity: ReviewSeverity::Critical,
                explanation: "硬编码凭证存在安全风险".into(),
                suggestion: "使用环境变量或配置管理存储凭证".into(),
            },
            ReviewPattern {
                name: "hardcoded_token".into(),
                pattern: "token = \"".into(),
                dimension: ReviewDimension::Security,
                severity: ReviewSeverity::Critical,
                explanation: "硬编码凭证存在安全风险".into(),
                suggestion: "使用环境变量或配置管理存储凭证".into(),
            },
            ReviewPattern {
                name: "unsafe_block".into(),
                pattern: "unsafe {".into(),
                dimension: ReviewDimension::Security,
                severity: ReviewSeverity::High,
                explanation: "unsafe 代码需要额外安全审查".into(),
                suggestion: "确保 unsafe 块有充分的安全性文档和测试".into(),
            },
            ReviewPattern {
                name: "tls_verify_disabled".into(),
                pattern: ".set_verify(false)".into(),
                dimension: ReviewDimension::Security,
                severity: ReviewSeverity::Critical,
                explanation: "禁用 TLS 验证会导致中间人攻击风险".into(),
                suggestion: "启用 TLS 证书验证".into(),
            },
            ReviewPattern {
                name: "danger_accept_invalid_certs".into(),
                pattern: "danger_accept_invalid_certs".into(),
                dimension: ReviewDimension::Security,
                severity: ReviewSeverity::Critical,
                explanation: "禁用 TLS 验证会导致中间人攻击风险".into(),
                suggestion: "启用 TLS 证书验证".into(),
            },
            // ── 性能 ──
            ReviewPattern {
                name: "clone_in_loop".into(),
                pattern: ".clone()".into(),
                dimension: ReviewDimension::Performance,
                severity: ReviewSeverity::Medium,
                explanation: "循环内克隆可能影响性能".into(),
                suggestion: "考虑使用引用或 Cow<T> 避免不必要的克隆".into(),
            },
            ReviewPattern {
                name: "unnecessary_collect".into(),
                pattern: ".collect::<Vec<_>>()".into(),
                dimension: ReviewDimension::Performance,
                severity: ReviewSeverity::Low,
                explanation: "不必要的中间收集可能浪费分配".into(),
                suggestion: "检查是否可以直接链式调用迭代器".into(),
            },
            // ── 可维护性 ──
            // 函数过长和嵌套过深通过 scan() 中的专用逻辑检测
            // ── 测试 ──
            ReviewPattern {
                name: "ignored_test".into(),
                pattern: "#[ignore]".into(),
                dimension: ReviewDimension::Testing,
                severity: ReviewSeverity::Medium,
                explanation: "被忽略的测试不会在 CI 中执行".into(),
                suggestion: "修复或移除被忽略的测试".into(),
            },
        ];

        Self { patterns }
    }

    /// 添加自定义模式
    pub fn add_pattern(&mut self, pattern: ReviewPattern) {
        self.patterns.push(pattern);
    }

    /// 扫描 diff 文本，返回发现的问题
    pub fn scan(&self, diff: &str) -> Vec<ReviewFinding> {
        let mut findings = Vec::new();

        // 逐行扫描，仅检查新增行（以 '+' 开头，但排除 '+++' 文件头）
        for (line_idx, line) in diff.lines().enumerate() {
            let is_added = line.starts_with('+') && !line.starts_with("+++");

            if !is_added {
                continue;
            }

            // 跳过测试代码中的 unwrap 检测（测试里 unwrap 是可接受的）
            let in_test_context = Self::is_test_context(diff, line_idx);

            for pattern in &self.patterns {
                if pattern.name == "unwrap_usage" && in_test_context {
                    continue;
                }

                if line.contains(&pattern.pattern) {
                    let (file, line_range) =
                        Self::extract_file_context(diff, line_idx);
                    findings.push(ReviewFinding {
                        severity: pattern.severity.clone(),
                        file,
                        line_range,
                        issue: pattern.name.clone(),
                        explanation: pattern.explanation.clone(),
                        suggestion: pattern.suggestion.clone(),
                        dimension: pattern.dimension.clone(),
                    });
                }
            }
        }

        // 检测测试缺少断言
        findings.extend(Self::detect_assertion_less_tests(diff));

        // 检测函数过长（新增行 > 50 行）
        findings.extend(Self::detect_long_functions(diff));

        // 检测嵌套过深（3+ 层缩进）
        findings.extend(Self::detect_deep_nesting(diff));

        // 去重：同一文件同一模式只报告一次
        findings.dedup_by(|a, b| a.file == b.file && a.issue == b.issue);

        findings
    }

    /// 从 diff 中提取文件路径和行号
    fn extract_file_context(diff: &str, match_line: usize) -> (String, String) {
        let lines: Vec<&str> = diff.lines().collect();
        let mut file = "unknown".to_string();
        let mut hunk_start: usize = 0;

        // 向上搜索最近的文件头和 hunk 头
        for i in (0..=match_line).rev() {
            let l = lines[i];
            if l.starts_with("@@ ") && hunk_start == 0 {
                // 解析 @@ -a,b +c,d @@ 获取新文件行号
                if let Some(plus_part) = l.split('+').nth(1) {
                    if let Some(num) = plus_part.split(',').next() {
                        hunk_start = num.parse::<usize>().unwrap_or(0);
                    }
                }
            }
            if l.starts_with("+++ ") {
                file = l.trim_start_matches("+++ ").trim_start_matches("b/").to_string();
                break;
            }
        }

        // 计算实际行号：从 hunk 起始行向下数新文件行
        let mut new_line = hunk_start;
        if hunk_start > 0 {
            // 从 hunk 头之后到匹配行，累计新文件行号
            let hunk_header_idx = (0..=match_line)
                .rev()
                .find(|&i| lines[i].starts_with("@@ "))
                .unwrap_or(0);
            for i in (hunk_header_idx + 1)..=match_line {
                let l = lines[i];
                if !l.starts_with('-') {
                    // 上下文行和新增行都算新文件行
                    new_line += 1;
                }
            }
            // 减 1 因为我们多算了当前行的 +1
            new_line -= 1;
        }

        let line_range = if hunk_start > 0 {
            format!("{new_line}")
        } else {
            format!("{}", match_line + 1)
        };

        (file, line_range)
    }

    /// 判断某行是否在测试上下文中
    fn is_test_context(diff: &str, line_idx: usize) -> bool {
        let lines: Vec<&str> = diff.lines().collect();
        // 向上搜索是否在 #[test] 或 #[cfg(test)] 范围内
        for i in (0..line_idx).rev() {
            let l = lines[i].trim();
            if l.contains("#[test]") || l.contains("#[cfg(test)]") || l.contains("mod tests") {
                return true;
            }
            // 如果遇到非测试的函数定义，停止搜索
            if l.starts_with("fn ") || l.starts_with("pub fn ") {
                if !l.contains("test") {
                    return false;
                }
            }
        }
        false
    }

    /// 检测无断言的测试函数
    fn detect_assertion_less_tests(diff: &str) -> Vec<ReviewFinding> {
        let mut findings = Vec::new();
        let lines: Vec<&str> = diff.lines().collect();
        let mut i = 0;

        while i < lines.len() {
            let line = lines[i];
            // 寻找新增的 #[test] 属性
            if line.starts_with('+') && line.contains("#[test]") {
                // 向下找函数体并检查是否有 assert
                let mut has_assert = false;
                let mut fn_name = String::new();
                let mut brace_depth: i32 = 0;
                let mut in_body = false;

                for j in (i + 1)..lines.len() {
                    let fl = lines[j];
                    if !in_body && fl.contains("fn ") {
                        fn_name = fl
                            .split("fn ")
                            .nth(1)
                            .unwrap_or("")
                            .split('(')
                            .next()
                            .unwrap_or("unknown")
                            .trim()
                            .to_string();
                    }
                    if fl.contains('{') {
                        brace_depth += fl.matches('{').count() as i32;
                        in_body = true;
                    }
                    if fl.contains('}') {
                        brace_depth -= fl.matches('}').count() as i32;
                    }
                    if fl.contains("assert") {
                        has_assert = true;
                        break;
                    }
                    if in_body && brace_depth <= 0 {
                        break;
                    }
                }

                if in_body && !has_assert {
                    let (file, line_range) = Self::extract_file_context(diff, i);
                    findings.push(ReviewFinding {
                        severity: ReviewSeverity::High,
                        file,
                        line_range,
                        issue: "test_no_assertion".into(),
                        explanation: format!(
                            "测试函数 `{fn_name}` 缺少断言，无法验证行为"
                        ),
                        suggestion: "添加 assert!、assert_eq! 或 assert_ne! 断言".into(),
                        dimension: ReviewDimension::Testing,
                    });
                }
            }
            i += 1;
        }

        findings
    }

    /// 检测过长函数（新增行 > 50 行）
    fn detect_long_functions(diff: &str) -> Vec<ReviewFinding> {
        let mut findings = Vec::new();
        let lines: Vec<&str> = diff.lines().collect();
        let mut i = 0;

        while i < lines.len() {
            let line = lines[i];
            // 寻找新增的函数定义
            if line.starts_with('+') && (line.contains("fn ") && line.contains('(')) {
                let fn_name = line
                    .split("fn ")
                    .nth(1)
                    .unwrap_or("")
                    .split('(')
                    .next()
                    .unwrap_or("unknown")
                    .trim()
                    .to_string();

                let mut added_lines: usize = 0;
                let mut brace_depth: i32 = 0;
                let mut in_body = false;

                for j in i..lines.len() {
                    let fl = lines[j];
                    if fl.contains('{') {
                        brace_depth += fl.matches('{').count() as i32;
                        in_body = true;
                    }
                    if fl.contains('}') {
                        brace_depth -= fl.matches('}').count() as i32;
                    }
                    if fl.starts_with('+') {
                        added_lines += 1;
                    }
                    if in_body && brace_depth <= 0 {
                        break;
                    }
                }

                if added_lines > 50 {
                    let (file, line_range) = Self::extract_file_context(diff, i);
                    findings.push(ReviewFinding {
                        severity: ReviewSeverity::Medium,
                        file,
                        line_range,
                        issue: "long_function".into(),
                        explanation: format!(
                            "函数 `{fn_name}` 新增 {added_lines} 行，超过 50 行建议阈值"
                        ),
                        suggestion: "考虑拆分为更小的辅助函数".into(),
                        dimension: ReviewDimension::Maintainability,
                    });
                }
            }
            i += 1;
        }

        findings
    }

    /// 检测嵌套过深（3+ 层缩进的新增代码）
    fn detect_deep_nesting(diff: &str) -> Vec<ReviewFinding> {
        let mut findings = Vec::new();
        let mut reported_files: Vec<String> = Vec::new();

        for (line_idx, line) in diff.lines().enumerate() {
            if !line.starts_with('+') || line.starts_with("+++") {
                continue;
            }

            let content = &line[1..]; // 去掉 '+' 前缀
            let indent = content.len() - content.trim_start().len();
            // 4 空格一层，3 层 = 12 空格
            if indent >= 12 && !content.trim().is_empty() {
                let (file, line_range) = Self::extract_file_context(diff, line_idx);
                if !reported_files.contains(&file) {
                    reported_files.push(file.clone());
                    findings.push(ReviewFinding {
                        severity: ReviewSeverity::Low,
                        file,
                        line_range,
                        issue: "deep_nesting".into(),
                        explanation: "嵌套过深（3+ 层），降低可读性".into(),
                        suggestion: "使用 early return 或提取辅助函数减少嵌套".into(),
                        dimension: ReviewDimension::Maintainability,
                    });
                }
            }
        }

        findings
    }
}

// ─── ReviewReport 实现 ────────────────────────────────────────

impl ReviewReport {
    /// 是否通过审查
    pub fn is_correct(&self) -> bool {
        self.verdict == ReviewVerdict::Correct
    }

    /// 按严重性获取发现
    pub fn findings_by_severity(&self, severity: &ReviewSeverity) -> Vec<&ReviewFinding> {
        self.findings
            .iter()
            .filter(|f| &f.severity == severity)
            .collect()
    }

    /// 按维度获取发现
    pub fn findings_by_dimension(&self, dimension: &ReviewDimension) -> Vec<&ReviewFinding> {
        self.findings
            .iter()
            .filter(|f| &f.dimension == dimension)
            .collect()
    }

    /// 导出为 markdown
    pub fn to_markdown(&self) -> String {
        let mut md = String::new();

        md.push_str("# 代码审查报告\n\n");
        md.push_str(&format!(
            "**裁决**: {}\n",
            match &self.verdict {
                ReviewVerdict::Correct => "✅ Correct",
                ReviewVerdict::Incorrect => "❌ Incorrect",
            }
        ));
        md.push_str(&format!("**置信度**: {:.1}%\n", self.confidence * 100.0));
        md.push_str(&format!("**迭代次数**: {}\n\n", self.iterations));

        if !self.findings.is_empty() {
            md.push_str("## 发现\n\n");
            for finding in &self.findings {
                let severity = match &finding.severity {
                    ReviewSeverity::Critical => "Critical",
                    ReviewSeverity::High => "High",
                    ReviewSeverity::Medium => "Medium",
                    ReviewSeverity::Low => "Low",
                };
                let dimension = match &finding.dimension {
                    ReviewDimension::Correctness => "Correctness",
                    ReviewDimension::Security => "Security",
                    ReviewDimension::Performance => "Performance",
                    ReviewDimension::Maintainability => "Maintainability",
                    ReviewDimension::Testing => "Testing",
                };

                md.push_str(&format!("### [{}] {}\n", severity, finding.issue));
                md.push_str(&format!(
                    "- **文件**: {}:{}\n",
                    finding.file, finding.line_range
                ));
                md.push_str(&format!("- **维度**: {}\n", dimension));
                md.push_str(&format!("- **说明**: {}\n", finding.explanation));
                md.push_str(&format!("- **建议**: {}\n\n", finding.suggestion));
            }
        }

        md.push_str("## 总结\n\n");
        md.push_str(&self.summary);
        md.push('\n');

        md
    }

    /// 导出为 JSON 字符串
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_else(|_| "{}".to_string())
    }
}

// ─── CodeReviewer 实现 ────────────────────────────────────────

impl CodeReviewer {
    /// 创建审查器（默认最多 3 次迭代）
    pub fn new() -> Self {
        Self {
            matcher: PatternMatcher::with_defaults(),
            max_iterations: 3,
        }
    }

    /// 设置最大迭代次数
    pub fn with_max_iterations(mut self, n: usize) -> Self {
        self.max_iterations = n;
        self
    }

    /// 执行迭代式代码审查
    pub fn review(&self, diff: &str) -> ReviewReport {
        let mut iterations = 0;
        let mut findings = Vec::new();

        for _ in 0..self.max_iterations {
            iterations += 1;
            findings = self.review_iteration(diff);

            // 无 Critical/High 发现则提前结束
            let has_severe = findings.iter().any(|f| {
                matches!(f.severity, ReviewSeverity::Critical | ReviewSeverity::High)
            });

            if !has_severe {
                break;
            }
        }

        let verdict = Self::determine_verdict(&findings);
        let confidence = Self::calculate_confidence(&findings, iterations);
        let summary = Self::generate_summary(&findings);

        ReviewReport {
            findings,
            improved_diff: None,
            summary,
            verdict,
            confidence,
            iterations,
        }
    }

    /// 单次审查迭代
    fn review_iteration(&self, diff: &str) -> Vec<ReviewFinding> {
        self.matcher.scan(diff)
    }

    /// 根据发现确定裁决
    fn determine_verdict(findings: &[ReviewFinding]) -> ReviewVerdict {
        let has_critical_or_high = findings.iter().any(|f| {
            matches!(f.severity, ReviewSeverity::Critical | ReviewSeverity::High)
        });

        if has_critical_or_high {
            ReviewVerdict::Incorrect
        } else {
            ReviewVerdict::Correct
        }
    }

    /// 计算置信度
    fn calculate_confidence(findings: &[ReviewFinding], _iterations: usize) -> f64 {
        let mut confidence = 0.9_f64;

        for finding in findings {
            match finding.severity {
                ReviewSeverity::Critical => confidence -= 0.15,
                ReviewSeverity::High => confidence -= 0.1,
                ReviewSeverity::Medium => confidence -= 0.03,
                ReviewSeverity::Low => {}
            }
        }

        confidence.clamp(0.0, 1.0)
    }

    /// 生成改进建议摘要
    fn generate_summary(findings: &[ReviewFinding]) -> String {
        if findings.is_empty() {
            return "代码审查通过，未发现问题。".to_string();
        }

        let critical = findings
            .iter()
            .filter(|f| f.severity == ReviewSeverity::Critical)
            .count();
        let high = findings
            .iter()
            .filter(|f| f.severity == ReviewSeverity::High)
            .count();
        let medium = findings
            .iter()
            .filter(|f| f.severity == ReviewSeverity::Medium)
            .count();
        let low = findings
            .iter()
            .filter(|f| f.severity == ReviewSeverity::Low)
            .count();

        let mut parts = Vec::new();
        if critical > 0 {
            parts.push(format!("{critical} 个严重问题"));
        }
        if high > 0 {
            parts.push(format!("{high} 个高危问题"));
        }
        if medium > 0 {
            parts.push(format!("{medium} 个中等问题"));
        }
        if low > 0 {
            parts.push(format!("{low} 个低风险问题"));
        }

        format!("共发现 {} 个问题：{}。", findings.len(), parts.join("、"))
    }
}

impl Default for CodeReviewer {
    fn default() -> Self {
        Self::new()
    }
}

// ─── 测试 ─────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_diff(added_lines: &[&str]) -> String {
        let mut diff = String::new();
        diff.push_str("diff --git a/src/lib.rs b/src/lib.rs\n");
        diff.push_str("--- a/src/lib.rs\n");
        diff.push_str("+++ b/src/lib.rs\n");
        diff.push_str("@@ -1,5 +1,10 @@\n");
        for line in added_lines {
            diff.push_str(&format!("+{line}\n"));
        }
        diff
    }

    // 1. 空 diff → Correct，无发现
    #[test]
    fn empty_diff_clean_review() {
        let reviewer = CodeReviewer::new();
        let report = reviewer.review("");
        assert!(report.is_correct());
        assert!(report.findings.is_empty());
        assert_eq!(report.verdict, ReviewVerdict::Correct);
    }

    // 2. 检测 .unwrap()
    #[test]
    fn detect_unwrap() {
        let diff = make_diff(&["let val = some_option.unwrap();"]);
        let reviewer = CodeReviewer::new();
        let report = reviewer.review(&diff);
        let unwraps: Vec<_> = report
            .findings
            .iter()
            .filter(|f| f.issue == "unwrap_usage")
            .collect();
        assert!(!unwraps.is_empty(), "应检测到 unwrap 使用");
        assert_eq!(unwraps[0].severity, ReviewSeverity::Medium);
    }

    // 3. 检测硬编码凭证
    #[test]
    fn detect_hardcoded_secret() {
        let diff = make_diff(&[r#"let password = "hunter2";"#]);
        let reviewer = CodeReviewer::new();
        let report = reviewer.review(&diff);
        let secrets: Vec<_> = report
            .findings
            .iter()
            .filter(|f| f.issue == "hardcoded_password")
            .collect();
        assert!(!secrets.is_empty(), "应检测到硬编码凭证");
        assert_eq!(secrets[0].severity, ReviewSeverity::Critical);
        assert_eq!(secrets[0].dimension, ReviewDimension::Security);
    }

    // 4. 检测 unsafe 块
    #[test]
    fn detect_unsafe() {
        let diff = make_diff(&["unsafe {"]);
        let reviewer = CodeReviewer::new();
        let report = reviewer.review(&diff);
        let unsafes: Vec<_> = report
            .findings
            .iter()
            .filter(|f| f.issue == "unsafe_block")
            .collect();
        assert!(!unsafes.is_empty(), "应检测到 unsafe 代码");
        assert_eq!(unsafes[0].severity, ReviewSeverity::High);
    }

    // 5. 检测 todo!()
    #[test]
    fn detect_todo_macro() {
        let diff = make_diff(&["    todo!()"]);
        let reviewer = CodeReviewer::new();
        let report = reviewer.review(&diff);
        let todos: Vec<_> = report
            .findings
            .iter()
            .filter(|f| f.issue == "todo_macro")
            .collect();
        assert!(!todos.is_empty(), "应检测到 todo!()");
        assert_eq!(todos[0].severity, ReviewSeverity::High);
    }

    // 6. 检测 panic!()
    #[test]
    fn detect_panic_in_lib() {
        let diff = make_diff(&[r#"    panic!("unexpected state");"#]);
        let reviewer = CodeReviewer::new();
        let report = reviewer.review(&diff);
        let panics: Vec<_> = report
            .findings
            .iter()
            .filter(|f| f.issue == "panic_in_lib")
            .collect();
        assert!(!panics.is_empty(), "应检测到 panic!()");
        assert_eq!(panics[0].severity, ReviewSeverity::High);
    }

    // 7. 检测被忽略的测试
    #[test]
    fn detect_ignored_test() {
        let diff = make_diff(&["#[ignore]", "#[test]", "fn my_test() {", "    assert!(true);", "}"]);
        let reviewer = CodeReviewer::new();
        let report = reviewer.review(&diff);
        let ignored: Vec<_> = report
            .findings
            .iter()
            .filter(|f| f.issue == "ignored_test")
            .collect();
        assert!(!ignored.is_empty(), "应检测到被忽略的测试");
        assert_eq!(ignored[0].severity, ReviewSeverity::Medium);
    }

    // 8. 安全的 diff → Correct 裁决
    #[test]
    fn clean_diff_correct_verdict() {
        let diff = make_diff(&[
            "pub fn add(a: i32, b: i32) -> i32 {",
            "    a + b",
            "}",
        ]);
        let reviewer = CodeReviewer::new();
        let report = reviewer.review(&diff);
        assert_eq!(report.verdict, ReviewVerdict::Correct);
        assert!(report.is_correct());
    }

    // 9. 含 Critical 发现 → Incorrect 裁决
    #[test]
    fn dirty_diff_incorrect_verdict() {
        let diff = make_diff(&[r#"let secret = "abc123";"#]);
        let reviewer = CodeReviewer::new();
        let report = reviewer.review(&diff);
        assert_eq!(report.verdict, ReviewVerdict::Incorrect);
        assert!(!report.is_correct());
    }

    // 10. 发现越多置信度越低
    #[test]
    fn confidence_decreases_with_findings() {
        let clean_diff = make_diff(&["let x = 1;"]);
        let dirty_diff = make_diff(&[
            r#"let password = "pw";"#,
            r#"let secret = "sk";"#,
            "    todo!()",
            "    panic!(\"oh no\")",
        ]);

        let reviewer = CodeReviewer::new();
        let clean_report = reviewer.review(&clean_diff);
        let dirty_report = reviewer.review(&dirty_diff);

        assert!(
            clean_report.confidence > dirty_report.confidence,
            "clean={} should be > dirty={}",
            clean_report.confidence,
            dirty_report.confidence,
        );
    }

    // 11. Markdown 格式验证
    #[test]
    fn report_markdown_format() {
        let diff = make_diff(&["unsafe {"]);
        let reviewer = CodeReviewer::new();
        let report = reviewer.review(&diff);
        let md = report.to_markdown();

        assert!(md.contains("# 代码审查报告"), "应包含标题");
        assert!(md.contains("**裁决**"), "应包含裁决");
        assert!(md.contains("**置信度**"), "应包含置信度");
        assert!(md.contains("**迭代次数**"), "应包含迭代次数");
        assert!(md.contains("## 发现"), "应包含发现章节");
        assert!(md.contains("## 总结"), "应包含总结章节");
        assert!(md.contains("- **文件**:"), "应包含文件信息");
        assert!(md.contains("- **维度**:"), "应包含维度信息");
    }

    // 12. JSON 格式验证
    #[test]
    fn report_json_format() {
        let diff = make_diff(&["unsafe {"]);
        let reviewer = CodeReviewer::new();
        let report = reviewer.review(&diff);
        let json = report.to_json();

        // 应该是合法 JSON
        let parsed: serde_json::Value = serde_json::from_str(&json).expect("应为合法 JSON");
        assert!(parsed.get("findings").is_some());
        assert!(parsed.get("verdict").is_some());
        assert!(parsed.get("confidence").is_some());
        assert!(parsed.get("iterations").is_some());
        assert!(parsed.get("summary").is_some());
    }

    // 13. 按严重性筛选
    #[test]
    fn findings_by_severity() {
        let diff = make_diff(&[
            r#"let password = "pw";"#, // Critical
            "    todo!()",              // High
            "let x = val.unwrap();",   // Medium
        ]);
        let reviewer = CodeReviewer::new();
        let report = reviewer.review(&diff);

        let critical = report.findings_by_severity(&ReviewSeverity::Critical);
        let high = report.findings_by_severity(&ReviewSeverity::High);
        let medium = report.findings_by_severity(&ReviewSeverity::Medium);

        assert!(!critical.is_empty(), "应有 Critical 发现");
        assert!(!high.is_empty(), "应有 High 发现");
        assert!(!medium.is_empty(), "应有 Medium 发现");
    }

    // 14. 按维度筛选
    #[test]
    fn findings_by_dimension() {
        let diff = make_diff(&[
            r#"let password = "pw";"#, // Security
            "    todo!()",              // Correctness
            "#[ignore]",               // Testing
        ]);
        let reviewer = CodeReviewer::new();
        let report = reviewer.review(&diff);

        let security = report.findings_by_dimension(&ReviewDimension::Security);
        let correctness = report.findings_by_dimension(&ReviewDimension::Correctness);
        let testing = report.findings_by_dimension(&ReviewDimension::Testing);

        assert!(!security.is_empty(), "应有 Security 发现");
        assert!(!correctness.is_empty(), "应有 Correctness 发现");
        assert!(!testing.is_empty(), "应有 Testing 发现");
    }
}
