/**
 * @orangecoding/agent - Core agent engine with agent loop, sub-agents, and workflows.
 *
 * Re-exports all public API from the package.
 */

// ---------------------------------------------------------------------------
// Context
// ---------------------------------------------------------------------------
export { AgentContext } from "./context.js";

// ---------------------------------------------------------------------------
// Executor
// ---------------------------------------------------------------------------
export { ToolExecutor, filteredRegistry } from "./executor.js";

// ---------------------------------------------------------------------------
// Security
// ---------------------------------------------------------------------------
export type { SecurityGuard } from "./security-bridge.js";

// ---------------------------------------------------------------------------
// Compaction
// ---------------------------------------------------------------------------
export { Compactor } from "./compaction.js";

// ---------------------------------------------------------------------------
// Hooks
// ---------------------------------------------------------------------------
export { HookManager } from "./hooks.js";
export type { HookPoint, Hook } from "./hooks.js";

// ---------------------------------------------------------------------------
// Intent Gate
// ---------------------------------------------------------------------------
export { IntentGate, suggestCategory } from "./intent-gate.js";
export type { IntentCategory, IntentAnalysis } from "./intent-gate.js";

// ---------------------------------------------------------------------------
// Comment Checker
// ---------------------------------------------------------------------------
export { checkComments, isContentClean } from "./comment-checker.js";
export type { CommentCheckResult, CommentCheckerConfig } from "./comment-checker.js";

// ---------------------------------------------------------------------------
// Memory
// ---------------------------------------------------------------------------
export { MemoryStore } from "./memory.js";
export { LongMemoryStore } from "./long-memory.js";
export type { LongMemoryConfig, MemoryEntry, MemoryIndex, MemoryIndexEntry } from "./long-memory.js";

// ---------------------------------------------------------------------------
// Tiered Memory
// ---------------------------------------------------------------------------
export { TieredMemoryManager } from "./tiered-memory.js";
export type { TieredMemoryConfig } from "./tiered-memory.js";

// ---------------------------------------------------------------------------
// Skills
// ---------------------------------------------------------------------------
export { SkillRegistry } from "./skills.js";
export type { Skill, SkillContext, SkillComposition } from "./skills.js";
export { SkillMatcher } from "./skill-matcher.js";
export type { SkillMatch } from "./skill-matcher.js";

// ---------------------------------------------------------------------------
// TTSR
// ---------------------------------------------------------------------------
export { TTSR } from "./ttsr.js";
export type { Rule } from "./ttsr.js";

// ---------------------------------------------------------------------------
// Tool Definitions
// ---------------------------------------------------------------------------
export { buildToolDefinitions } from "./tool-defs.js";

// ---------------------------------------------------------------------------
// Harness Profile
// ---------------------------------------------------------------------------
export {
  HarnessProfile,
  defaultHarnessProfile,
} from "./harness-profile.js";
export type {
  OutputLanguage,
  ReasoningEffort,
  ReasoningPolicy,
  LongTaskPolicy,
  StopReason,
  ProgressSnapshot,
  HarnessProfileData,
} from "./harness-profile.js";

// ---------------------------------------------------------------------------
// Harness State
// ---------------------------------------------------------------------------
export {
  HarnessState,
  MemoryCheckpointStore,
  checkpointSummary,
} from "./harness-state.js";
export type {
  HarnessState as HarnessStateType,
  HarnessTraceEvent,
  ContextBlockKind,
  ContextBlock,
  HarnessCheckpoint,
  CheckpointSummary,
  CheckpointStore,
} from "./harness-state.js";

// ---------------------------------------------------------------------------
// Harness Engine
// ---------------------------------------------------------------------------
export { HarnessEngine } from "./harness-engine.js";
export type { HarnessEngineConfig } from "./harness-engine.js";

// ---------------------------------------------------------------------------
// Harness Context
// ---------------------------------------------------------------------------
export { HarnessContextBuilder, containsBlockKind, containsBlockText } from "./harness-context.js";
export type { HarnessContextConfig, HarnessContextInput } from "./harness-context.js";

// ---------------------------------------------------------------------------
// Harness Memory
// ---------------------------------------------------------------------------
export { HarnessMemoryManager } from "./harness-memory.js";

// ---------------------------------------------------------------------------
// Harness Embedding
// ---------------------------------------------------------------------------
export { SemanticMemoryManager } from "./harness-embedding.js";
export type {
  EmbeddingVector,
  EmbeddingProvider,
  SemanticMemoryEntry,
  SemanticMemoryConfig,
} from "./harness-embedding.js";

// ---------------------------------------------------------------------------
// Harness Guardrail
// ---------------------------------------------------------------------------
export {
  GuardrailPipeline,
  defaultGuardrailPipeline,
  GuardrailLogger,
  TokenBudgetGuardrail,
  OutputLengthGuardrail,
  LLMGuardrail,
  DangerousToolGuardrail,
  RepeatedToolGuardrail,
  toolCallKey,
} from "./harness-guardrail.js";
export type {
  GuardrailPhase,
  GuardrailDecision,
  GuardrailContext,
  GuardrailResult,
  Guardrail,
  GuardrailLogEntry,
  LLMGuardrailConfig,
  DefaultGuardrailPipelineConfig,
} from "./harness-guardrail.js";

// ---------------------------------------------------------------------------
// Harness Handoff / Orchestrator
// ---------------------------------------------------------------------------
export {
  ToolUseBudget,
  Orchestrator,
  applyModelSettingsToChatOptions,
} from "./harness-handoff.js";
export type {
  HandoffRequest,
  HandoffResult,
  HandoffHandler,
  AgentModelSettings,
  OrchestratorTask,
  OrchestratorResult,
} from "./harness-handoff.js";

// ---------------------------------------------------------------------------
// Harness Trace
// ---------------------------------------------------------------------------
export {
  TRACE_SCHEMA_VERSION,
  MemoryTraceStore,
  FileTraceStore,
  traceEventsToSpans,
} from "./harness-trace.js";
export type {
  TraceEvent,
  TraceQuery,
  TraceStore,
  OTLPSpan,
} from "./harness-trace.js";

// ---------------------------------------------------------------------------
// Harness Checkpoint File
// ---------------------------------------------------------------------------
export { FileCheckpointStore } from "./harness-checkpoint-file.js";

// ---------------------------------------------------------------------------
// Agent Loop
// ---------------------------------------------------------------------------
export {
  AgentLoop,
  defaultLoopConfig,
} from "./loop.js";
export type {
  AgentLoopConfig,
  AgentLoopResult,
} from "./loop.js";

// Resume
// ---------------------------------------------------------------------------
export { ResumeManager } from "./resume.js";
export type { ResumeResult } from "./resume.js";

// ---------------------------------------------------------------------------
// Session Analysis
// ---------------------------------------------------------------------------
export { SessionAnalyzer } from "./session-analysis.js";
export type {
  ToolUsageStat,
  StopReasonStat,
  TokenEfficiency,
  IterationProfile,
  ErrorCluster,
  SessionInsight,
  SessionAnalysisReport,
} from "./session-analysis.js";

// ---------------------------------------------------------------------------
// Fork Agent
// ---------------------------------------------------------------------------
export { ForkAgent } from "./fork.js";

// ---------------------------------------------------------------------------
// Agents
// ---------------------------------------------------------------------------
export { BaseAgent } from "./agents/base.js";
export type { Agent } from "./agents/base.js";
export { newSisyphus } from "./agents/sisyphus.js";
export { newAtlas } from "./agents/atlas.js";
export { newExplore } from "./agents/explore.js";
export { newHephaestus } from "./agents/hephaestus.js";
export { newJunior } from "./agents/junior.js";
export { newLibrarian } from "./agents/librarian.js";
export { newMetis } from "./agents/metis.js";
export { newMomus } from "./agents/momus.js";
export { newMultimodal } from "./agents/multimodal.js";
export { newOracle } from "./agents/oracle.js";
export { newPrometheus, newPrometheusResearch, newPrometheusPlanner, needsInterview, researchPrompt, interviewPrompt, planPrompt } from "./agents/prometheus.js";
export type { InterviewMode, PrometheusConfig, InterviewResult } from "./agents/prometheus.js";
export { newFilteredAgent } from "./agents/helpers.js";

// ---------------------------------------------------------------------------
// Workflows
// ---------------------------------------------------------------------------
export { BoulderRecovery } from "./workflows/boulder.js";
export type { BoulderResult } from "./workflows/boulder.js";
export { PlanningWorkflow } from "./workflows/planning.js";
export type { PlanResult, PlanningWorkflowConfig } from "./workflows/planning.js";
export { ExecutionWorkflow } from "./workflows/execution.js";
export type { ExecutionResult } from "./workflows/execution.js";
export { UltraWork } from "./workflows/ultra-work.js";
export type { UltraWorkResult, UltraWorkConfig, UltraWorkMode, AgentResult } from "./workflows/ultra-work.js";

// ---------------------------------------------------------------------------
// OpenAgent Framework
// ---------------------------------------------------------------------------
export { OpenAgentRuntime, AgentRegistrationBuilder } from "./open-agent.js";
export type { AgentPlugin, OpenAgentConfig } from "./open-agent.js";
export {
  coderPlugin,
  plannerPlugin,
  reviewerPlugin,
  explorerPlugin,
  executorPlugin,
} from "./open-agent.js";
