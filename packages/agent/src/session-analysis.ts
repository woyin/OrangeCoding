/**
 * Session analysis engine — examines completed sessions for self-improvement insights.
 *
 * Consumes HarnessCheckpoint data to produce structured insights:
 *   - Tool usage patterns (frequency, success/fail ratio)
 *   - Error clustering (common failure modes)
 *   - Token efficiency (cost per task, waste detection)
 *   - Iteration efficiency (how many loops to complete)
 *   - Guardrail trigger frequency
 *
 * Insights are stored via LongMemoryStore so future sessions can benefit.
 */

import type { CheckpointStore, HarnessCheckpoint } from "./harness-state.js";
import { FileCheckpointStore } from "./harness-checkpoint-file.js";
import type { LongMemoryStore } from "./long-memory.js";

// ---------------------------------------------------------------------------
// Analysis types
// ---------------------------------------------------------------------------

export interface ToolUsageStat {
  tool: string;
  count: number;
  errorCount: number;
}

export interface StopReasonStat {
  reason: string;
  count: number;
  percentage: number;
}

export interface TokenEfficiency {
  totalTokens: number;
  avgTokensPerSession: number;
  avgTokensPerToolCall: number;
  highWatermark: number;
  lowWatermark: number;
}

export interface IterationProfile {
  avgIterations: number;
  maxIterations: number;
  minIterations: number;
  avgToolCallsPerSession: number;
}

export interface ErrorCluster {
  pattern: string;
  count: number;
  sampleTask: string;
}

export interface SessionInsight {
  category: "tool_usage" | "error_pattern" | "token_efficiency" | "iteration" | "general";
  title: string;
  description: string;
  severity: "info" | "warning" | "critical";
  actionable: boolean;
  suggestion: string;
}

export interface SessionAnalysisReport {
  analyzedAt: string;
  sessionCount: number;
  toolUsage: ToolUsageStat[];
  stopReasons: StopReasonStat[];
  tokenEfficiency: TokenEfficiency;
  iterationProfile: IterationProfile;
  errorClusters: ErrorCluster[];
  insights: SessionInsight[];
}

// ---------------------------------------------------------------------------
// SessionAnalyzer
// ---------------------------------------------------------------------------

export class SessionAnalyzer {
  private _checkpointStore: CheckpointStore;
  private _longMemory?: LongMemoryStore;

  constructor(checkpointDir: string, longMemory?: LongMemoryStore) {
    this._checkpointStore = new FileCheckpointStore(checkpointDir);
    this._longMemory = longMemory;
  }

  /**
   * Analyze all completed sessions and produce a report.
   */
  async analyze(): Promise<SessionAnalysisReport> {
    const summaries = await this._checkpointStore.list(undefined, "");
    const checkpoints: HarnessCheckpoint[] = [];

    for (const summary of summaries) {
      try {
        const cp = await this._checkpointStore.load(undefined, summary.runID);
        checkpoints.push(cp);
      } catch {
        /* skip corrupted */
      }
    }

    const toolUsage = this.analyzeToolUsage(checkpoints);
    const stopReasons = this.analyzeStopReasons(checkpoints);
    const tokenEfficiency = this.analyzeTokenEfficiency(checkpoints);
    const iterationProfile = this.analyzeIterationProfile(checkpoints);
    const errorClusters = this.analyzeErrorClusters(checkpoints);
    const insights = this.generateInsights(
      toolUsage,
      stopReasons,
      tokenEfficiency,
      iterationProfile,
      errorClusters,
      checkpoints.length,
    );

    const report: SessionAnalysisReport = {
      analyzedAt: new Date().toISOString(),
      sessionCount: checkpoints.length,
      toolUsage,
      stopReasons,
      tokenEfficiency,
      iterationProfile,
      errorClusters,
      insights,
    };

    await this.persistInsights(insights);
    return report;
  }

  // --- Tool Usage ---

  private analyzeToolUsage(checkpoints: HarnessCheckpoint[]): ToolUsageStat[] {
    const toolMap = new Map<string, { count: number; errors: number }>();

    for (const cp of checkpoints) {
      if (!cp.recentToolKeys) continue;
      for (const key of cp.recentToolKeys) {
        const existing = toolMap.get(key) ?? { count: 0, errors: 0 };
        existing.count += 1;
        toolMap.set(key, existing);
      }
    }

    // Estimate error count from failed states
    for (const cp of checkpoints) {
      if (cp.state === "failed" && cp.recentToolKeys && cp.recentToolKeys.length > 0) {
        const lastTool = cp.recentToolKeys[cp.recentToolKeys.length - 1]!;
        const existing = toolMap.get(lastTool);
        if (existing) {
          existing.errors += 1;
        }
      }
    }

    const stats: ToolUsageStat[] = [];
    for (const [tool, data] of toolMap) {
      stats.push({ tool, count: data.count, errorCount: data.errors });
    }
    return stats.sort((a, b) => b.count - a.count);
  }

  // --- Stop Reasons ---

  private analyzeStopReasons(checkpoints: HarnessCheckpoint[]): StopReasonStat[] {
    const reasonMap = new Map<string, number>();
    const total = checkpoints.length || 1;

    for (const cp of checkpoints) {
      const reason = cp.stopReason ?? (cp.state === "completed" ? "completed" : cp.state);
      reasonMap.set(reason, (reasonMap.get(reason) ?? 0) + 1);
    }

    const stats: StopReasonStat[] = [];
    for (const [reason, count] of reasonMap) {
      stats.push({ reason, count, percentage: Math.round((count / total) * 100) });
    }
    return stats.sort((a, b) => b.count - a.count);
  }

  // --- Token Efficiency ---

  private analyzeTokenEfficiency(checkpoints: HarnessCheckpoint[]): TokenEfficiency {
    const tokens: number[] = [];
    const toolCalls: number[] = [];

    for (const cp of checkpoints) {
      const total = cp.tokenUsage?.totalTokens ?? 0;
      tokens.push(total);
      toolCalls.push(cp.toolCallsMade || 1);
    }

    const totalTokens = tokens.reduce((a, b) => a + b, 0);
    const totalToolCalls = toolCalls.reduce((a, b) => a + b, 0);
    const count = tokens.length || 1;

    return {
      totalTokens,
      avgTokensPerSession: Math.round(totalTokens / count),
      avgTokensPerToolCall: totalToolCalls > 0 ? Math.round(totalTokens / totalToolCalls) : 0,
      highWatermark: tokens.length > 0 ? Math.max(...tokens) : 0,
      lowWatermark: tokens.length > 0 ? Math.min(...tokens) : 0,
    };
  }

  // --- Iteration Profile ---

  private analyzeIterationProfile(checkpoints: HarnessCheckpoint[]): IterationProfile {
    const iterations: number[] = [];
    const toolCalls: number[] = [];

    for (const cp of checkpoints) {
      iterations.push(cp.iteration || 0);
      toolCalls.push(cp.toolCallsMade || 0);
    }

    const count = iterations.length || 1;

    return {
      avgIterations: Math.round(iterations.reduce((a, b) => a + b, 0) / count),
      maxIterations: iterations.length > 0 ? Math.max(...iterations) : 0,
      minIterations: iterations.length > 0 ? Math.min(...iterations) : 0,
      avgToolCallsPerSession: Math.round(toolCalls.reduce((a, b) => a + b, 0) / count),
    };
  }

  // --- Error Clustering ---

  private analyzeErrorClusters(checkpoints: HarnessCheckpoint[]): ErrorCluster[] {
    const errorMap = new Map<string, { count: number; tasks: string[] }>();

    for (const cp of checkpoints) {
      if (!cp.lastErrorMessage) continue;
      const pattern = cp.lastErrorMessage.split(":")[0]?.trim() ?? "unknown";
      const existing = errorMap.get(pattern) ?? { count: 0, tasks: [] };
      existing.count += 1;
      if (existing.tasks.length < 3) {
        existing.tasks.push(cp.task.slice(0, 80));
      }
      errorMap.set(pattern, existing);
    }

    const clusters: ErrorCluster[] = [];
    for (const [pattern, data] of errorMap) {
      clusters.push({
        pattern,
        count: data.count,
        sampleTask: data.tasks[0] ?? "",
      });
    }
    return clusters.sort((a, b) => b.count - a.count);
  }

  // --- Insight Generation ---

  private generateInsights(
    toolUsage: ToolUsageStat[],
    stopReasons: StopReasonStat[],
    tokenEfficiency: TokenEfficiency,
    iterationProfile: IterationProfile,
    errorClusters: ErrorCluster[],
    sessionCount: number,
  ): SessionInsight[] {
    const insights: SessionInsight[] = [];

    // High error rate for specific tools
    for (const stat of toolUsage) {
      if (stat.count >= 3 && stat.errorCount > 0) {
        const errorRate = Math.round((stat.errorCount / stat.count) * 100);
        if (errorRate > 30) {
          insights.push({
            category: "tool_usage",
            title: "High error rate: " + stat.tool,
            description: stat.tool + " has " + String(errorRate) + "% error rate (" + String(stat.errorCount) + "/" + String(stat.count) + ")",
            severity: errorRate > 60 ? "critical" : "warning",
            actionable: true,
            suggestion: "Investigate " + stat.tool + " tool implementation for common failure causes. Consider adding retry logic or improving error messages.",
          });
        }
      }
    }

    // Failure stop reason percentage
    const failStat = stopReasons.find((s) => s.reason === "failed" || s.reason === "provider_error");
    if (failStat && failStat.percentage > 20) {
      insights.push({
        category: "error_pattern",
        title: "High failure rate",
        description: String(failStat.percentage) + "% of sessions ended in failure (" + String(failStat.count) + "/" + String(sessionCount) + ")",
        severity: failStat.percentage > 50 ? "critical" : "warning",
        actionable: true,
        suggestion: "Review error clusters below. Consider improving prompt quality, adjusting guardrails, or adding fallback strategies.",
      });
    }

    // Token efficiency
    if (tokenEfficiency.highWatermark > tokenEfficiency.avgTokensPerSession * 3) {
      insights.push({
        category: "token_efficiency",
        title: "Token usage outlier detected",
        description: "Highest session used " + String(tokenEfficiency.highWatermark) + " tokens vs average " + String(tokenEfficiency.avgTokensPerSession),
        severity: "warning",
        actionable: true,
        suggestion: "Investigate the outlier session. It may indicate a runaway loop or overly complex task that could benefit from decomposition.",
      });
    }

    if (tokenEfficiency.avgTokensPerToolCall > 5000) {
      insights.push({
        category: "token_efficiency",
        title: "High token cost per tool call",
        description: "Average " + String(tokenEfficiency.avgTokensPerToolCall) + " tokens per tool call",
        severity: "info",
        actionable: true,
        suggestion: "Consider reducing context window size or using more targeted prompts to lower per-call token usage.",
      });
    }

    // Iteration efficiency
    if (iterationProfile.maxIterations > iterationProfile.avgIterations * 4) {
      insights.push({
        category: "iteration",
        title: "Iteration count outlier",
        description: "Max iterations (" + String(iterationProfile.maxIterations) + ") is much higher than average (" + String(iterationProfile.avgIterations) + ")",
        severity: "warning",
        actionable: true,
        suggestion: "The longest session may be stuck in a loop. Review max_iteration limits and guardrail configurations.",
      });
    }

    // Error clusters
    for (const cluster of errorClusters.slice(0, 3)) {
      if (cluster.count >= 2) {
        insights.push({
          category: "error_pattern",
          title: "Recurring error: " + cluster.pattern.slice(0, 40),
          description: "Seen " + String(cluster.count) + " times. Example task: " + cluster.sampleTask,
          severity: cluster.count >= 5 ? "critical" : "warning",
          actionable: true,
          suggestion: "Address root cause of \"" + cluster.pattern.slice(0, 60) + "\" to prevent recurrence.",
        });
      }
    }

    // General health
    const completedStat = stopReasons.find((s) => s.reason === "completed");
    const completedPct = completedStat?.percentage ?? 0;
    if (completedPct >= 80 && sessionCount >= 5) {
      insights.push({
        category: "general",
        title: "Healthy completion rate",
        description: String(completedPct) + "% of sessions completed successfully",
        severity: "info",
        actionable: false,
        suggestion: "System is performing well. Continue monitoring for regressions.",
      });
    }

    return insights;
  }

  // --- Persist insights to long-term memory ---

  private async persistInsights(insights: SessionInsight[]): Promise<void> {
    if (!this._longMemory || insights.length === 0) return;

    const actionable = insights.filter((i) => i.actionable);
    if (actionable.length === 0) return;

    const keyPoints = actionable
      .slice(0, 5)
      .map((i) => "[" + i.severity + "] " + i.title + ": " + i.suggestion);

    const content = actionable
      .map((i) => "### " + i.title + "\n" + i.description + "\nSuggestion: " + i.suggestion)
      .join("\n\n");

    await this._longMemory.store({
      topic: "session-insights",
      content,
      summary: "Analysis of " + String(insights.length) + " insights from recent sessions.",
      keyPoints,
      lastUpdated: new Date(),
      accessCount: 0,
    });
  }

  /**
   * Format a report as a human-readable string.
   */
  formatReport(report: SessionAnalysisReport): string {
    const lines: string[] = [];
    lines.push("# Session Analysis Report");
    lines.push("> Generated: " + report.analyzedAt + " | Sessions analyzed: " + String(report.sessionCount));
    lines.push("");

    // Stop reasons
    if (report.stopReasons.length > 0) {
      lines.push("## Stop Reasons");
      for (const sr of report.stopReasons) {
        lines.push("- " + sr.reason + ": " + String(sr.count) + " (" + String(sr.percentage) + "%)");
      }
      lines.push("");
    }

    // Tool usage
    if (report.toolUsage.length > 0) {
      lines.push("## Tool Usage");
      for (const tu of report.toolUsage) {
        const errLabel = tu.errorCount > 0 ? " (" + String(tu.errorCount) + " errors)" : "";
        lines.push("- " + tu.tool + ": " + String(tu.count) + " calls" + errLabel);
      }
      lines.push("");
    }

    // Token efficiency
    lines.push("## Token Efficiency");
    lines.push("- Total: " + String(report.tokenEfficiency.totalTokens));
    lines.push("- Avg/session: " + String(report.tokenEfficiency.avgTokensPerSession));
    lines.push("- Avg/tool call: " + String(report.tokenEfficiency.avgTokensPerToolCall));
    lines.push("- Range: " + String(report.tokenEfficiency.lowWatermark) + " - " + String(report.tokenEfficiency.highWatermark));
    lines.push("");

    // Iteration profile
    lines.push("## Iteration Profile");
    lines.push("- Avg iterations: " + String(report.iterationProfile.avgIterations));
    lines.push("- Range: " + String(report.iterationProfile.minIterations) + " - " + String(report.iterationProfile.maxIterations));
    lines.push("- Avg tool calls/session: " + String(report.iterationProfile.avgToolCallsPerSession));
    lines.push("");

    // Error clusters
    if (report.errorClusters.length > 0) {
      lines.push("## Error Clusters");
      for (const ec of report.errorClusters) {
        lines.push("- [" + String(ec.count) + "x] " + ec.pattern);
        lines.push("  Task: " + ec.sampleTask);
      }
      lines.push("");
    }

    // Insights
    if (report.insights.length > 0) {
      lines.push("## Insights");
      for (const insight of report.insights) {
        const icon = insight.severity === "critical" ? "[!]" : insight.severity === "warning" ? "[~]" : "[i]";
        lines.push("### " + icon + " " + insight.title);
        lines.push(insight.description);
        if (insight.actionable) {
          lines.push("Action: " + insight.suggestion);
        }
        lines.push("");
      }
    }

    return lines.join("\n");
  }
}
