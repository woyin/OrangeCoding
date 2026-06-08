/**
 * @module @orangecoding/mesh
 * Multi-agent coordination layer.
 */

// ---------------------------------------------------------------------------
// Registry & ManagedAgent
// ---------------------------------------------------------------------------
export { AgentRegistry } from "./registry.js";
export type { AgentInfo, HealthReport, ManagedAgent } from "./registry.js";

// ---------------------------------------------------------------------------
// AgentPool
// ---------------------------------------------------------------------------
export { AgentPool } from "./agent-pool.js";
export type { AgentPoolConfig, AgentFactory, PoolStatus } from "./agent-pool.js";

// ---------------------------------------------------------------------------
// Budget
// ---------------------------------------------------------------------------
export { BudgetGuard } from "./budget.js";
export type { ToolBudget, BudgetUsage } from "./budget.js";

// ---------------------------------------------------------------------------
// MessageBus
// ---------------------------------------------------------------------------
export { MessageBus } from "./bus.js";
export type { MessageHandler } from "./bus.js";

// ---------------------------------------------------------------------------
// Collaboration
// ---------------------------------------------------------------------------
export { CollaborationRouter } from "./collaboration.js";
export type {
  TaskClassifier,
  AssignmentPlan,
  CollaborationProtocol,
} from "./collaboration.js";

// ---------------------------------------------------------------------------
// MasterWorker
// ---------------------------------------------------------------------------
export { MasterWorker } from "./master-worker.js";

// ---------------------------------------------------------------------------
// Pipeline
// ---------------------------------------------------------------------------
export { Pipeline } from "./pipeline.js";

// ---------------------------------------------------------------------------
// PeerNegotiation
// ---------------------------------------------------------------------------
export { PeerNegotiation } from "./peer.js";
export type { Bid, PeerNegotiationConfig } from "./peer.js";

// ---------------------------------------------------------------------------
// DynamicCollaboration
// ---------------------------------------------------------------------------
export { DynamicCollaboration } from "./dynamic.js";

// ---------------------------------------------------------------------------
// HealthMonitor
// ---------------------------------------------------------------------------
export { HealthMonitor } from "./health.js";
export type { HealthMonitorConfig } from "./health.js";

// ---------------------------------------------------------------------------
// Negotiator & BuddyObserver
// ---------------------------------------------------------------------------
export { Negotiator, BuddyObserver } from "./negotiator.js";
export type { HandoffMessage } from "./negotiator.js";

// ---------------------------------------------------------------------------
// TaskOrchestrator
// ---------------------------------------------------------------------------
export { TaskOrchestrator } from "./orchestrator.js";
export type { TaskFunc, OrchestratorTask } from "./orchestrator.js";

// ---------------------------------------------------------------------------
// MessageStore
// ---------------------------------------------------------------------------
export { InMemoryMessageStore } from "./message-store.js";
export type { MessageStore, MessageId, MeshMessage } from "./message-store.js";

// ---------------------------------------------------------------------------
// ReliableBus
// ---------------------------------------------------------------------------
export { ReliableBus, Delivery } from "./reliable-bus.js";
export type { Subscription, SecurityGuard as BusSecurityGuard } from "./reliable-bus.js";

// ---------------------------------------------------------------------------
// Security
// ---------------------------------------------------------------------------
export { PermissionGuard, CommandApprovalGuard } from "./security.js";
export type { SecurityGuard, Approver } from "./security.js";

// ---------------------------------------------------------------------------
// Stream
// ---------------------------------------------------------------------------
export { Stream, StreamEventType } from "./stream.js";
export type { StreamEvent } from "./stream.js";

// ---------------------------------------------------------------------------
// Validation
// ---------------------------------------------------------------------------
export { OutputValidator } from "./validation.js";
