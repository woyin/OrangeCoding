/**
 * HarnessProfile groups the agent-harness behavior that is orthogonal to tools.
 * Ported from modules/agent/harness_profile.go.
 */

import type { ChatOptions } from "@orangecoding/ai";
import type { TokenUsage } from "@orangecoding/core";

// ---------------------------------------------------------------------------
// OutputLanguage
// ---------------------------------------------------------------------------

export type OutputLanguage = "auto" | "zh-CN" | "en";

// ---------------------------------------------------------------------------
// ReasoningEffort
// ---------------------------------------------------------------------------

export type ReasoningEffort = "low" | "medium" | "high";

// ---------------------------------------------------------------------------
// ReasoningPolicy
// ---------------------------------------------------------------------------

export interface ReasoningPolicy {
  effort: ReasoningEffort;
  budgetTokens: number;
}

// ---------------------------------------------------------------------------
// LongTaskPolicy
// ---------------------------------------------------------------------------

export interface LongTaskPolicy {
  enabled: boolean;
  maxToolCalls: number;
  progressEveryNCalls: number;
  compactionMaxTokens: number;
}

// ---------------------------------------------------------------------------
// StopReason
// ---------------------------------------------------------------------------

export type StopReason =
  | "completed"
  | "max_iterations"
  | "canceled"
  | "provider_error"
  | "tool_budget"
  | "guardrail";

// ---------------------------------------------------------------------------
// ProgressSnapshot
// ---------------------------------------------------------------------------

export interface ProgressSnapshot {
  iteration: number;
  toolCallsMade: number;
  tokensUsed: TokenUsage;
  reason: string;
  createdAt: Date;
}

// ---------------------------------------------------------------------------
// HarnessProfile
// ---------------------------------------------------------------------------

export interface HarnessProfileData {
  language: OutputLanguage;
  longTask: LongTaskPolicy;
  reasoning: ReasoningPolicy;
}

const DEFAULT_HARNESS_PROFILE_DATA: HarnessProfileData = {
  language: "zh-CN",
  longTask: {
    enabled: true,
    maxToolCalls: 120,
    progressEveryNCalls: 5,
    compactionMaxTokens: 24000,
  },
  reasoning: {
    effort: "high",
    budgetTokens: 4096,
  },
};

export class HarnessProfile {
  language: OutputLanguage;
  longTask: LongTaskPolicy;
  reasoning: ReasoningPolicy;

  constructor(data?: Partial<HarnessProfileData>) {
    const defaults = DEFAULT_HARNESS_PROFILE_DATA;
    this.language = data?.language ?? defaults.language;
    this.longTask = { ...defaults.longTask, ...data?.longTask };
    this.reasoning = { ...defaults.reasoning, ...data?.reasoning };
  }

  normalized(): HarnessProfile {
    const defaults = defaultHarnessProfile();
    const p = new HarnessProfile({
      language: this.language || defaults.language,
      longTask: this.longTask,
      reasoning: this.reasoning,
    });

    if (!p.language) p.language = defaults.language;
    if (!p.reasoning.effort) p.reasoning.effort = defaults.reasoning.effort;
    if (!p.reasoning.budgetTokens) p.reasoning.budgetTokens = defaults.reasoning.budgetTokens;
    if (p.longTask.enabled) {
      if (!p.longTask.maxToolCalls) p.longTask.maxToolCalls = defaults.longTask.maxToolCalls;
      if (!p.longTask.progressEveryNCalls) p.longTask.progressEveryNCalls = defaults.longTask.progressEveryNCalls;
      if (!p.longTask.compactionMaxTokens) p.longTask.compactionMaxTokens = defaults.longTask.compactionMaxTokens;
    }
    return p;
  }

  /** Returns the system prompt text to append for this profile's behavior. */
  systemPromptAddendum(): string {
    const p = this.normalized();
    const parts: string[] = ["\n\n[OrangeCoding Harness]\n"];
    if (p.language === "zh-CN") {
      parts.push("- 默认使用简体中文回答；保留代码、命令、路径、API 名称和错误文本的原文。\n");
      parts.push("- 中文表达要直接、结构清晰，先给结论，再给必要证据和下一步。\n");
    }
    if (p.longTask.enabled) {
      parts.push("- 长任务要持续推进：维护简短的阶段目标，定期报告可验证进度，遇到阻塞时说明阻塞事实和下一步。\n");
      parts.push("- 长任务不要反复重读无关上下文；优先保留当前目标、关键决策、待验证假设和最近工具结果。\n");
      parts.push("- 长任务要把工作拆成可检查的阶段；每个阶段结束时记录结果、剩余风险和下一步验证。\n");
    }
    parts.push("- 工具调用前先选择最窄可用工具，严格按工具 JSON schema 填写参数；不确定参数时先读取上下文或列目录。\n");
    parts.push("- 适合并行探索、评审、验证或文档整理的工作，应通过 task 工具的 delegate 动作形成 sub-agent brief，并明确 scope、expected_output 和验收证据。\n");
    parts.push("- 使用充分的内部推理来处理复杂任务，但不要输出隐藏推理链；输出可审计的摘要、证据和决策理由。\n");
    return parts.join("");
  }

  applyToChatOptions(opts: ChatOptions): ChatOptions {
    const p = this.normalized();
    if (!opts.reasoning_effort) {
      opts.reasoning_effort = p.reasoning.effort;
    }
    if (!opts.reasoning_budget_tokens && p.reasoning.budgetTokens > 0) {
      opts.reasoning_budget_tokens = p.reasoning.budgetTokens;
    }
    return opts;
  }

  shouldRecordProgress(toolCalls: number): boolean {
    const p = this.normalized();
    if (!p.longTask.enabled) return false;
    if (toolCalls === 0) return true;
    return toolCalls % p.longTask.progressEveryNCalls === 0;
  }
}

/** DefaultHarnessProfile returns conservative defaults for long-running Chinese work. */
export function defaultHarnessProfile(): HarnessProfile {
  return new HarnessProfile(DEFAULT_HARNESS_PROFILE_DATA);
}
