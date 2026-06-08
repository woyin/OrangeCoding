// ---------------------------------------------------------------------------
// TaskId
// ---------------------------------------------------------------------------

export type TaskId = string;

export function newTaskId(id: string): TaskId {
  return id;
}

// ---------------------------------------------------------------------------
// TaskType enum
// ---------------------------------------------------------------------------

export const TaskType = {
  Coding: "coding",
  Review: "review",
  Exploration: "exploration",
  General: "general",
} as const;

export type TaskType = (typeof TaskType)[keyof typeof TaskType];

// ---------------------------------------------------------------------------
// TaskStatus enum
// ---------------------------------------------------------------------------

export const TaskStatus = {
  Pending: "pending",
  Running: "running",
  Completed: "completed",
  Failed: "failed",
  Skipped: "skipped",
} as const;

export type TaskStatus = (typeof TaskStatus)[keyof typeof TaskStatus];

// ---------------------------------------------------------------------------
// Task
// ---------------------------------------------------------------------------

export interface Task {
  id: TaskId;
  type: TaskType;
  description: string;
  priority: number;
  parentId?: TaskId;
  dependencies: TaskId[];
}

// ---------------------------------------------------------------------------
// TaskResult
// ---------------------------------------------------------------------------

export interface TaskResult {
  taskId: TaskId;
  status: TaskStatus;
  output: string;
  error?: Error;
}

export function isTaskError(result: TaskResult): boolean {
  return result.status === TaskStatus.Failed || result.error != null;
}
