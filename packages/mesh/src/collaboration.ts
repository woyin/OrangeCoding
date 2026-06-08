/**
 * @module collaboration
 * Collaboration protocols and routing.
 */

import type { TaskType, Task, TaskResult, TaskId, AgentId } from "@orangecoding/core";

// ---------------------------------------------------------------------------
// TaskClassifier
// ---------------------------------------------------------------------------

/** Determines the task type for routing. */
export interface TaskClassifier {
  classify(task: Task): TaskType;
}

// ---------------------------------------------------------------------------
// AssignmentPlan
// ---------------------------------------------------------------------------

/** Maps tasks to agents. */
export interface AssignmentPlan {
  tasks: Task[];
  assignments: Map<TaskId, AgentId>;
}

// ---------------------------------------------------------------------------
// CollaborationProtocol
// ---------------------------------------------------------------------------

/** Executes a multi-agent collaboration strategy. */
export interface CollaborationProtocol {
  execute(plan: AssignmentPlan): Promise<TaskResult[]>;
}

// ---------------------------------------------------------------------------
// FallbackProtocol (internal)
// ---------------------------------------------------------------------------

class FallbackProtocol implements CollaborationProtocol {
  async execute(): Promise<TaskResult[]> {
    throw new Error("no protocol available for task");
  }
}

// ---------------------------------------------------------------------------
// CollaborationRouter
// ---------------------------------------------------------------------------

/** Routes tasks to the appropriate collaboration protocol. */
export class CollaborationRouter {
  private classifier: TaskClassifier;
  private protocols: Map<string, CollaborationProtocol>;
  private fallback: CollaborationProtocol;

  constructor(
    classifier: TaskClassifier,
    protocols: Map<string, CollaborationProtocol>,
  ) {
    this.classifier = classifier;
    this.protocols = protocols;
    this.fallback = new FallbackProtocol();
  }

  /** Route selects and executes the appropriate protocol for a task. */
  async route(task: Task): Promise<TaskResult[]> {
    const taskType = this.classifier.classify(task);
    const protocol = this.protocols.get(taskType) ?? this.fallback;

    const plan: AssignmentPlan = {
      tasks: [task],
      assignments: new Map(),
    };
    return protocol.execute(plan);
  }

  /** SetProtocol replaces the protocol for a given task type. */
  setProtocol(taskType: TaskType, protocol: CollaborationProtocol): void {
    this.protocols.set(taskType, protocol);
  }
}
