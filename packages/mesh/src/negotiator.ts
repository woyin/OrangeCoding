/**
 * @module negotiator
 *
 * Task negotiation between mesh peers.
 *
 * When a task arrives, the negotiator:
 * 1. Classifies the task (complexity, type, required skills)
 * 2. Identifies suitable agents based on capabilities
 * 3. Negotiates task acceptance with candidate agents
 * 4. Assigns the task to the best available agent
 *
 * Supports both automatic assignment and human-in-the-loop approval.
 */

import type { AgentId } from "@orangecoding/core";
import type { AgentRegistry } from "./registry.js";
import type { MessageBus } from "./bus.js";

// ---------------------------------------------------------------------------
// HandoffMessage
// ---------------------------------------------------------------------------

/** Data payload published when one agent hands off a task to another agent. */
export interface HandoffMessage {
  from: AgentId;
  to: AgentId;
  task: string;
}

// ---------------------------------------------------------------------------
// Negotiator
// ---------------------------------------------------------------------------

/**
 * Coordinates task handoffs between agents. It validates that both the source
 * and target agents exist in the registry before publishing the handoff message
 * on the bus.
 */
export class Negotiator {
  private registry: AgentRegistry;
  private bus: MessageBus;

  constructor(registry: AgentRegistry, bus: MessageBus) {
    this.registry = registry;
    this.bus = bus;
  }

  /**
   * Handoff publishes a task handoff message from one agent to another. Both
   * agents must be registered in the registry; otherwise an error is returned.
   */
  handoff(fromID: AgentId, toID: AgentId, task: string): void {
    if (!this.registry.get(fromID)) {
      throw new Error(`handoff: source agent ${fromID} not registered`);
    }
    if (!this.registry.get(toID)) {
      throw new Error(`handoff: target agent ${toID} not registered`);
    }

    this.bus.publish("agent.handoff", {
      from: fromID,
      to: toID,
      task,
    } satisfies HandoffMessage);
  }
}

// ---------------------------------------------------------------------------
// BuddyObserver
// ---------------------------------------------------------------------------

/** Watches for events on the bus and invokes registered handlers. */
export class BuddyObserver {
  private bus: MessageBus;
  private handlers = new Map<string, (data: unknown) => void>();

  constructor(bus: MessageBus) {
    this.bus = bus;
  }

  /**
   * Watch subscribes to events of the given type on the bus and calls handler
   * whenever a message arrives on that topic.
   */
  watch(eventType: string, handler: (data: unknown) => void): void {
    this.handlers.set(eventType, handler);
    this.bus.subscribe(eventType, (_topic: string, data: unknown) => {
      handler(data);
    });
  }
}
